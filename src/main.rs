use std::{env, ffi::{CStr, CString}};

mod bindings {
    include!("bindings.rs");
}
use bindings::*;

fn main() {
    let ifname = env::args().nth(1).expect("Usage: program <interface_name>");
    let ifname_c = CString::new(ifname).expect("CString conversion failed");
    unsafe {
        // --------------------------------------------------------------------
        // 1. Find adapters
        // --------------------------------------------------------------------
        let adapter_list = ec_find_adapters();
        if adapter_list.is_null() {
            eprintln!("No adapters found");
            return;
        }
        // --------------------------------------------------------------------
        // 2. Prepare SOEM context
        // --------------------------------------------------------------------
        let mut ctxt: ecx_contextt = std::mem::zeroed();

        // --------------------------------------------------------------------
        // 3. Open master
        // --------------------------------------------------------------------
        let ret = ecx_init(&mut ctxt, ifname_c.as_ptr());
        if ret <= 0 {
            eprintln!("Failed to initialize SOEM");
            ec_free_adapters(adapter_list);
            return;
        }
        println!("SOEM initialized");

        // --------------------------------------------------------------------
        // 4. Scan slaves
        // --------------------------------------------------------------------
        let slave_count = ecx_config_init(&mut ctxt);
        if slave_count <= 0 {
            eprintln!("No slaves found");
            ecx_close(&mut ctxt);
            ec_free_adapters(adapter_list);
            return;
        }
        println!("{} slave(s) found", slave_count);

        // --------------------------------------------------------------------
        // 5. Print slave names
        // --------------------------------------------------------------------
        for i in 1..=slave_count {
            let name_ptr = ctxt.slavelist[i as usize].name.as_ptr();
            let name = CStr::from_ptr(name_ptr);
            println!("Slave {}: {}", i, name.to_string_lossy());
        }

        // --------------------------------------------------------------------
        // 6. Close master
        // --------------------------------------------------------------------
        ecx_close(&mut ctxt);
        ec_free_adapters(adapter_list);
        println!("Done.");
    }
}
