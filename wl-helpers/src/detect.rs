unsafe extern "C" {
    pub safe fn __wl_rtld_get_supported_feature_array() -> &'static FeatureArray;
}

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
pub mod x86;
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
pub use x86::FeatureArray;
