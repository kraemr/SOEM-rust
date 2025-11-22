extern crate bindgen;
extern crate cc;


// Note! requires some extra build flags and extra logic to work on windows
fn compile_soem(){
    cc::Build::new()
        .files([
            "vendor/soem/src/ec_base.c",
            "vendor/soem/src/ec_coe.c",
            "vendor/soem/src/ec_config.c",  
            "vendor/soem/src/ec_dc.c",  
            "vendor/soem/src/ec_eoe.c", 
            "vendor/soem/src/ec_foe.c",  
            "vendor/soem/src/ec_main.c",  
            "vendor/soem/src/ec_print.c",  
            "vendor/soem/src/ec_soe.c",
            "vendor/soem/osal/linux/osal.c",
            "vendor/soem/oshw/linux/nicdrv.c",
            "vendor/soem/oshw/linux/oshw.c"
        ])
    .include("vendor/soem/include")
    .include("vendor/soem/osal/linux")
    .include("vendor/soem/osal")
    .include("vendor/soem/oshw/linux/")
    .compile("soem");
}

/* Tested with bindgen 0.72.1 on linux */
fn regen_bindings() {
    // Generate Rust bindings with include path fixes
    let bindings = bindgen::Builder::default()
        .header("vendor/soem/include/soem/soem.h")
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
