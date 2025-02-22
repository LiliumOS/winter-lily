use linux_syscall::syscall;

use core::ffi::{c_char, c_ulong, c_void};
use core::ptr::NonNull;

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::resolver::Resolver;
use linux_syscall::{SYS_exit, SYS_prctl, SYS_write};

use crate::auxv::AuxEnt;
use crate::elf::{DynEntryType, ElfDyn};
use crate::helpers::{FusedUnsafeCell, NullTerm, SyncPointer};
use crate::loader::LOADER;
use crate::{env::__ENV, resolver};

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

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
            linux_raw_sys::prctl::PR_SET_VMA,
            linux_raw_sys::prctl::PR_SET_VMA_ANON_NAME,
            stack_addr,
            STACK_SIZE - 4096,
            c"ldso-stack".as_ptr()
        );
    }
    unsafe { __ENV.as_ptr().write(crate::helpers::SyncPointer(envp)) }
    let base_addr = safe_addr_of_mut!(__base_addr);

    let end_addr = safe_addr_of!(__vaddr_end);

    let native_region_base = end_addr.wrapping_sub(NATIVE_REGION_SIZE);

    unsafe {
        __MMAP_ADDR
            .as_ptr()
            .write(SyncPointer(base_addr.add(NATIVE_REGION_SIZE)))
    }

    LOADER.native_base.store(
        native_region_base.cast_mut(),
        core::sync::atomic::Ordering::Relaxed,
    );

    let auxv =
        unsafe { NullTerm::<AuxEnt, usize>::from_ptr_unchecked(NonNull::new_unchecked(auxv)) };

    let mut rand = [0u8; 16];

    let mut execfd = -1;

    unsafe { (&mut *RESOLVER.as_ptr()).set_resolve_error_callback(resolve_error) };
    unsafe { (&mut *RESOLVER.as_ptr()).set_loader_backend(&LOADER) };

    for auxent in auxv {
        match auxent.at_tag as u32 {
            linux_raw_sys::general::AT_RANDOM => {
                rand = unsafe { auxent.at_val.cast::<[u8; 16]>().read() }
            }
            linux_raw_sys::general::AT_SECURE => {
                if auxent.at_val.addr() != 0 {
                    unsafe {
                        let _ = syscall!(
                            SYS_write,
                            linux_raw_sys::general::STDERR_FILENO,
                            CANNOT_RUN_IN_SECURE.as_ptr(),
                            CANNOT_RUN_IN_SECURE.len()
                        );
                    }
                    unsafe {
                        let _ = syscall!(SYS_exit, 1);
                    }
                    crash_unrecoverably();
                }
            }
            linux_raw_sys::general::AT_EXECFD => {
                execfd = auxent.at_val.addr() as i32;
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
            c"ld-lilium-x86_64.so",
            core::ptr::null_mut(),
        );
    }

    println!("Hello world");

    0
}

unsafe extern "C" {
    safe static _DYNAMIC: ElfDyn;
    unsafe static mut __base_addr: c_void;
    safe static __vaddr_end: c_void;
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
            linux_raw_sys::general::STDERR_FILENO,
            RES_ERROR.as_ptr(),
            RES_ERROR.len()
        );
    }
    unsafe {
        let _ = syscall!(SYS_write, linux_raw_sys::general::STDERR_FILENO, ptr, len);
    }
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            &0x0Au8 as *const u8,
            1
        );
    }
    unsafe {
        let _ = syscall!(SYS_exit, 1);
    }
    unsafe { core::arch::asm!("ud2", options(noreturn)) }
}

use crate::ldso::{__MMAP_ADDR, SearchType};
use crate::resolver::lookup_soname;
