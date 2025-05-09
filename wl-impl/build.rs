#![feature(iter_next_chunk)]
use std::env::VarError;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("cargo should set this");
    let env = std::env::var("WL_LILIUM_TARGET");
    let lilium_target = match &env {
        Ok(st) => {
            let mut v = st.split("-");

            let [arch, second, third, fourth] = v.next_chunk().map_or_else(
                |mut v| [v.next(), v.next(), v.next(), v.next()],
                |v| v.map(Some),
            );

            let arch = arch.expect("There must be an architecture");
            let second = second.expect("There must be at least 2 components");

            let vendor = if third.is_some() && fourth.is_some() {
                second
            } else if third.is_some() && second != "lilium" {
                second
            } else {
                "pc"
            };

            let os = if let Some(third) = third {
                if fourth.is_some() {
                    third
                } else if second != "lilium" {
                    second
                } else {
                    third
                }
            } else {
                second
            };

            let env = if let Some(fourth) = fourth {
                fourth
            } else if let Some(third) = third {
                if second == "lilium" { third } else { "std" }
            } else {
                "std"
            };

            (arch, vendor, os, env)
        }
        Err(VarError::NotUnicode(os)) => panic!("WL_LILIUM_TARGET set to non-UTF-8 text ({os:?})"),
        Err(VarError::NotPresent) => {
            println!("cargo::rustc-env=WL_LILIUM_TARGET={arch}-pc-lilium-std");
            (&*arch, "pc", "lilium", "std")
        }
    };

    println!("cargo::rustc-env=WL_LILIUM_TARGET_ARCH={}", lilium_target.0);
    println!(
        "cargo::rustc-env=WL_LILIUM_TARGET_VENDOR={}",
        lilium_target.1
    );
    println!("cargo::rustc-env=WL_LILIUM_TARGET_OS={}", lilium_target.2);
    println!("cargo::rustc-env=WL_LILIUM_TARGET_ENV={}", lilium_target.3);

    match std::env::var("WL_VENDOR_NAME") {
        Ok(_) => {}
        Err(VarError::NotUnicode(os)) => panic!("WL_VENDOR_NAME set to non-UTF-8 text ({os:?})"),
        Err(VarError::NotPresent) => println!("cargo::rustc-env=WL_VENDOR_NAME=winter-lily"),
    }

    let file = format!("c/signal_support/{}.c", arch);

    println!("cargo::rerun-if-changed={file}");
    println!(
        "cargo::rustc-link-search=native={}",
        std::env::var("OUT_DIR").unwrap()
    );

    cc::Build::new()
        .file(file)
        .std("c17")
        .pic(true)
        .cargo_metadata(true)
        .flag_if_supported("-ffreestanding")
        .flag("-ftls-model=initial-exec")
        .flag("-fvisibility=protected")
        .include("c/signal_support/include")
        .include(format!("c/signal_support/include/{arch}/"))
        .compile("signal_support");
}
