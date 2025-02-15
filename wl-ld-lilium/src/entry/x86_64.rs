use core::ffi::{c_char, c_void};
use std::ptr::NonNull;

use ld_so_impl::resolver::Resolver;
use libc::c_ulong;
use linux_syscall::{SYS_exit, SYS_write, syscall};

use crate::auxv::AuxEnt;
use crate::elf::{DynEntryType, ElfDyn};
use crate::helpers::{FusedUnsafeCell, NullTerm};
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
    "pop rdi",
    "mov rbp, rsp",
    "lea rsi, [rsp]",
    "lea rdx, [rsp+8*rdi+8]",
    "mov rcx, rdx",
    "_start._find_auxv:",
    "mov rax, qword ptr [rcx]",
    "lea rcx, [rcx+8]",
    "test rax, rax",
    "jnz _start._find_auxv",
    "call {rust_entry}",
    "mov edi, eax",
    "mov eax, 60",
    "syscall",
    "ud2",
    rust_entry = sym __rust_entry
}

pub const NATIVE_REGION_SIZE: usize = 4096 * 4096 * 8;

static RESOLVER: FusedUnsafeCell<Resolver> = FusedUnsafeCell::new(Resolver::ZERO);

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

#[repr(usize)]
pub enum SearchType {
    Host,
    Winter = 1,
}

fn lookup_soname(resolver: &'static Resolver, libname: &core::ffi::CStr, udata: *mut c_void) {
    // Safety: `lookup_soname` is only ever called using a provided udata, which is `core::ptr::without_provenance(SearchType as usize)`
    let search: SearchType = unsafe { core::mem::transmute(udata) };
}

unsafe extern "C" fn __rust_entry(
    argc: i32,
    argv: *mut *mut c_char,
    envp: *mut *mut c_char,
    auxv: *mut AuxEnt,
) -> i32 {
    unsafe { __ENV.as_ptr().write(crate::helpers::SyncPointer(envp)) }
    let base_addr = safe_addr_of_mut!(__base_addr);
    let auxv =
        unsafe { NullTerm::<AuxEnt, usize>::from_ptr_unchecked(NonNull::new_unchecked(auxv)) };

    let mut entry = core::ptr::null::<c_void>();

    let mut rand = [0u8; 16];

    unsafe { (&mut *RESOLVER.as_ptr()).set_resolve_error_callback(resolve_error) };

    for auxent in auxv {
        match auxent.at_tag as c_ulong {
            libc::AT_ENTRY => entry = auxent.at_val,
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
