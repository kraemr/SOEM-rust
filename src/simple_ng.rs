use std::{
    env,
    ffi::{CString, c_void},
    mem::MaybeUninit,
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

/*
How do i dynamically get offsets for terminals????
*/
pub fn set_output(fieldbus: &mut Fieldbus, slave_index: usize,byte_offset : u16, output_bit: u8, value: bool) {
    let context = &mut fieldbus.context;
    let grp =  &mut (*context).grouplist[fieldbus.group as usize] ;

    // Get pointer to the start of this slave's outputs
    let slave =   &(*context).slavelist[slave_index];
    println!("Slave {}: Vendor {}, Product {}, Rev {}\n",
       slave_index,
       slave.eep_man,
       slave.eep_id,
       slave.eep_rev);

    unsafe {
        let ptr = grp.outputs.add(byte_offset as usize);
        if value {
            *ptr |= 1 << output_bit;
        } else {
            *ptr &= !(1 << output_bit);
        }

        // send/receive processdata
        ecx_send_processdata(context);
        ecx_receive_processdata(context, EC_TIMEOUTRET.try_into().unwrap());
    }
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

/*
generated with: sudo ./samples/slaveinfo/slaveinfo eth0 -sdo

For WAGO each terminal is a module: 
0x9000      "Module Identification IOM 1"                 [RECORD  maxsub(0x0a / 10)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x0a / 10
    0x09      "Module PDO Group"                            [UNSIGNED16       R_R_R_]      0x0002 / 2
    0x0a      "Module Ident"                                [UNSIGNED32       R_R_R_]      0x80000022 / 2147483682
0x9010      "Module Identification IOM 2"                 [RECORD  maxsub(0x0a / 10)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x0a / 10
    0x09      "Module PDO Group"                            [UNSIGNED16       R_R_R_]      0x0001 / 1
    0x0a      "Module Ident"                                [UNSIGNED32       R_R_R_]      0x06521772 / 106043250
0x9020      "Module Identification IOM 3"                 [RECORD  maxsub(0x0a / 10)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x0a / 10
    0x09      "Module PDO Group"                            [UNSIGNED16       R_R_R_]      0x0002 / 2
    0x0a      "Module Ident"                                [UNSIGNED32       R_R_R_]      0x80000083 / 2147483779
0xf000      "Modular Device Profile"                      [RECORD  maxsub(0x05 / 5)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x05 / 5
    0x01      "Index Distance"                              [UNSIGNED16       R_R_R_]      0x0010 / 16
    0x02      "Maximum Number of Modules"                   [UNSIGNED16       R_R_R_]      0x0040 / 64
    0x03      "Standard Entries in Object 0x8yy0"           [UNSIGNED32       R_R_R_]      0x00000000 / 0
    0x04      "Standard Entries in Object 0x9yy0"           [UNSIGNED32       R_R_R_]      0x00000300 / 768
    0x05      "Module PDO Group of Device"                  [UNSIGNED16       R_R_R_]      0x0000 / 0
0xf00e      "PDO Group Alignment PDO Numbers"             [ARRAY  maxsub(0x03 / 3)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x03 / 3
    0x01      ""                                            [UNSIGNED16       R_R_R_]      0x0000 / 0
    0x02      ""                                            [UNSIGNED16       R_R_R_]      0x0000 / 0
    0x03      ""                                            [UNSIGNED16       R_R_R_]      0x0101 / 257
0xf00f      "Module PDO Group Alignment"                  [ARRAY  maxsub(0x03 / 3)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x03 / 3
    0x01      ""                                            [UNSIGNED16       R_R_R_]      0x0000 / 0
    0x02      ""                                            [UNSIGNED16       R_R_R_]      0x0000 / 0
    0x03      ""                                            [UNSIGNED16       R_R_R_]      0x0002 / 2
0xf030      "Configured Module List"                      [ARRAY  maxsub(0x40 / 64)]
    0x00      "Number of Entries"                           [UNSIGNED8        RWRWRW]      0x00 / 0
0xf040      "Detected Address List"                       [ARRAY  maxsub(0x40 / 64)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x03 / 3
    0x01      ""                                            [UNSIGNED16       R_R_R_]      0x0001 / 1
    0x02      ""                                            [UNSIGNED16       R_R_R_]      0x0002 / 2
    0x03      ""                                            [UNSIGNED16       R_R_R_]      0x0003 / 3
0xf050      "Detected Module List"                        [ARRAY  maxsub(0x40 / 64)]
    0x00      "Number of Entries"                           [UNSIGNED8        R_R_R_]      0x03 / 3
    0x01      ""                                            [UNSIGNED32       R_R_R_]      0x80000022 / 2147483682
    0x02      ""                                            [UNSIGNED32       R_R_R_]      0x06521772 / 106043250
    0x03      ""                                            [UNSIGNED32       R_R_R_]      0x80000083 / 2147483779

*/

fn main() {
    let ifname = env::args().nth(1).expect("Usage: program <interface>");
    let ifname_c = CString::new(ifname).unwrap();
    let mut fieldbus = Fieldbus::new(ifname_c);

    if fieldbus.start() {
        for i in 1..=100000 {
            println!("Iteration {}", i);
            if !fieldbus.dump() {
                // check state could be implemented
            }

                let byte_offset = 0x001C;

            if i % 100 == 0 {
                // 0x001C is the start of the 750-501 process image
                set_output(&mut fieldbus,1,0x001C,1,true);
            }else{
                set_output(&mut fieldbus,1,0x001C,1,false);
            }
            unsafe { osal_usleep(5000) };
        }
        fieldbus.stop();
    }
}
