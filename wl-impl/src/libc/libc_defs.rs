#![allow(non_camel_case_types)] // want to match the C name here
use core::{
    alloc::Layout,
    arch::{global_asm, naked_asm},
    ffi::{c_int, c_uint, c_ulong, c_void},
};

use alloc::alloc::{alloc, dealloc};
use lilium_sys::{
    misc::MaybeValid,
    uuid::{Uuid, parse_uuid},
};
use linux_raw_sys::general::{
    __sighandler_t, SA_RESTART, SA_SIGINFO, kernel_sigset_t, siginfo_t, stack_t,
};
use linux_syscall::{SYS_rt_sigaction, syscall};

#[repr(C, align(64))]

struct jmp_buf {
    __reg_nv: [*mut c_void; 8],
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
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
            "pop rsi",
            "mov qword ptr [rdi+48], rsp",
            "mov qword ptr [rdi+56], rsi",
            "jmp rsi",
            ".protected {sym}",

            sym = sym __setjmp
        }
    }
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
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
#[derive(Debug)]
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

    eprint!("sigaction({signum}, {:?}, {src:p})", unsafe {
        action.as_ref()
    });

    let ksig_action = if !action.is_null() {
        kernel_sigaction {
            sa_handler_kernel: unsafe { core::mem::transmute((*action).sa_handler) },
            sa_flags: (unsafe { (*action).sa_flags } | SA_RESTORER | SA_SIGINFO | SA_RESTART)
                as u64,
            sa_restorer: Some(impl_restorer),
            sa_mask: unsafe { (*action).sa_mask },
        }
    } else {
        unsafe { core::mem::zeroed() }
    };

    let mut rksig_action: kernel_sigaction = unsafe { core::mem::zeroed() };

    let res = unsafe {
        syscall!(
            SYS_rt_sigaction,
            signum,
            if action.is_null() {
                core::ptr::null()
            } else {
                &raw const ksig_action
            },
            if src.is_null() {
                core::ptr::null_mut()
            } else {
                &raw mut rksig_action
            },
            core::mem::size_of::<kernel_sigset_t>(),
        )
    };

    let r = res.as_usize_unchecked() as i32;

    eprintln!(" = {r}");

    if r >= 0 && !src.is_null() {
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

#[unsafe(naked)]
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

use crate::{eprint, eprintln, helpers::exit_unrecoverably};

#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct mcontext_t {
    pub gregs: [*mut c_void; NGREGS],
    pub fpregs: *mut c_void, // Too lazy to define `fpstate_t` so you get a raw pointer
    __reserved: [u64; 8],
}

#[cfg_attr(target_arch = "x86_64", repr(C, align(16)))]
pub struct max_align_t;

#[unsafe(no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let align = size
        .isolate_most_significant_one()
        .max(core::mem::size_of::<max_align_t>());
    let Ok(layout) = Layout::from_size_align(size + 2 * size_of::<usize>(), align) else {
        return core::ptr::null_mut();
    };

    let ptr = unsafe { alloc(layout).cast::<usize>() };

    if ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        ptr.write(size);
    }
    unsafe {
        ptr.add(1).write(align);
    }

    unsafe { ptr.add(2).cast() }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn aligned_alloc(size: usize, align: usize) -> *mut c_void {
    if size == 0 {
        return core::ptr::null_mut();
    }

    let Ok(layout) = Layout::from_size_align(size + 2 * size_of::<usize>(), align) else {
        return core::ptr::null_mut();
    };

    let ptr = unsafe { alloc(layout) };

    if ptr.is_null() {
        return core::ptr::null_mut();
    }

    let offset = 2 * core::mem::size_of::<usize>()
        + unsafe {
            ptr.add(2 * core::mem::size_of::<usize>())
                .align_offset(align)
        };
    let new_base = unsafe { ptr.add(offset) };
    let ptr = unsafe { ptr.cast::<usize>().sub(2) };
    unsafe {
        ptr.write(size);
    }
    unsafe {
        ptr.add(1).write(align);
    }
    new_base.cast()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let base = unsafe { ptr.cast::<usize>().sub(2) };
    let size = unsafe { base.read() };
    let align = unsafe { base.add(1).read() };
    let Ok(layout) = Layout::from_size_align(size + 2 * size_of::<usize>(), align) else {
        panic!(
            "Potential Heap Memory Corruption (attempted double free?). Expected allocation size and alignment before {ptr:p} but values were bad (Size was {size} and alignment was {align})"
        )
    };

    let real_base = base.map_addr(|v| v & !(align - 1));

    unsafe {
        dealloc(real_base.cast::<u8>(), layout);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __stack_chk_fail() -> ! {
    exit_unrecoverably(Some(parse_uuid("466fbae6-be8b-5525-bd04-ee7153b74f55")))
}
