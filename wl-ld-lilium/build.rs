fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    println!("cargo::rerun-if-changed=build.rs");
    // println!("cargo::rustc-link-arg=-no-dynamic-linker");
    println!("cargo::rustc-link-arg=-soname");
    println!("cargo::rustc-link-arg=wl-ld-lilium-{arch}.so");
    println!("cargo::rustc-env=ARCH={arch}");
    println!(
        "cargo::rustc-link-arg=-T{}/{}.ld",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        arch
    );
}
