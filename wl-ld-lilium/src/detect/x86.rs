use core::{arch::x86_64::CpuidResult, cell::LazyCell, iter::OnceWith};

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

use crate::helpers::OnceLock;

// Contains the feature array.
// The layout of the array is as follows:
// 0. cpuid[eax=1].ecx
// 1. cpuid[eax=1].edx
// 2. cpuid[eax=7,ecx=0].ecx
// 3. cpuid[eax=7,ecx=0].edx
// 4. cpuid[eax=7,ecx=0].ebx
// 5. cpuid[eax=7,ecx=1].eax
// 6. cpuid[eax=7,ecx=1].ecx
// 7. cpuid[eax=7,ecx=1].edx
// 8. cpuid[eax=7,ecx=1].ebx
// 9. cpuid[eax=7,ecx=2].eax
// 10. cpuid[eax=7,ecx=2].ecx
// 11. cpuid[eax=7,ecx=2].edx
// 12. Reserved
// 13. Reserved
// 14. cpuid[eax=0x80000001].ecx*
// 15. cpuid[eax=0x80000001].edx
// 16. cpuid[eax=0x24,ecx=0].ebx
// 32. cpuid[eax=0x0D, ecx=0].eax
// 33. cpuid[eax=0x0D, ecx=0].edx
// 34. cpuid[eax=0x0D,ecx=0].ecx
// 35. cpuid[eax=0x0D,ecx=1].eax
//
// Reserved fields are set to `0` in the described version of the Kernel. The value may be changed in future versions and must not be relied upon by the Software.
//
// ## Notes about Extended Processor Info (cpuid[eax=0x80000001])
// The value set in `cpu_feature_info[14]` does not exactly match the content of the `ecx` register after a `cpuid` instruction for that leaf,
//  specifically the following differences are observed:
// * Bits 0-9, 12-17, 23, and 24, which are mirrors of the same bits in `cpuid[eax=1].ecx` (`cpu_feature_info[0]`) on AMD Processors only, are set to `0` regardless of the processor,
// * Bit 10, which indicates `syscall` support on the AMD k6 processor only, is clear,
// * Bit 11, which indicates `syscall` support, is set to `1` on an AMD k6 processor that indicates support via `cpuid[eax=0x80000001].ecx[10]`, and
// * Bit 11 may be set to `0` if executed from a 32-bit process running on a 64-bit OS, even if `cpuid` would report it's support.
pub static CPU_FEATURE_INFO: OnceLock<[u32; 48]> = OnceLock::new();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wl_rtld_get_supported_feature_array() -> &'static [u32; 48] {
    CPU_FEATURE_INFO.get_or_init(init_cpuid_features)
}

pub fn init_cpuid_features() -> [u32; 48] {
    let eax1 = unsafe { arch::__cpuid(1) };
    let eax7_ecx0 = unsafe { arch::__cpuid_count(7, 0) };
    let eax7_ecx1 = unsafe { arch::__cpuid_count(7, 1) };
    let eax7_ecx2 = unsafe { arch::__cpuid_count(7, 2) };
    let eax80000001 = unsafe { arch::__cpuid(0x80000001) };

    let mut eax80000001_ecx_val = eax80000001.ecx;

    if cfg!(target_pointer_width = "32") {
        eax80000001_ecx_val &= !0x1BFDFF;
    } else {
        eax80000001_ecx_val &= !0x1BF5FF;
    }

    let eax24_ecx0 = if (eax7_ecx1.edx & (1 << 19)) != 0 {
        unsafe { arch::__cpuid_count(0x24, 0) }
    } else {
        CpuidResult {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
        }
    };

    let eax0d = if (eax1.ecx & (1 << 26)) != 0 {
        [unsafe { arch::__cpuid_count(0x0D, 0) }, unsafe {
            arch::__cpuid_count(0x0D, 1)
        }]
    } else {
        unsafe { core::mem::zeroed() }
    };

    let features = [
        eax1.ecx,
        eax1.edx,
        eax7_ecx0.ecx,
        eax7_ecx1.edx,
        eax7_ecx0.ebx,
        eax7_ecx1.eax,
        eax7_ecx1.ecx,
        eax7_ecx1.edx,
        eax7_ecx1.ebx,
        eax7_ecx2.eax,
        eax7_ecx2.ecx,
        eax7_ecx2.edx,
        0,
        0,
        eax80000001_ecx_val,
        eax80000001.edx,
        eax24_ecx0.eax,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        eax0d[0].eax,
        eax0d[0].edx,
        eax0d[0].ecx,
        eax0d[1].eax,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];

    features
}
