use std::{
    env,
    ffi::{CString, CStr, c_void},
    mem::MaybeUninit,
    time::Duration,
};

mod bindings {
    include!("bindings.rs");
}
use bindings::*;

pub struct Fieldbus {
    context: ecx_contextt,
    iface: CString,
    group: u8,
    roundtrip_time: i32,
    map: [u8; 4096],
}

impl Fieldbus {
    pub fn new(iface: CString) -> Self {
        let context = unsafe { MaybeUninit::zeroed().assume_init() };
        Fieldbus {
            context,
            iface,
            group: 0,
            roundtrip_time: 0,
            map: [0u8; 4096],
        }
    }

    pub fn roundtrip(&mut self) -> i32 {
        let context = &mut self.context as *mut ecx_contextt;

        unsafe {
            let mut start = osal_current_time();
            ecx_send_processdata(context);
            let wkc = ecx_receive_processdata(context, EC_TIMEOUTRET.try_into().unwrap());
            let mut end = osal_current_time();
            let mut diff = MaybeUninit::<timespec>::zeroed().assume_init();
            osal_time_diff(&mut start, &mut end, &mut diff);
            self.roundtrip_time = (diff.tv_sec * 1_000_000 + diff.tv_nsec / 1000) as i32;
            wkc
        }
    }

    pub fn start(&mut self) -> bool {
        let context = &mut self.context as *mut ecx_contextt;

        println!("Initializing SOEM on {:?}", self.iface);
        if unsafe { ecx_init(context, self.iface.as_ptr()) } == 0 {
            println!("No socket connection");
            return false;
        }

        println!("Finding autoconfig slaves...");
        if unsafe { ecx_config_init(context) } <= 0 {
            println!("No slaves found");
            return false;
        }

        println!("Sequential mapping of I/O...");
        unsafe {
            ecx_config_map_group(
                context,
                self.map.as_mut_ptr() as *mut c_void,
                self.group,
            );
        }

        println!("Configuring distributed clock...");
        unsafe { ecx_configdc(context) };

        println!("Waiting for all slaves in safe operational...");
        unsafe {
            ecx_statecheck(
                context,
                0,
                ec_state_EC_STATE_SAFE_OP as u16,
                (EC_TIMEOUTSTATE * 4).try_into().unwrap(),
            )
        };

        println!("Send a roundtrip to make outputs happy...");
        self.roundtrip();

        println!("Setting operational state...");
        let slave = unsafe { &mut (*context).slavelist[0] };
        slave.state = ec_state_EC_STATE_OPERATIONAL as u16;
        unsafe { ecx_writestate(context, 0) };

        // Poll ten times
        for _ in 0..10 {
            self.roundtrip();
            unsafe {
                ecx_statecheck(
                    context,
                    0,
                    ec_state_EC_STATE_OPERATIONAL as u16,
                    (EC_TIMEOUTSTATE / 10).try_into().unwrap(),
                )
            };
            if slave.state == ec_state_EC_STATE_OPERATIONAL as u16 {
                println!("All slaves are now operational");
                return true;
            }
        }

        println!("Failed to reach operational");
        unsafe { ecx_readstate(context) };
        false
    }

    pub fn stop(&mut self) {
        let context = &mut self.context as *mut ecx_contextt;
        let slave = unsafe { &mut (*context).slavelist[0] };
        println!("Requesting init state on all slaves...");
        slave.state = ec_state_EC_STATE_INIT as u16;
        unsafe { ecx_writestate(context, 0) };
        println!("Close socket...");
        unsafe { ecx_close(context) };
    }

    pub fn dump(&mut self) -> bool {
        let context = &mut self.context as *mut ecx_contextt;
        let grp = unsafe { &*context }.grouplist[self.group as usize];

        let wkc = self.roundtrip();
        let expected_wkc = (grp.outputsWKC + grp.inputsWKC) as i32;
        println!("WKC: {} (expected {})", wkc, expected_wkc);
        if wkc < expected_wkc {
            println!("Mismatch");
            return false;
        }

        // Dump outputs
        let outputs = unsafe { std::slice::from_raw_parts(grp.outputs, grp.Obytes as usize) };
        print!("O: ");
        for b in outputs {
            print!("{:02X} ", b);
        }
        println!();

        // Dump inputs
        let inputs = unsafe { std::slice::from_raw_parts(grp.inputs, grp.Ibytes as usize) };
        print!("I: ");
        for b in inputs {
            print!("{:02X} ", b);
        }
        println!();

        true
    }
}

fn main() {
    let ifname = env::args().nth(1).expect("Usage: program <interface>");
    let ifname_c = CString::new(ifname).unwrap();
    let mut fieldbus = Fieldbus::new(ifname_c);

    if fieldbus.start() {
        for i in 1..=10000 {
            println!("Iteration {}", i);
            if !fieldbus.dump() {
                // check state could be implemented
            }
            unsafe { osal_usleep(5000) };
        }
        fieldbus.stop();
    }
}
