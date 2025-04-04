#![allow(non_camel_case_types)] // want to match the C name here
use core::{
    arch::{global_asm, naked_asm},
    ffi::{c_int, c_uint, c_ulong, c_void},
};

use lilium_sys::misc::MaybeValid;
use linux_raw_sys::general::{__sighandler_t, SA_SIGINFO, kernel_sigset_t, siginfo_t, stack_t};
use linux_syscall::{SYS_rt_sigaction, syscall};

#[repr(C, align(64))]

struct jmp_buf {
    __reg_nv: [*mut c_void; 8],
}

#[unsafe(no_mangle)]
#[naked]
#[cfg(target_arch = "x86_64")]
unsafe extern "C" fn __setjmp(buf: *mut jmp_buf) -> i32 {
    unsafe {
        naked_asm! {
            "mov rax, 0", // init for first call
            "mov qword ptr [rdi], rbx",
            "mov qword ptr [rdi+8], rbp",
            "mov qword ptr [rdi+16], r12",
            "mov qword ptr [rdi+24], r13",
            "mov qword ptr [rdi+32], r14",
            "mov qword ptr [rdi+40], r15",
            "mov rsi, qword ptr [rsp]",
            "mov qword ptr [rdi+48], rsp",
            "mov qword ptr [rdi+56], rsi",
            "ret",
            ".protected {sym}",

            sym = sym __setjmp
        }
    }
}

#[unsafe(no_mangle)]
#[naked]
#[cfg(target_arch = "x86_64")]
unsafe extern "C-unwind" fn longjmp(buf: *mut jmp_buf, status: i32) -> ! {
    unsafe {
        naked_asm! {
            "mov rax, 1",
            "test rsi, rsi",
            "cmovne rax, rsi",
            "mov rbx, qword ptr [rdi]",
            "mov rbp, qword ptr [rdi+8]",
            "mov r12, qword ptr [rdi+16]",
            "mov r13, qword ptr [rdi+24]",
            "mov r14, qword ptr [rdi+32]",
            "mov r15, qword ptr [rdi+40]",
            "mov rsp, qword ptr [rdi+48]",
            "mov rdi, qword ptr [rdi+56]",
            "jmp rdi",
            ".protected {sym}",
            sym = sym longjmp
        }
    }
}

#[repr(C)]
pub struct sigaction_t {
    pub sa_handler: MaybeValid<unsafe extern "C" fn(signo: c_int)>,
    pub sa_mask: kernel_sigset_t,
    pub sa_flags: c_uint,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sigaction(
    signum: i32,
    action: *const sigaction_t,
    src: *mut sigaction_t,
) -> i32 {
    use linux_raw_sys::general::{SA_RESTORER, kernel_sigaction};

    let ksig_action = kernel_sigaction {
        sa_handler_kernel: unsafe { core::mem::transmute((*action).sa_handler) },
        sa_flags: (unsafe { (*action).sa_flags } | SA_RESTORER | SA_SIGINFO) as u64,
        sa_restorer: Some(impl_restorer),
        sa_mask: unsafe { (*action).sa_mask },
    };

    let mut rksig_action: kernel_sigaction = unsafe { core::mem::zeroed() };

    let res = unsafe {
        syscall!(
            SYS_rt_sigaction,
            signum,
            &raw const ksig_action,
            if src.is_null() {
                core::ptr::null_mut()
            } else {
                &raw mut rksig_action
            }
        )
    };

    let r = res.as_usize_unchecked() as i32;

    if r >= 0 && src.is_null() {
        unsafe {
            (*src).sa_handler = core::mem::transmute(rksig_action.sa_handler_kernel);
        }
        unsafe {
            (*src).sa_flags = rksig_action.sa_flags as c_uint & !SA_RESTORER;
        }
        unsafe {
            (*src).sa_mask = rksig_action.sa_mask;
        }
    }
    r
}

global_asm! {
    ".protected {sigaction}",
    sigaction = sym sigaction
}

#[naked]
#[cfg(target_arch = "x86_64")]
unsafe extern "C" fn impl_restorer() {
    unsafe {
        naked_asm! {
            "mov rax, 15",
            "syscall"
        }
    }
}

#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct ucontext_t {
    pub uc_flags: c_ulong,
    pub uc_link: *mut ucontext_t,
    pub uc_stack: stack_t,
    pub uc_mcontext: mcontext_t,
    pub uc_sigmask: kernel_sigset_t,
    __fpregs_mem: [u64; 64],
    pub __ssp: [u64; 4],
}

#[cfg(target_arch = "x86_64")]
pub const NGREGS: usize = 19;

#[cfg(target_arch = "x86_64")]
mod regno_imp {
    pub const REG_R8: usize = 0;
    pub const REG_R9: usize = 1;
    pub const REG_R10: usize = 2;
    pub const REG_R11: usize = 3;
    pub const REG_R12: usize = 4;
    pub const REG_R13: usize = 5;
    pub const REG_R14: usize = 6;
    pub const REG_R15: usize = 7;
    pub const REG_RDI: usize = 8;
    pub const REG_RSI: usize = 9;
    pub const REG_RBP: usize = 10;
    pub const REG_RBX: usize = 11;
    pub const REG_RDX: usize = 12;
    pub const REG_RAX: usize = 13;
    pub const REG_RCX: usize = 14;
    pub const REG_RSP: usize = 15;
    pub const REG_RIP: usize = 16;
    pub const REG_EFLAGS: usize = 17;
    pub const REG_CSGSFS: usize = 18;
}

pub use regno_imp::*;

#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct mcontext_t {
    pub gregs: [*mut c_void; NGREGS],
    pub fpregs: *mut c_void, // Too lazy to define `fpstate_t` so you get a raw pointer
    __reserved: [u64; 8],
}
