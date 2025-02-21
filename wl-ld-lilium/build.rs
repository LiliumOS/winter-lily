fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // println!("cargo::rustc-link-arg=-no-dynamic-linker");
    println!(
        "cargo::rustc-link-arg=-T{}/{}.ld",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap()
    );
}
