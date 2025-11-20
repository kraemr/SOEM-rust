use std::{env, ffi::{CStr, CString, c_void}, time::{Duration, Instant}};

mod bindings {
    include!("bindings.rs");
}
use bindings::*;


pub struct Fieldbus 
{
    context: ecx_contextt,
    iface: *const std::os::raw::c_char,
    group: ::std::os::raw::c_uchar,
    roundtrip_time:i32,
    map: [u8; 4096],
}


pub fn fieldbus_initialize( iface : CString,context : ecx_context) -> Fieldbus
{
   return Fieldbus { context: context, iface: iface.into_raw(), group: 0, roundtrip_time: 0, map: [0u8;4096] }
}

pub fn fieldbus_roundtrip(fieldbus : &mut Fieldbus) -> i32
{
    let mut context : ecx_contextt = fieldbus.context;
    let wkc : i32;
    // Init just so rust doesnt complain about it not being initialized
    let mut diff :  timespec = unsafe { osal_current_time() };
    let mut start : timespec = unsafe { osal_current_time() };
    unsafe { ecx_send_processdata(&mut context) };
    let mut end : timespec = unsafe { osal_current_time() };
    
    wkc = unsafe { ecx_receive_processdata(&mut context, EC_TIMEOUTRET.try_into().unwrap()) };
    end = unsafe { osal_current_time() };

    unsafe { osal_time_diff(&mut start, &mut end, &mut diff) };
    fieldbus.roundtrip_time = (diff.tv_sec * 1000000 + diff.tv_nsec / 1000) as i32;
    return wkc;
}


pub fn fieldbus_start(fieldbus : &mut Fieldbus) -> bool {
    let context = &mut fieldbus.context as *mut ecx_contextt;
    let grp = (unsafe { *context }).grouplist[fieldbus.group as usize];

    println!("Initializing SOEM on {:?} ... ", fieldbus.iface);
    if (unsafe { ecx_init(context, fieldbus.iface) } == 0)
    {    
        println!("no socket connection");
        return false;
    }
    println!("done\n");

    println!("Finding autoconfig slaves... ");
    if (unsafe { ecx_config_init(context) } <= 0)
    {
      println!("no slaves found\n");
      return false;
    }
    unsafe{ println!("{} slaves found\n", (*context).slavecount);}

    println!("Sequential mapping of I/O... ");
    unsafe { ecx_config_map_group(context, fieldbus.map.as_mut_ptr() as *mut c_void, fieldbus.group) };
    /*println!("mapped %dO+%dI bytes from %d segments",
          grp->Obytes, grp->Ibytes, grp->nsegments);*/
    /* if grp.nsegments > 1
    {
      /* Show how slaves are distributed */
      for (i = 0; i < grp->nsegments; ++i)
      {
         print!("%s%d", i == 0 ? " (" : "+", grp->IOsegment[i]);
      }
      println!(" slaves)");
   }*/
   println!("Configuring distributed clock... ");
   unsafe { ecx_configdc(context) };
   println!("done\n");

   println!("Waiting for all slaves in safe operational... ");
   unsafe { ecx_statecheck(context, 0, ec_state_EC_STATE_SAFE_OP as u16, (EC_TIMEOUTSTATE * 4).try_into().unwrap()) };
   println!("done\n");

   println!("Send a roundtrip to make outputs in slaves happy... ");
   fieldbus_roundtrip(fieldbus);
   println!("done\n");

   println!("Setting operational state..");
   /* Act on slave 0 (a virtual slave used for broadcasting) */
   let mut slave = (unsafe { *context }).slavelist[0];
   slave.state = ec_state_EC_STATE_OPERATIONAL as u16;
   unsafe { ecx_writestate(context, 0) };
   /* Poll the result ten times before giving up */
   let mut i = 10;
    while i > 0 {
        fieldbus_roundtrip(fieldbus);
        unsafe { ecx_statecheck(context, 0, ec_state_EC_STATE_OPERATIONAL as u16, (EC_TIMEOUTSTATE / 10).try_into().unwrap()) };
        if slave.state == ec_state_EC_STATE_OPERATIONAL as u16
        {
            print!(" all slaves are now operational\n");
            return true;
        }
        i-=1;
    }
   println!(" failed,");
   unsafe { ecx_readstate(context) };
   /*
   for (i = 1; i <= context->slavecount; ++i)
   {
      slave = context->slavelist + i;
      if (slave->state != EC_STATE_OPERATIONAL)
      {
         printf(" slave %d is 0x%04X (AL-status=0x%04X %s)",
                i, slave->state, slave->ALstatuscode,
                ec_ALstatuscode2string(slave->ALstatuscode));
      }
   }
   printf("\n");
*/
   return false;
}

pub fn fieldbus_stop(fieldbus : *mut Fieldbus)
{
   let context = &mut (unsafe { (*fieldbus).context });
   /* Act on slave 0 (a virtual slave used for broadcasting) */
   let slave = &mut context.slavelist[0];
   println!("Requesting init state on all slaves... ");
   slave.state = ec_state_EC_STATE_INIT as u16;
   unsafe { ecx_writestate(context, 0) };
   println!("done");
   println!("Close socket... ");
   unsafe { ecx_close(context) };
   println!("done");
}



