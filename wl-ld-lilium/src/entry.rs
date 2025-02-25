use alloc::borrow::ToOwned;
use ld_so_impl::loader::Error;
use linux_raw_sys::general::O_RDONLY;
use linux_syscall::{Result as _, SYS_openat, syscall};

use core::ffi::{CStr, c_char, c_ulong, c_void};
use core::ptr::NonNull;

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::resolver::Resolver;
use linux_syscall::{SYS_exit, SYS_prctl, SYS_write};

use crate::auxv::AuxEnt;
use crate::elf::{DynEntryType, ElfDyn};
use crate::helpers::{FusedUnsafeCell, NullTerm, SyncPointer, debug, open_rdonly};
use crate::loader::LOADER;
use crate::{env::__ENV, resolver};

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};

const USAGE_TAIL: &str = "[OPTION]... <binary file> [args...]";

const ARCH: &str = core::env!("ARCH");

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

unsafe extern "C" fn __rust_entry(
    argc: i32,
    mut argv: *mut *mut c_char,
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

    if execfd == !0 {
        if argc < 1 {
            crash_unrecoverably()
        }

        let prg_name = core::str::from_utf8(unsafe { CStr::from_ptr(*argv) }.to_bytes())
            .expect("UTF-8 Required");

        argv = unsafe { argv.add(1) };
        let mut args =
            unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new_unchecked(argv)) };

        let mut exec_name = None::<&CStr>;

        let mut args = args
            .by_ref()
            .map(|&ptr| unsafe { CStr::from_ptr(ptr) })
            .inspect(|v| debug("visit_argv", v.to_bytes()));

        let mut argv0_override = None;

        while let Some(arg) = args.next() {
            match core::str::from_utf8(arg.to_bytes()) {
                Ok("--help") => {
                    println!("Usage: {prg_name} {USAGE_TAIL}");
                    return 0;
                }
                Ok("--version") => {
                    println!(
                        "wl-ld-lilium-{ARCH}.so (VERSION {})",
                        core::env!("CARGO_PKG_VERSION")
                    );
                    println!("winter-lily compatibility layer for linux");
                    println!(
                        "(C) 2025 Lilium Project Developers. This Project is released under the terms of the MIT and Apache-2.0 License"
                    );
                    return 0;
                }
                Ok("--argv0") => {
                    let ptr = unsafe { argv.add(1).read() };

                    argv0_override = Some(ptr);

                    argv = unsafe { argv.add(2) };
                }
                Ok("--preload-subsystem") => todo!("--preload-subsystem"),
                Ok("--preload-native") => todo!("--preload-native"),
                Ok("--preload-lilium") => todo!("--preload-lilium"),
                Ok(x) if x.starts_with("--") => {
                    eprintln!(
                        "Unknown Option {x}. Note that if this is a relative program name, use `./{x}` instead"
                    );
                    return 1;
                }
                Ok(_) | Err(_) => {
                    exec_name = Some(arg);
                    break;
                }
            }
        }

        if let Some(exec_name) = exec_name {
            let fd = unsafe {
                syscall!(
                    SYS_openat,
                    linux_raw_sys::general::AT_FDCWD,
                    exec_name.as_ptr(),
                    O_RDONLY
                )
            };

            if let Err(e) = fd.check() {
                eprintln!(
                    "Failed to open {}: {:?}",
                    unsafe { core::str::from_utf8_unchecked(exec_name.to_bytes()) },
                    e
                );
                return 1;
            }

            execfd = fd.as_usize_unchecked() as i32;
        } else {
            eprintln!("Usage: {prg_name} {USAGE_TAIL}");
            return 1;
        }
    }

    ldso::load_subsystem("base", c"libusi-base.so");

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

pub static WL_RESOLVER: FusedUnsafeCell<Resolver> = FusedUnsafeCell::new(Resolver::ZERO);

const CANNOT_RUN_IN_SECURE: &str =
    "Cannot run winter-lily in secure mode (suid/sgid of target binary is set)";

fn resolve_error(c: &core::ffi::CStr, e: Error) -> ! {
    let bytes = c.to_bytes();

    eprintln!(
        "Could not find: {} ({:?})",
        unsafe { core::str::from_utf8_unchecked(bytes) },
        e
    );
    unsafe {
        let _ = syscall!(SYS_exit, 1);
    }
    unsafe { core::arch::asm!("ud2", options(noreturn)) }
}

use crate::ldso::{self, __MMAP_ADDR, SearchType};
use crate::resolver::lookup_soname;
