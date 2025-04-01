use core::ffi::c_void;

use linux_raw_sys::general::siginfo_t;

use core::arch::naked_asm;

use crate::syscall_handler::__handle_syscall;

#[unsafe(no_mangle)]
unsafe extern "C" fn __sa_handler_seh_impl(signo: u32, siginfo: *mut siginfo_t, uctx: *mut c_void) {
    if signo == linux_raw_sys::general::SIGSYS
        && (unsafe { (*siginfo).__bindgen_anon_1.__bindgen_anon_1.si_code } == 1
            || unsafe { (*siginfo).__bindgen_anon_1.__bindgen_anon_1.si_code } == 2)
    {
        todo!()
    }
}

#[cfg(target_arch = "x86_64")]
#[naked]
unsafe extern "C" fn invoke_syscall_uctx(uctx: *const c_void) {
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
            RAX = const 0,
            RDI = const 1,
            RSI = const 2,
            RDX = const 3,
            R10 = const 4,
            R8 = const 5,
            R9 = const 6,
            handle_syscall = sym __handle_syscall
        }
    }
}
