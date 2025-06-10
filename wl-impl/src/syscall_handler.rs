use core::sync::atomic::{AtomicUsize, Ordering};
use core::{arch::global_asm, ffi::c_void, sync::atomic::AtomicPtr};

use crate::syscall_helpers::SysCallTyErased;
use core::convert::Infallible;
use lilium_sys::sys::result::SysResult;
use lilium_sys::uuid::Uuid;

pub struct SubsysInfo {
    pub name: &'static str,
    pub uuid: Uuid,
    pub subsys_version: u64,
    pub max_sysno: u16,
}

static SYSCALL_SUBSYS_ARRAY: [AtomicPtr<[Option<SysCallTyErased>; 4096]>; 64] =
    [const { AtomicPtr::new(core::ptr::null_mut()) }; 64];

static NEXT_DYN_SUBSYS: AtomicUsize = AtomicUsize::new(8);

static SUBSYS_REGISTRY: [AtomicPtr<SubsysInfo>; 64] =
    [const { AtomicPtr::new(core::ptr::null_mut()) }; 64];

pub unsafe fn register_subsys(
    subsys: usize,
    arr: &'static [Option<SysCallTyErased>; 4096],
    info: &'static SubsysInfo,
) {
    let subsys = if subsys == !0 {
        let val = NEXT_DYN_SUBSYS.fetch_add(1, Ordering::Relaxed);
        if val > SYSCALL_SUBSYS_ARRAY.len() {
            panic!(
                "Cannot register more than {} subsystems",
                SYSCALL_SUBSYS_ARRAY.len()
            )
        }
        val
    } else {
        subsys
    };
    core::sync::atomic::fence(Ordering::Release);
    SYSCALL_SUBSYS_ARRAY[subsys].store(core::ptr::from_ref(arr).cast_mut(), Ordering::Relaxed);
    SUBSYS_REGISTRY[subsys].store(core::ptr::from_ref(info).cast_mut(), Ordering::Relaxed);
}

pub fn all_subsystems() -> impl Iterator<Item = (u16, &'static SubsysInfo)> {
    SUBSYS_REGISTRY
        .iter()
        .enumerate()
        // SAFETY: we only store NULL or a reference from `register_subsys`, which is `'static`
        .filter_map(|(num, r)| unsafe {
            r.load(Ordering::Acquire).as_ref().map(|v| (num as u16, v))
        })
}

use core::arch::naked_asm;

use lilium_sys::sys::result::errors::UNSUPPORTED_KERNEL_FUNCTION;

#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
pub(crate) unsafe extern "sysv64" fn __handle_syscall(_: Infallible) -> SysResult {
    unsafe {
        naked_asm! {
            "lea r11, [{SUBSYS_ARR}+rip]",
            "mov r10, rax",
            "shr r10, 12",
            "and rax, 0xFFF",
            "cmp r10, 64",
            "jae 2f",
            "mov r11, qword ptr [8*r10+r11]",
            "test r11, r11",
            "jz 2f",
            "mov r11, qword ptr [r11+8*rax]",
            "test r11, r11",
            "jz 2f",
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

#[macro_export]
macro_rules! def_subsystem{
    ($syscalls:ident) => {
        const _: () = {
            #[unsafe(export_name = $crate::wl_init_subsystem_name!())]
            pub extern "C" fn __init_subsystem() {
                $crate::syscall_handler::
            }
        };

    };
}
