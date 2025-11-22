extern crate bindgen;
extern crate cc;


// Note! requires some extra build flags and extra logic to work on windows
fn compile_soem(){
    cc::Build::new()
        .files([
            "SOEM/src/ec_base.c",
            "SOEM/src/ec_coe.c",
            "SOEM/src/ec_config.c",  
            "SOEM/src/ec_dc.c",  
            "SOEM/src/ec_eoe.c", 
            "SOEM/src/ec_foe.c",  
            "SOEM/src/ec_main.c",  
            "SOEM/src/ec_print.c",  
            "SOEM/src/ec_soe.c",
            "SOEM/osal/linux/osal.c",
            "SOEM/oshw/linux/nicdrv.c",
            "SOEM/oshw/linux/oshw.c"
        ])
    .include("SOEM/include")
    .include("SOEM/osal/linux")
    .include("SOEM/osal")
    .include("SOEM/oshw/linux/")
    .compile("SOEM");
}

/* Tested with bindgen 0.72.1 on linux */
fn regen_bindings() {
    // Generate Rust bindings with include path fixes
    let bindings = bindgen::Builder::default()
        .header("SOEM/include/soem/soem.h")
        .clang_arg("-Isoem/include")
        .clang_arg("-Isoem/osal")
        .clang_arg("-Isoem/osal/linux")
        .clang_arg("-Isoem/oshw/linux")
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file("src/bindings.rs")
        .expect("Couldn't write bindings!");


}

fn main() {
    let lib_enabled = std::env::var("CARGO_FEATURE_LIB").is_ok();
    let regen_enabled = std::env::var("CARGO_FEATURE_REGEN_BINDINGS").is_ok();

    if regen_enabled {
        println!("regenerating bindings");
        compile_soem();
        regen_bindings();
        return;
    }

    if lib_enabled {
        compile_soem();
        return;
    }
}
