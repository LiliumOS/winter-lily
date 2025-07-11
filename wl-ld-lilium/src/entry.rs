use alloc::borrow::ToOwned;
use ld_so_impl::elf::ElfHeader;
use ld_so_impl::loader::Error;
use lilium_sys::sys::auxv::{AT_LILIUM_INIT_HANDLES, AT_LILIUM_INIT_HANDLES_LEN, AT_RANDOM};
use lilium_sys::sys::handle::{Handle, HandlePtr};
use lilium_sys::sys::kstr::KSlice;
use linux_raw_sys::general::{
    MAP_ANONYMOUS, MAP_PRIVATE, O_RDONLY, PROT_NONE, PROT_READ, PROT_WRITE,
};
use linux_syscall::{Result as _, SYS_close, SYS_mmap, SYS_mprotect, SYS_openat, Syscall, syscall};
use wl_interface_map::{
    GetInitHandlesTy, wl_get_init_handles_name, wl_init_subsystem_name, wl_setup_process_name,
};

use core::ffi::{CStr, c_char, c_ulong, c_void};
use core::ptr::NonNull;

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::resolver::Resolver;
use linux_syscall::{SYS_exit, SYS_prctl, SYS_write};

use crate::auxv::AuxEnt;
use crate::elf::{DynEntryType, ElfDyn};
use crate::env;
use crate::helpers::{
    FusedUnsafeCell, NullTerm, SyncPointer, debug, open_sysroot_rdonly, rand::Gen,
};
use crate::helpers::{pread_exact, udata};
use crate::loader::{LOADER, TLS_MC, Tcb, set_tp, setup_tls_mc, update_tls};
use crate::{env::__environ, resolver};

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};

const USAGE_TAIL: &str = "[OPTION]... <binary file> [args...]";

const ARCH: &str = core::env!("ARCH");

const MMAP_REGION_SIZE: usize = 4096 * 256;

const NAME: &CStr = unsafe {
    CStr::from_bytes_with_nul_unchecked(
        core::concat!("wl-ld-lilium-", core::env!("ARCH"), ".so\0").as_bytes(),
    )
};

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

static __AUXV: FusedUnsafeCell<Option<&[AuxEnt]>> = FusedUnsafeCell::new(None);

static NATIVE_REGION_BASE: FusedUnsafeCell<SyncPointer<*const c_void>> =
    FusedUnsafeCell::new(SyncPointer::null());

