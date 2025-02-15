fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-link-arg=-Wl,--no-dynamic-linker");
    println!("cargo::rustc-link-arg=-Wl,-e,_start");
    println!(
        "cargo::rustc-link-arg=-Wl,-T,{}/{}.ld",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap()
    );
}
