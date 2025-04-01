fn main() {
    let pkg = std::env::var("CARGO_PKG_NAME").unwrap();
    eprintln!("{pkg}");
    let name = pkg.strip_prefix("wl-usi-").unwrap();
    println!("cargo::rerun-if-changed=../build-usi-lib.rs");
    println!("cargo::rustc-link-arg=-soname");
    println!("cargo::rustc-link-arg=libusi-{name}.so");
}