unsafe extern "C" fn __rust_entry(
    mut argc: usize,
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
            STACK_SIZE,
            c"ldso-stack".as_ptr()
        );
    }
    let envpc = unsafe { auxv.cast::<*mut c_char>().offset_from_unsigned(envp) };
    unsafe {
        safe_addr_of!(__environ)
            .cast::<*mut *mut c_char>()
            .cast_mut()
            .write(envp)
    }
    let base_addr = safe_addr_of_mut!(__base_addr);

    let end_addr = safe_addr_of!(__vaddr_end);
    let off = end_addr.align_offset(4096);
    let end_addr = end_addr.wrapping_byte_add(off);

    let native_region_base = end_addr.wrapping_sub(NATIVE_REGION_SIZE);

    unsafe {
        __MMAP_ADDR.as_ptr().write(SyncPointer(
            native_region_base.cast_mut().wrapping_sub(MMAP_REGION_SIZE),
        ))
    }

    LOADER.native_base.store(
        native_region_base.cast_mut(),
        core::sync::atomic::Ordering::Relaxed,
    );

    let auxv =
        unsafe { NullTerm::<AuxEnt, usize>::from_ptr_unchecked(NonNull::new_unchecked(auxv)) };

    let auxv = auxv.as_slice();

    unsafe { (*__AUXV.as_ptr()) = Some(auxv) };

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
        RESOLVER.resolve_object(base_addr, arr, Some(NAME), core::ptr::null_mut(), !0, None);
    }

    let mut rand = Gen::seed(rand);

    let lilium_base_addr =
        core::ptr::without_provenance_mut(((rand.next() as usize) & SLIDE_MASK) + (4096 * 4096));

    LOADER
        .winter_base
        .store(lilium_base_addr, core::sync::atomic::Ordering::Relaxed);

    unsafe { RESOLVER.force_resolve_now() }; // For now, config based off `LD_BIND_NOW` later 

    if let Some(n) = env::get_env("WL_LD_DEBUG")
        .or_else(|| env::get_env("LD_DEBUG"))
        .or(option_env!("WL_LD_DEBUG_DEFAULT"))
    {
        let dbg = n.parse().unwrap();
        unsafe {
            RESOLVER.set_debug(dbg);
        }
        unsafe {
            WL_RESOLVER.set_debug(dbg);
        }
    }

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
            .inspect(|v| debug("visit_argv", v.to_bytes()))
            .inspect(|_| argc -= 1);

        let mut argv0_override = core::ptr::null_mut();

        while let Some(arg) = args.next() {
            match core::str::from_utf8(arg.to_bytes()) {
                Ok("--help") => {
                    println!("Usage: {prg_name} {USAGE_TAIL}");
                    println!(
                        "Usage: [binary name] [args...] (requires having the sysroot be the root directory)"
                    );
                    println!();
                    println!(
                        "Entry point for winter-lily - allows executing most Lilium Programs on Linux"
                    );
                    println!("Options:");
                    println!("\t--help: Print this message and exit");
                    println!("\t--version: Print version information and exit");
                    println!(
                        "\t--argv0 [name]: Report <name> (instead of <binary name>) as argv0 passed to the process"
                    );
                    println!(
                        "\t--preload-lilium <module>: Causes <module> to be loaded as a Lilium library before the program or any library in the context of Lilium code."
                    );
                    println!(
                        "\t\t<module> is either a library name (looked up in the lilium search path) or a path to a library."
                    );
                    println!(
                        "\t--preload-native <module>, --preload-subsys <module>: Causes <module> to be loaded as a native library before any library other than ld.so."
                    );
                    println!(
                        "\t\t<module> is either a library name (looked up in the native search path) or a path to a library."
                    );
                    println!(
                        "\t\t--preload-subsys differs from --preload-native in that, after the library is loaded, a symbol named {} is found and executed.",
                        wl_init_subsystem_name!()
                    );
                    println!();
                    println!("Environment Variables:");
                    println!(
                        "\tLD_LIBRARY_PATH_WL_LILIUM: If set to a non-empty string, it contains a list of paths (separated by ':') that are searched when looking for lilium libraries"
                    );
                    println!(
                        "\tLD_LIBRARY_PATH_WL_NATIVE: If set to a non-empty string, it contains a list of paths (separated by ':') that are searched when looking for native libraries"
                    );
                    println!(
                        "\tWL_SYSROOT: Treats absolute paths used to lookup libraries and configuration files as if they are prefixed by the path in this string"
                    );
                    println!(
                        "\tWL_NATIVE_LD_SO_CONF: Look in this file, instead of /etc/ld.so.conf, for the paths to search for native libraries"
                    );
                    println!(
                        "\tWL_LILIUM_LD_SO_CONF: Look in this file, instead of /etc/ld-lilium.so.conf, for paths to search for lilium libraries"
                    );
                    println!(
                        "\tWL_SUBSYS_<name>: Specifies an **absolute path** to use when loading the subsystem with name <name>."
                    );
                    println!();
                    println!("Secure Mode:");
                    println!(
                        "\tLinux and ld.so support something called Secure Mode, which is used when running programs with higher capabilities or with setuid"
                    );
                    println!(
                        "\tProperly supporting Secure Mode requires adjusting substantial party of the behaviour of {prg_name}."
                    );
                    println!(
                        "\tFor simplicitly and security, Secure Mode support is not implemented for winter-lily programs."
                    );
                    println!(
                        "\tIf a Lilium program is run in Secure Mode (according to the `AT_SECURE` auxv entry), {prg_name} will print an error message and exit immediately"
                    );
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

                    argv0_override = ptr;

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
                    argc += 1;
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

        if !argv0_override.is_null() {
            unsafe {
                *argv = argv0_override;
            }
        }
    }

    let tls_block_ptr = unsafe {
        native_region_base
            .cast_mut()
            .wrapping_sub(MMAP_REGION_SIZE)
            .wrapping_sub(TLS_BLOCK_SIZE)
    };

    let init_tls_block = unsafe {
        syscall!(
            SYS_mmap,
            tls_block_ptr,
            TLS_BLOCK_SIZE,
            PROT_NONE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0
        )
    };

    if init_tls_block.check().is_err() {
        crash_unrecoverably();
    }

    let init_tls_block =
        core::ptr::with_exposed_provenance_mut::<c_void>(init_tls_block.as_usize_unchecked());

    let tlsmc = init_tls_block.wrapping_add(TLS_BLOCK_SIZE >> 1);

    let res = unsafe {
        syscall!(
            SYS_mprotect,
            tlsmc,
            core::mem::size_of::<Tcb>(),
            PROT_READ | PROT_WRITE
        )
    };

    if res.check().is_err() {
        crash_unrecoverably();
    }

    unsafe { setup_tls_mc(tlsmc) }

    let tls_block = unsafe {
        syscall!(
            SYS_mmap,
            tls_block_ptr,
            TLS_BLOCK_SIZE,
            PROT_NONE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0
        )
    };

    if tls_block.check().is_err() {
        crash_unrecoverably();
    }

    let tls_block =
        core::ptr::with_exposed_provenance_mut::<c_void>(tls_block.as_usize_unchecked());

    let tp = tls_block.wrapping_add(TLS_BLOCK_SIZE >> 1);

    let res = unsafe {
        syscall!(
            SYS_mprotect,
            tp,
            core::mem::size_of::<Tcb>(),
            PROT_READ | PROT_WRITE
        )
    };

    if res.check().is_err() {
        crash_unrecoverably();
    }

    set_tp(tp);

    unsafe { (&mut *WL_RESOLVER.as_ptr()).force_resolve_now() };
    unsafe { (&mut *WL_RESOLVER.as_ptr()).set_resolve_error_callback(resolve_error) };
    unsafe { (&mut *WL_RESOLVER.as_ptr()).set_loader_backend(&LOADER) };
    unsafe {
        (&mut *WL_RESOLVER.as_ptr()).delegate(&RESOLVER);
    }

    let base = ldso::load_subsystem("base");

    // eprintln!("Entries:");
    // eprintln!("{:#?}", RESOLVER.live_entries());

    let sym = RESOLVER.find_sym(wl_setup_process_name!(C), false);

    eprintln!("Found __wl_impl_setup_process: {:p}", sym);

    let setup_process: wl_interface_map::SetupProcessTy = unsafe { core::mem::transmute(sym) };

    let rand_bytes = bytemuck::must_cast([rand.next(), rand.next()]);

    let base_init_subsystem = RESOLVER.find_sym_in(wl_init_subsystem_name!(C), base, false);

    eprintln!("Found libusi-base.so:__init_subsystem {base_init_subsystem:p}");

    unsafe {
        setup_process(
            native_region_base.cast_mut().cast(),
            NATIVE_REGION_SIZE,
            wl_interface_map::FilterMode::Prctl,
            rand_bytes,
        )
    }

    let base_init_subsystem: wl_interface_map::InitSubsystemTy =
        unsafe { core::mem::transmute(base_init_subsystem) };

    unsafe {
        base_init_subsystem();
    }
    ldso::load_and_init_subsystem("thread");
    ldso::load_and_init_subsystem("io");
    ldso::load_and_init_subsystem("process");
    ldso::load_and_init_subsystem("debug");
    ldso::load_and_init_subsystem("kmgmt");

    let mut header: ElfHeader = bytemuck::zeroed();

    let binary = unsafe {
        WL_RESOLVER.load_from_handle(
            None,
            udata(SearchType::Winter as usize),
            udata(execfd as usize),
            true,
        )
    };

    pread_exact(execfd, 0, bytemuck::bytes_of_mut(&mut header)).unwrap();
    let _ = unsafe { syscall!(SYS_close, execfd) };

    let entry = unsafe { binary.base.add(header.e_entry as usize) };

    eprintln!("Found _start {entry:p}");

    update_tls();

    __setup_auxv(auxv, entry, argv, argc, envp, envpc, &mut rand)
}

fn __setup_auxv(
    host_auxv: &[AuxEnt],
    entry: *mut c_void,
    argv: *mut *mut c_char,
    argc: usize,
    envp: *mut *mut c_char,
    envpc: usize,
    rand: &mut Gen,
) -> ! {
    let mut lilium_aux = bytemuck::zeroed::<[AuxEnt; 16]>();
    let mut init_handles = bytemuck::zeroed::<[HandlePtr<Handle>; 64]>();
    let mut random_bytes = [rand.next(), rand.next()];
    let mut init_handles = KSlice::from_slice_mut(&mut init_handles);

    let get_init_handles = RESOLVER.find_sym(wl_get_init_handles_name!(C), false);

    eprintln!("Found get_init_handles: {get_init_handles:p}");

    let get_init_handles: GetInitHandlesTy = unsafe { core::mem::transmute(get_init_handles) };
    unsafe {
        get_init_handles(&mut init_handles);
    }

    lilium_aux[0] = AuxEnt {
        at_tag: AT_RANDOM as usize,
        at_val: core::ptr::addr_of_mut!(random_bytes).cast(),
    };
    lilium_aux[1] = AuxEnt {
        at_tag: AT_LILIUM_INIT_HANDLES as usize,
        at_val: init_handles.arr_ptr.cast(),
    };
    lilium_aux[2] = AuxEnt {
        at_tag: AT_LILIUM_INIT_HANDLES_LEN as usize,
        at_val: udata(init_handles.len),
    };

    unsafe { __call_entry_point(argc, argv, envp, envpc, lilium_aux.as_mut_ptr(), 3, entry) }
}

unsafe extern "C" {
    safe static _DYNAMIC: ElfDyn;
    unsafe static mut __base_addr: c_void;
    safe static __vaddr_end: c_void;
}

pub const SLIDE_MASK: usize = (4096 * 8 - 1) << 12;

pub const NATIVE_REGION_SIZE: usize = 4096 * 4096 * 64;

pub const STACK_DISPLACEMENT: usize = 4096 * 8;

pub const TLS_BLOCK_SIZE: usize = 4096 * 4096 * 4;

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

#[cfg(target_arch = "x86_64")]
use x86_64::__call_entry_point;
