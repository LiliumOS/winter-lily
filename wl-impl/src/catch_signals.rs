use core::ffi::c_void;

use lilium_sys::uuid::{Uuid, parse_uuid};
use linux_raw_sys::general::{SIGABRT, SIGBUS, SIGFPE, SIGILL, SIGQUIT, SIGSEGV, siginfo_t};

use core::arch::{global_asm, naked_asm};

use crate::libc::mcontext_t;

use crate::{helpers::exit_unrecoverably, libc::ucontext_t, syscall_handler::__handle_syscall};

pub fn sig_to_except(signo: u32) -> Option<Uuid> {
    match signo {
        SIGABRT => Some(const { parse_uuid("466fbae6-be8b-5525-bd04-ee7153b74f55") }),
        SIGBUS => Some(const { parse_uuid("ef1d81bc-58d9-5779-a4c7-540b9163cdf1") }),
        SIGSEGV => Some(const { parse_uuid("fcf8d451-89e6-50b5-b2e6-396aec58a74a") }),
        SIGILL => Some(const { parse_uuid("9dc46cba-85a4-5b94-be24-03717a40c72b") }),
        SIGFPE => Some(const { parse_uuid("5c91c672-f971-5b6b-a806-d6a6d2c8eb8a") }),
        SIGQUIT => None,
        _ => Some(const { parse_uuid("79a90b8e-8f4b-5134-8aa2-ff68877017db") }),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __sa_handler_seh_impl(signo: u32, siginfo: *mut siginfo_t, uctx: *mut c_void) {
    if signo == linux_raw_sys::general::SIGSYS {
        unsafe {
            invoke_syscall_uctx(&raw mut (*uctx.cast::<ucontext_t>()).uc_mcontext);
        }
        return;
    }

    exit_unrecoverably(sig_to_except(signo))
}

global_asm! {
    ".protected {__sa_handler_seh_impl}",
    __sa_handler_seh_impl = sym __sa_handler_seh_impl,
}

#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
unsafe extern "C" fn invoke_syscall_uctx(uctx: *mut mcontext_t) {
    use crate::libc::{REG_R8, REG_R9, REG_R10, REG_RAX, REG_RDI, REG_RDX, REG_RSI};

    unsafe {
        naked_asm! {
            "push rbx",
            "mov rbx, rdi",
            "mov rax, qword ptr [rbx+8*{RAX}]",
            "mov rdi, qword ptr [rbx+8*{RDI}]",
            "mov rsi, qword ptr [rbx+8*{RSI}]",
            "mov rdx, qword ptr [rbx+8*{RDX}]",
            "mov rcx, qword ptr [rbx+8*{R10}]", // This is correct, because the syscall interface uses `r10` to pass param 4, but we map this to RCX in the proper sysv64 ABI
            "mov r8, qword ptr [rbx+8*{R8}]",
            "mov r9, qword ptr [rbx+8*{R9}]",

            "call {handle_syscall}",
            "mov qword ptr [rbx+8*{RAX}], rax",
            "pop rbx",
            "ret",
            RAX = const REG_RAX,
            RDI = const REG_RDI,
            RSI = const REG_RSI,
            RDX = const REG_RDX,
            R10 = const REG_R10,
            R8 = const REG_R8,
            R9 = const REG_R9,
            handle_syscall = sym __handle_syscall
        }
    }
}
