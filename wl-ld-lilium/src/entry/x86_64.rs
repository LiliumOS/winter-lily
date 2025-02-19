use core::ffi::{c_char, c_void};
use std::ptr::NonNull;

use ld_so_impl::resolver::Resolver;
use libc::c_ulong;
use linux_syscall::{SYS_exit, SYS_prctl, SYS_write, syscall};

use crate::auxv::AuxEnt;
use crate::elf::{DynEntryType, ElfDyn};
use crate::helpers::{FusedUnsafeCell, NullTerm, SyncPointer};
use crate::{env::__ENV, resolver};

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};

unsafe extern "C" {
    unsafe static _DYNAMIC: ElfDyn;
    unsafe static mut __base_addr: c_void;
}

core::arch::global_asm! {
    ".globl _start",
    ".hidden _start",
    "_start:",
    "pop r12",
    "xor rbp, rbp",
    "lea r13, [rsp]",
    "lea r14, [rsp+8*rdi+8]",
    "mov r15, r14",
    "_start._find_auxv:",
    "mov rax, qword ptr [r15]",
    "lea r15, [r15+8]",
    "test rax, rax",
    "jnz _start._find_auxv",
    "_start._setup_stack:",
    "mov rdi, rsp",
    "add rdi, 0xFFFF",
    "and rdi, 0xFFFFFFFFFFFF0000",
    "add rdi, {STACK_DISPLACEMENT}",
    "mov rsi, {STACK_SIZE}",
    "mov rdx, {MMAP_PROT}",
    "mov r10, {MMAP_FLAGS}",
    "mov r8, -1",
    "mov r9, 0",
    "mov rax, 9",
    "syscall",
    "mov rsp, rax",
    "mov r8, rax",
    "add rsp, {STACK_SIZE}",
    "mov rdi, r12",
    "mov rsi, r13",
    "mov rdx, r14",
    "mov rcx, r15",
    "call {rust_entry}",
    "mov edi, eax",
    "mov eax, 60",
    "syscall",
    "ud2",
    rust_entry = sym __rust_entry,
    STACK_DISPLACEMENT = const STACK_DISPLACEMENT,
    STACK_SIZE = const STACK_SIZE,
    MMAP_PROT = const const {libc::PROT_READ | libc::PROT_WRITE },
    MMAP_FLAGS = const const { libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_GROWSDOWN | libc::MAP_STACK }
}

pub const NATIVE_REGION_SIZE: usize = 4096 * 4096 * 8;

pub const STACK_DISPLACEMENT: usize = 4096 * 8;

pub const STACK_SIZE: usize = 4096 * 128;

pub static RESOLVER: FusedUnsafeCell<Resolver> = FusedUnsafeCell::new(Resolver::ZERO);

const RES_ERROR: &str = "Could not find: ";

const CANNOT_RUN_IN_SECURE: &str =
    "Cannot run winter-lily in secure mode (suid/sgid of target binary is set)";

fn resolve_error(c: &core::ffi::CStr) -> ! {
    let bytes = c.to_bytes();
    let len = bytes.len();
    let ptr = bytes.as_ptr();

    unsafe {
        let _ = syscall!(
            SYS_write,
            libc::STDERR_FILENO,
            RES_ERROR.as_ptr(),
            RES_ERROR.len()
        );
    }
    unsafe {
        let _ = syscall!(SYS_write, libc::STDERR_FILENO, ptr, len);
    }
    unsafe {
        let _ = syscall!(SYS_write, libc::STDERR_FILENO, &0x0Au8 as *const u8, 1);
    }
    unsafe {
        let _ = syscall!(SYS_exit, 1);
    }
    unsafe { core::arch::asm!("ud2", options(noreturn)) }
}

use crate::ldso::{__MMAP_ADDR, SearchType};
use crate::resolver::lookup_soname;

unsafe extern "C" fn __rust_entry(
    argc: i32,
    argv: *mut *mut c_char,
    envp: *mut *mut c_char,
    auxv: *mut AuxEnt,
    stack_addr: *mut c_void,
) -> i32 {
    unsafe {
        let _ = syscall!(
            SYS_prctl,
            libc::PR_SET_VMA,
            libc::PR_SET_VMA_ANON_NAME,
            stack_addr,
            STACK_SIZE - 4096,
            c"ldso-stack".as_ptr()
        );
    }
    unsafe { __ENV.as_ptr().write(crate::helpers::SyncPointer(envp)) }
    let base_addr = safe_addr_of_mut!(__base_addr);

    unsafe {
        __MMAP_ADDR
            .as_ptr()
            .write(SyncPointer(base_addr.add(NATIVE_REGION_SIZE)))
    }

    let auxv =
        unsafe { NullTerm::<AuxEnt, usize>::from_ptr_unchecked(NonNull::new_unchecked(auxv)) };

    let mut rand = [0u8; 16];

    unsafe { (&mut *RESOLVER.as_ptr()).set_resolve_error_callback(resolve_error) };
    unsafe { (&mut *RESOLVER.as_ptr()).set_resolve_needed(lookup_soname) };

    for auxent in auxv {
        match auxent.at_tag as c_ulong {
            libc::AT_RANDOM => rand = unsafe { auxent.at_val.cast::<[u8; 16]>().read() },
            libc::AT_SECURE => {
                if auxent.at_val.addr() != 0 {
                    unsafe {
                        let _ = syscall!(
                            SYS_write,
                            libc::STDERR_FILENO,
                            CANNOT_RUN_IN_SECURE.as_ptr(),
                            CANNOT_RUN_IN_SECURE.len()
                        );
                    }
                    unsafe {
                        let _ = syscall!(SYS_exit, 1);
                    }
                    unsafe { core::arch::asm!("ud2", options(noreturn)) }
                }
            }
            _ => {}
        }
    }

    let dyn_arr = safe_addr_of!(_DYNAMIC);

    let dyn_arr: NullTerm<'_, ElfDyn, _> = unsafe {
        NullTerm::<ElfDyn, u64>::from_ptr_unchecked(NonNull::new_unchecked(dyn_arr.cast_mut()))
    };

    let arr = dyn_arr.as_slice();

    unsafe {
        RESOLVER.resolve_object(
            base_addr,
            arr,
            &"ld-lilium-x86_64.so",
            core::ptr::null_mut(),
        );
    }

    println!("Hello World!");

    42
}
