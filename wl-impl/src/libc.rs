use lilium_sys::misc::MaybeValid;
pub use linux_raw_sys::general::*;

pub type Result<T> = core::result::Result<T, Error>;

pub use linux_errno::Error;

pub use linux_syscall::Result as Check;

pub use linux_syscall::syscall;

pub use core::ffi::*;

macro_rules! def_syscall{
    ($(fn $name:ident($($params:ident: $param_tys:ty),* $(,)?) -> $ret:ty;)*) => {
        $(pub unsafe fn $name($($params: $param_tys),*) -> $crate::libc::Result<$ret> {

            paste::paste! {
                let res = $crate::libc::syscall!(
                    linux_syscall:: [<SYS_ $name>],
                    $($params),*
                );
            }

            $crate::libc::Check::check(&res)?;

            Ok(unsafe { $crate::libc::FromSysVal::from_raw(res.as_usize_unchecked()) })
        })*
    };
}

def_syscall! {
    fn ftruncate(fd: i32, off: __kernel_loff_t) -> c_int;
    fn close(fd: i32) -> ();
    fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
    fn mmap(addr_hint: *mut c_void, length: usize, prot: c_uint, flags: c_uint, fd: c_int, off: __kernel_off_t) -> *mut c_void;
    fn munmap(addr: *mut c_void, length: usize) -> ();
    fn mremap(old_addr: *mut c_void, old_len: usize, new_len: usize, flags: c_uint) -> *mut c_void;
    fn write(fd: i32, data: *const c_void, len: usize) -> usize;
}

pub trait FromSysVal {
    unsafe fn from_raw(raw: usize) -> Self;
}

impl FromSysVal for usize {
    unsafe fn from_raw(raw: usize) -> Self {
        raw
    }
}

impl FromSysVal for c_int {
    unsafe fn from_raw(raw: usize) -> Self {
        raw as c_int
    }
}

impl FromSysVal for isize {
    unsafe fn from_raw(raw: usize) -> Self {
        raw as isize
    }
}

impl<T> FromSysVal for *mut T {
    unsafe fn from_raw(raw: usize) -> Self {
        core::ptr::with_exposed_provenance_mut(raw)
    }
}

impl<T> FromSysVal for *const T {
    unsafe fn from_raw(raw: usize) -> Self {
        core::ptr::with_exposed_provenance(raw)
    }
}

impl FromSysVal for () {
    unsafe fn from_raw(_: usize) -> Self {}
}

mod libc_defs;

pub use libc_defs::*;

pub fn siginfo(
    info_handler: unsafe extern "C" fn(signo: c_int, info: *const siginfo_t, ucontext: *mut c_void),
) -> MaybeValid<unsafe extern "C" fn(signo: c_int)> {
    unsafe { core::mem::transmute(info_handler) }
}
