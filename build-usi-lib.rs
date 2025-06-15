fn main() {
    let pkg = std::env::var("CARGO_PKG_NAME").unwrap();
    let link_target_dir = std::env::var("LINK_TARGET_DIR").unwrap();
    eprintln!("{pkg}");
    let name = pkg.strip_prefix("wl-usi-").unwrap();
    println!("cargo::rerun-if-changed=../build-usi-lib.rs");
    println!("cargo::rustc-link-search=native={link_target_dir}");
    println!("cargo::rustc-link-lib=dylib=wl_ld_lilium");
    println!("cargo::rustc-link-arg=-soname");
    println!("cargo::rustc-link-arg=libusi-{name}.so");
    println!("cargo::rustc-link-arg=-rpath");
    println!("cargo::rustc-link-arg=$ORIGIN");
}
