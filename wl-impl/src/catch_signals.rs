use std::os::raw::c_void;

use libc::{mcontext_t, siginfo_t, ucontext_t};

use std::arch::naked_asm;

use crate::syscall_handler::__handle_syscall;

#[unsafe(no_mangle)]
unsafe extern "C" fn __sa_handler_seh_impl(signo: i32, siginfo: *mut siginfo_t, uctx: *mut c_void) {
    if signo == libc::SIGSYS
        && (unsafe { (*siginfo).si_code } == 1 || unsafe { (*siginfo).si_code } == 2)
    {
        let uctx = uctx.cast::<ucontext_t>();
        unsafe { invoke_syscall_uctx(&raw const (*uctx).uc_mcontext) }
    }
}

#[cfg(target_arch = "x86_64")]
#[naked]
unsafe extern "C" fn invoke_syscall_uctx(uctx: *const mcontext_t) {
    use libc::{REG_R8, REG_R9, REG_R10, REG_RAX, REG_RDI, REG_RDX, REG_RSI};

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