pub fn fieldbus_check_state(fieldbus : *mut Fieldbus)
{
    let context =  unsafe {  &mut (*fieldbus).context as *mut ecx_contextt };
    let grp = &mut (unsafe { *context }).grouplist[(unsafe { (*fieldbus).group }) as usize];
    let mut i : int32= 1;
    grp.docheckstate = 0;
    unsafe { ecx_readstate(context) };

    while i < (unsafe { *context }).slavecount {
        let slave = &mut (unsafe { *context }).slavelist[i as usize];
        if slave.group != (unsafe { (*fieldbus).group })
        {
             /* This slave is part of another group: do nothing */
             println!("This slave is part of another group: do nothing");
        }
        else if slave.state != ec_state_EC_STATE_OPERATIONAL as u16
        {
            grp.docheckstate = 1;
            if slave.state == (ec_state_EC_STATE_SAFE_OP + ec_state_EC_STATE_ERROR) as u16
            {
                println!("* Slave {} is in SAFE_OP+ERROR, attempting ACK\n", i);
                slave.state = (ec_state_EC_STATE_SAFE_OP + ec_state_EC_STATE_ACK) as u16;
                unsafe { ecx_writestate(context, i as u16) };
            }
            else if slave.state == ec_state_EC_STATE_SAFE_OP as u16
            {
                println!("* Slave {} is in SAFE_OP, change to OPERATIONAL\n", i);
                slave.state = ec_state_EC_STATE_OPERATIONAL as u16;
                unsafe { ecx_writestate(context, i as u16) };
            }
            else if slave.state > ec_state_EC_STATE_NONE as u16
            {
                if unsafe { ecx_reconfig_slave(context, i as u16, EC_TIMEOUTRET.try_into().unwrap()) } != 0
                {
                    slave.islost = 0;
                    println!("* Slave {} reconfigured\n", i as u16);
                }
            }
            else if slave.islost == 0
            {
                unsafe { ecx_statecheck(context, i as u16, ec_state_EC_STATE_OPERATIONAL as u16, EC_TIMEOUTRET.try_into().unwrap()) };
                if slave.state == ec_state_EC_STATE_NONE as u16
                {
                    slave.islost = 1;
                    println!("* Slave {:?} lost\n", i);
                }
            }
        }
        else if slave.islost != 0
        {
             if slave.state != ec_state_EC_STATE_NONE as u16
            {
                slave.islost = 0;
                println!("* Slave {} found", i);
            }
            else if (unsafe { ecx_recover_slave(context, i as u16, EC_TIMEOUTRET.try_into().unwrap()) }) == 1
            {
                slave.islost = 0;
                println!("* Slave {} recovered\n", i);
            }
        }
        i+=1;
    }

   if grp.docheckstate == 0
   {
      println!("All slaves resumed OPERATIONAL\n");
   }

}

pub fn fieldbus_dump(fieldbus : *mut Fieldbus) -> bool
{
   let mut n : i32;
   let wkc : i32;
   let expected_wkc : i32;
   let context = &mut (unsafe { (*fieldbus).context });
   let grp = context.grouplist[((unsafe { (*fieldbus).group })) as usize];
   wkc = fieldbus_roundtrip(unsafe { &mut *fieldbus });

   expected_wkc = grp.outputsWKC as i32 * 2 + grp.inputsWKC as i32;
   println!("WKC {}", wkc);
   if wkc < expected_wkc
   {
      println!(" wrong (expected {})\n", expected_wkc);
      return false;
   }

   println!("  O:");
   n = 0;
   // We take the pointers address so we can index without moving the pointer in the struct
   let outputs_ptr : *mut u8 = grp.outputs;
   while n < grp.Obytes.try_into().unwrap() {
    let byte = unsafe { *outputs_ptr.add(1) };
    println!(" {:?}", byte);
    n+=1;
   }


   println!("  I:");
   n = 0;
   let inputs_ptr : *mut u8 = grp.inputs;
   while n < grp.Ibytes.try_into().unwrap()
   {
    let byte = unsafe { *inputs_ptr.add(1) };
    println!(" {:?}", byte);
   }

   //println!("  T: {}\r", (long long)context->DCtime);
   return true;
}


pub fn run_loop(fieldbus : *mut Fieldbus){
        let mut i = 1;
        let mut min_time = 0;
        let mut max_time = 0;
        while i <= 10000 {
            println!("Iteration {}:", i);
            if (!fieldbus_dump(fieldbus))
            {
                fieldbus_check_state(fieldbus);
            }
            else if i == 1
            {
                max_time = unsafe { (*fieldbus).roundtrip_time };
                min_time = max_time;
            }
            else if unsafe { (*fieldbus).roundtrip_time } < min_time
            { 
                min_time = unsafe { (*fieldbus).roundtrip_time };
            }
            else if unsafe { (*fieldbus).roundtrip_time } > max_time
            {
                max_time = unsafe {(*fieldbus).roundtrip_time };
            }
            i+=1;
            unsafe { osal_usleep(5000) };
        }
      print!("\nRoundtrip time (usec): min {} max {}\n", min_time, max_time);
      fieldbus_stop(fieldbus);
}

fn main() {
    let ifname = env::args().nth(1).expect("Usage: program <interface_name>");
    let ifname_c = CString::new(ifname).expect("CString conversion failed");
    unsafe {
        let ctxt: ecx_contextt = std::mem::zeroed();
        println!("SOEM initialized");
        let mut fieldbus = fieldbus_initialize(ifname_c,ctxt);
        if fieldbus_start(&mut fieldbus) {
            run_loop(&mut fieldbus);
        }


    }
}
