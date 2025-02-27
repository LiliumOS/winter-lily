use core::{arch::global_asm, ffi::c_void, sync::atomic::AtomicPtr};

use crate::syscall_helpers::SysCallTyErased;
use core::convert::Infallible;
use lilium_sys::sys::result::SysResult;

static SYSCALL_SUBSYS_ARRAY: [AtomicPtr<[Option<SysCallTyErased>; 4096]>; 64] =
    [const { AtomicPtr::new(core::ptr::null_mut()) }; 64];

pub unsafe fn register_subsys(subsys: usize, arr: &'static [Option<SysCallTyErased>; 4096]) {
    SYSCALL_SUBSYS_ARRAY[subsys].store(
        core::ptr::from_ref(arr).cast_mut(),
        core::sync::atomic::Ordering::Release,
    )
}

use core::arch::naked_asm;

use lilium_sys::sys::result::errors::UNSUPPORTED_KERNEL_FUNCTION;

#[cfg(target_arch = "x86_64")]
#[naked]
pub(crate) unsafe extern "sysv64" fn __handle_syscall(_: Infallible) -> SysResult {
    unsafe {
        naked_asm! {
            "lea r11, [{SUBSYS_ARR}+rip]",
            "mov r10, rax",
            "shr r10, 12",
            "and rax, 0xFFF",
            "cmp r10, 8",
            "ja 2f",
            "mov r11, qword ptr [r10+8*r11]",
            "test r11, r11",
            "jnz 2f",
            "mov r11, qword ptr [r11+8*rax]",
            "test r11, r11",
            "jnz 2f",
            "jmp r11",
            "2:",
            "mov rax, {UNSUPPORTED_KERNEL_FUNCTION}",
            "ret",
            UNSUPPORTED_KERNEL_FUNCTION = const UNSUPPORTED_KERNEL_FUNCTION,
            SUBSYS_ARR = sym SYSCALL_SUBSYS_ARRAY
        }
    }
}

#[macro_export]
macro_rules! erase {
    ($fn:path) => {{
        fn __typecheck() {
            let __val = $fn;
            __val;
        }

        let __val = $fn;

        let __res: $crate::syscall_helpers::_core::option::Option<
            $crate::syscall_helpers::SysCallTyErased,
        > = unsafe { core::mem::transmute(__val as *mut ()) };
        __res
    }};
}
