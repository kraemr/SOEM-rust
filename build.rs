extern crate bindgen;
extern crate cc;



fn compile_soem(){
    cc::Build::new()
        .files([
            "soem/src/ec_base.c",
            "soem/src/ec_coe.c",
            "soem/src/ec_config.c",  
            "soem/src/ec_dc.c",  
            "soem/src/ec_eoe.c", 
            "soem/src/ec_foe.c",  
            "soem/src/ec_main.c",  
            "soem/src/ec_print.c",  
            "soem/src/ec_soe.c",
            "soem/osal/linux/osal.c",
            "soem/oshw/linux/nicdrv.c",
            "soem/oshw/linux/oshw.c"
        ])
        .include("soem/build/include/")
    .include("soem/include")
    .include("soem/osal/linux")
    .include("soem/osal")
    .include("soem/oshw/linux/")
    .compile("soem");

}

/*
Test with bindgen 0.72.1
*/
fn regen_bindings() {


    // Generate Rust bindings with include path fixes
    let bindings = bindgen::Builder::default()
        .header("soem/include/soem/soem.h")
        .clang_arg("-Isoem/include")
        .clang_arg("-Isoem/build/include")   // <- important for ec_options.h
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
