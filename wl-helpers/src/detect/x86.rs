#[macro_export]
#[doc(hidden)]
macro_rules! __x86_feature_to_bit {
    ("x87") => {
        (1, 0)
    };
    ("cmpxchg8b") => {
        (1, 8)
    };
    ("cmpxchg16b") => {
        (0, 16)
    };
    ("rdrand") => {
        (0, 30)
    };
    ("rdseed") => {
        (2, 18)
    };
    ("tsc") => {
        (1, 4)
    };
    ("msr") => {
        (1, 5)
    };
    ("apic") => {
        (1, 9)
    };
    ("sep") => {
        (1, 11)
    };
    ("mtrr") => {
        (1, 12)
    };
    ("pge") => {
        (1, 13)
    };
    ("cmov") => {
        (1, 15)
    };
    ("pat") => {
        (1, 16)
    };
    ("mmx") => {
        (1, 23)
    };
    ("fxsr") => {
        (1, 24)
    };
    ("sse") => {
        (1, 25)
    };
    ("sse2") => {
        (1, 26)
    };
    ("sse3") => {
        (0, 0)
    };
    ("pclmulqdq") => {
        (0, 1)
    };
    ("monitor") => {
        (0, 3)
    };
    ("vmx") => {
        (0, 5)
    };
    ("smx") => {
        (0, 6)
    };
    ("ssse3") => {
        (0, 9)
    };
    ("fma") => {
        (0, 12)
    };
    ("pcid") => {
        (0, 17)
    };
    ("sse4.1") => {
        (0, 19)
    };
    ("sse4.2") => {
        (0, 20)
    };
    ("movbe") => {
        (0, 22)
    };
    ("popcnt") => {
        (0, 23)
    };
    ("aes") => {
        (0, 25)
    };
    ("xsave") => {
        (0, 26)
    };
    ("avx") => {
        (0, 28)
    };
    ("f16c") => {
        (0, 29)
    };
    ("pretetchwt1") => {
        (2, 0)
    };
    ("avx512-vbmi") => {
        (2, 1)
    };
    ("umip") => {
        (2, 2)
    };
    ("pku") => {
        (2, 3)
    };
    ("waitpkg") => {
        (2, 5)
    };
    ("avx512-vbmi2") => {
        (2, 6)
    };
    ("shstk") => {
        (2, 7)
    };
    ("gfni") => {
        (2, 8)
    };
    ("vaes") => {
        (2, 9)
    };
    ("avx512-vnni") => {
        (2, 11)
    };
    ("avx512-bitalg") => {
        (2, 12)
    };
    ("tme_en") => {
        (2, 13)
    };
    ("avx512-vpopcntdq") => {
        (2, 14)
    };
    ("la57") => {
        (2, 16)
    };
    ("rdpid") => {
        (2, 22)
    };
    ("kl") => {
        (2, 23)
    };
    ("movdiri") => {
        (2, 27)
    };
    ("movdir64b") => {
        (2, 28)
    };
    ("enqcmd") => {
        (2, 29)
    };
    ("sgx-lc") => {
        (2, 30)
    };
    ("pks") => {
        (2, 31)
    };
    ("sgx-keys") => {
        (3, 1)
    };
    ("avx512-4vnniw") => {
        (3, 2)
    };
    ("avx512-4fmaps") => {
        (3, 3)
    };
    ("fsrm") => {
        (3, 4)
    };
    ("uintr") => {
        (3, 5)
    };
    ("avx512-vp2intersect") => {
        (3, 8)
    };
    ("serialize") => {
        (3, 14)
    };
    ("pconfig") => {
        (3, 18)
    };
    ("cet-ibt") => {
        (3, 20)
    };
    ("amx-bf16") => {
        (3, 22)
    };
    ("avx512-fp16") => {
        (3, 23)
    };
    ("amx-tile") => {
        (3, 24)
    };
    ("amx-int8") => {
        (3, 25)
    };
    ("fsgsbase") => {
        (4, 0)
    };
    ("sgx") => {
        (4, 2)
    };
    ("bmi1") => {
        (4, 3)
    };
    ("hle") => {
        (4, 4)
    };
    ("avx2") => {
        (4, 5)
    };
    ("smep") => {
        (4, 7)
    };
    ("bmi2") => {
        (4, 8)
    };
    ("erms") => {
        (4, 9)
    };
    ("invpcid") => {
        (4, 10)
    };
    ("rtm") => {
        (4, 11)
    };
    ("mpx") => {
        (4, 14)
    };
    ("avx512f") => {
        (4, 16)
    };
    ("avx512dq") => {
        (4, 17)
    };
    ("adx") => {
        (4, 19)
    };
    ("smap") => {
        (4, 20)
    };
    ("avx512-ifma") => {
        (4, 21)
    };
    ("clflushopt") => {
        (4, 23)
    };
    ("clwb") => {
        (4, 24)
    };
    ("avx512pf") => {
        (4, 26)
    };
    ("avx512er") => {
        (4, 27)
    };
    ("avx512cd") => {
        (4, 28)
    };
    ("sha") => {
        (4, 29)
    };
    ("avx512bw") => {
        (4, 30)
    };
    ("avx512vl") => {
        (4, 31)
    };
    ("sha512") => {
        (5, 0)
    };
    ("sm3") => {
        (5, 1)
    };
    ("sm4") => {
        (5, 2)
    };
    ("rao-int") => {
        (5, 3)
    };
    ("avx-vnni") => {
        (5, 4)
    };
    ("avx512-bf16") => {
        (5, 5)
    };
    ("lass") => {
        (5, 6)
    };
    ("cmpccxadd") => {
        (5, 7)
    };
    ("fzrm") => {
        (5, 11)
    };
    ("rsrcs") => {
        (5, 12)
    };
    ("fred") => {
        (5, 17)
    };
    ("lkgs") => {
        (5, 18)
    };
    ("wrmsrns") => {
        (5, 19)
    };
    ("nmi_src") => {
        (5, 20)
    };
    ("amx-fp16") => {
        (5, 21)
    };
    ("hreset") => {
        (5, 22)
    };
    ("avx-ifma") => {
        (5, 23)
    };
    ("lam") => {
        (5, 26)
    };
    ("msrlist") => {
        (5, 27)
    };
    ("legacy_reduced_isa") => {
        (6, 2)
    };
    ("sipi64") => {
        (6, 4)
    };
    ("avx-vnni-int8") => {
        (7, 4)
    };
    ("avx-ne-convert") => {
        (7, 5)
    };
    ("amx-complex") => {
        (7, 8)
    };
    ("avx-vnni-int16") => {
        (7, 10)
    };
    ("utmr") => {
        (7, 13)
    };
    ("prefetchi") => {
        (7, 14)
    };
    ("user_msr") => {
        (7, 15)
    };
    ("cet-sss") => {
        (7, 18)
    };
    ("avx10") => {
        (7, 19)
    };
    ("apx") => {
        (7, 21)
    };
    ("mwait") => {
        (7, 23)
    };
    ("pbndkb") => {
        (8, 1)
    };
    ("lahf_lm") => {
        (14, 0)
    };
    ("svm") => {
        (14, 2)
    };
    ("cr8_legacy") => {
        (14, 4)
    };
    ("abm") => {
        (14, 5)
    };
    ("sse4a") => {
        (14, 6)
    };
    ("3dnowprefetch") => {
        (14, 8)
    };
    ("xop") => {
        (14, 11)
    };
    ("skinit") => {
        (14, 12)
    };
    ("fma4") => {
        (14, 16)
    };
    ("tbm") => {
        (14, 21)
    };
    ("monitorx") => {
        (14, 29)
    };
    ("syscall") => {
        (15, 11)
    };
    ("nx") => {
        (15, 20)
    };
    ("mmxext") => {
        (15, 22)
    };
    ("fxsr_opt") => {
        (15, 25)
    };
    ("pdpe1gb") => {
        (15, 26)
    };
    ("rdtscp") => {
        (15, 27)
    };
    ("lm") => {
        (15, 29)
    };
    ("3dnowext") => {
        (15, 30)
    };
    ("3dnow") => {
        (15, 31)
    };
    ("avx10-128") => {
        (16, 16)
    };
    ("avx10-256") => {
        (16, 17)
    };
    ("avx10-512") => {
        (16, 18)
    };
    ("xsaveopt") => {
        (35, 0)
    };
    ("xsavec") => {
        (35, 1)
    };
    ("xgetbv_ecx1") => {
        (35, 2)
    };
    ("xss") => {
        (35, 3)
    };
    ("xfd") => {
        (35, 4)
    };
    ("osxsave") => {
        (0, 27)
    };
    ($tt:tt) => {
        ::core::compile_error!(::core::concat!("Unknown feature ", ::core::stringify!($tt)))
    };
}

#[macro_export]
macro_rules! is_x86_feature_enabled {
    ($feature:tt $(,)?) => {{
        let _ = $crate::__x86_feature_to_bit!($feature); // test to ensure the feature is valid

        #[allow(unexpected_cfgs)]
        const __VAL: bool = ::core::cfg!(target_feature = $feature);

        __VAL
    }};
}

#[macro_export]
macro_rules! is_x86_feature_detected {
    ($feature:tt $(,)?) => {
        $crate::is_x86_feature_enabled!($feature)
            || ({
                let (idx, bit) = $crate::__x86_feature_to_bit!($feature);
                (($crate::detect::__wl_rtld_get_supported_feature_array())[idx] & (1 << bit)) != 0
            })
    };
}

#[macro_export]
macro_rules! test_x86_features {
    ($($features:tt),+ $(,)?) => {
        ($($crate::is_x86_feature_enabled!($features))&&+) || ({
            let features_arr = $crate::detect::__wl_rtld_get_supported_feature_array();

            $(($crate::is_x86_feature_enabled!($features) || {
                let (idx, bit) = $crate::__x86_feature_to_bit!($features);
                (features_arr[idx] & (1 << bit)) != 0
            }))&&+
        })
    }
}

pub type FeatureArray = [u32; 48];
