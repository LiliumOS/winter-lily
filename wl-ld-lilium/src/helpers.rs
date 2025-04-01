use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, Range},
    str::Utf8Error,
    sync::atomic::{AtomicUsize, Ordering},
};

use core::ptr::NonNull;
use core::{
    alloc::{AllocError, Allocator},
    ffi::{CStr, c_char},
    iter::zip,
    mem::ManuallyDrop,
    ops::{ControlFlow, FromResidual, Residual, Try},
};

use alloc::boxed::Box;
use bytemuck::{NoUninit, Zeroable};
use ld_so_impl::arch::crash_unrecoverably;
use lilium_sys::misc::MaybeValid;

#[repr(transparent)]
pub struct SyncPointer<T>(pub T);

unsafe impl<T: ?Sized> Send for SyncPointer<*const T> {}
unsafe impl<T: ?Sized> Sync for SyncPointer<*const T> {}
unsafe impl<T: ?Sized> Send for SyncPointer<*mut T> {}
unsafe impl<T: ?Sized> Sync for SyncPointer<*mut T> {}

impl<T> SyncPointer<*const T> {
    pub const fn null() -> Self {
        Self(core::ptr::null())
    }
}

impl<T> SyncPointer<*mut T> {
    pub const fn null_mut() -> Self {
        Self(core::ptr::null_mut())
    }
}

/// A type that can be reliably compared with zero bitwise
///
/// The type must not contain any padding bytes or [`MaybeUninit`][core::mem::MaybeUninit]
pub unsafe trait ZeroPrimitive: Copy + Zeroable {}

unsafe impl ZeroPrimitive for u8 {}
unsafe impl ZeroPrimitive for u16 {}
unsafe impl ZeroPrimitive for u32 {}
unsafe impl ZeroPrimitive for u64 {}
unsafe impl ZeroPrimitive for i8 {}
unsafe impl ZeroPrimitive for i16 {}
unsafe impl ZeroPrimitive for i32 {}
unsafe impl ZeroPrimitive for i64 {}
unsafe impl ZeroPrimitive for usize {}
unsafe impl ZeroPrimitive for isize {}
unsafe impl ZeroPrimitive for char {}
unsafe impl<T> ZeroPrimitive for *mut T {}
unsafe impl<T> ZeroPrimitive for *const T {}
unsafe impl<T: ZeroPrimitive> ZeroPrimitive for MaybeValid<T> {}

const unsafe fn is_zero<T: ZeroPrimitive>(p: *const T) -> bool {
    let mut p = p.cast::<u8>();
    let _ = unsafe { p.add(core::mem::size_of::<T>()) }; // Give llvm a hint that it can vectorize this
    let mut i = 0;
    while i != core::mem::size_of::<T>() {
        if unsafe { p.read() } != 0 {
            return false;
        }
        i = i + 1;
        p = unsafe { p.add(1) };
    }
    true
}

pub struct NullTerm<'a, T, R = T>(NonNull<T>, PhantomData<&'a [T]>, PhantomData<R>);

impl<'a, T, R> Clone for NullTerm<'a, T, R> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData, PhantomData)
    }
}

impl<'a, T, R> NullTerm<'a, T, R> {
    pub const unsafe fn from_ptr_unchecked(ptr: NonNull<T>) -> Self {
        const {
            assert!(core::mem::size_of::<R>() <= core::mem::size_of::<T>());
        }
        Self(ptr, PhantomData, PhantomData)
    }
}

impl<'a, T, R: ZeroPrimitive> NullTerm<'a, T, R> {
    pub const fn as_slice(&self) -> &'a [T] {
        let mut end = self.0;
        loop {
            let ptr = end.cast::<R>();
            if unsafe { is_zero(ptr.as_ptr()) } {
                break unsafe {
                    core::slice::from_ptr_range(Range {
                        start: self.0.as_ptr().cast_const(),
                        end: end.as_ptr().cast_const(),
                    })
                };
            }
            end = unsafe { end.add(1) };
        }
    }
}

impl<'a> NullTerm<'a, u8, u8> {
    #[inline(always)]
    pub const fn as_utf8(&self) -> Result<&'a str, Utf8Error> {
        Ok(unsafe { core::str::from_utf8_unchecked(self.as_slice()) })
    }
}

impl<'a, T, R: ZeroPrimitive> Iterator for NullTerm<'a, T, R> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let ptr = self.0.cast::<R>();

        if unsafe { is_zero(ptr.as_ptr()) } {
            None
        } else {
            let val = self.0;
            self.0 = unsafe { self.0.add(1) };
            Some(unsafe { val.as_ref() })
        }
    }
}

/// A cell that allows safe shared access but only unsafe mutable access
pub struct FusedUnsafeCell<T: ?Sized>(UnsafeCell<T>);

unsafe impl<T: ?Sized + Sync> Sync for FusedUnsafeCell<T> {}

impl<T> FusedUnsafeCell<T> {
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: ?Sized> FusedUnsafeCell<T> {
    pub const fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    /// Obtains a pointer to the inner value
    ///
    /// ## Safety
    /// This pointer is the same as a pointer returned from [`UnsafeCell`] - it can be used either immutably or mutably.
    /// However, care must be taken when used mutable, as it is safe to obtain a shared reference to the internal value.
    ///
    /// The raw pointer can be used for mutable access only if no outstanding shared references exist.
    /// As shared references can be created safely, this can only be done near the very start of the object's lifetime, or if you can otherwise assure no other references are outstanding.
    ///
    /// Additionally, like [`UnsafeCell`], no synchronization is performed. You must ensure any synchronization required is performed after mutations before any other thread uses [`FusedUnsafeCell::get_shared`]
    ///
    /// It is trivially safe to use the pointer for immutable access, provided it or another pointer isn't also being used as a mutable reference.
    /// This can be safer if mutation is happening,
    pub const fn as_ptr(&self) -> *mut T {
        self.0.get()
    }

    /// Obtains a shared reference to the inner value safely.
    pub const fn get_shared(&self) -> &T {
        unsafe { &*self.0.get() }
    }
}

impl<T: ?Sized> Deref for FusedUnsafeCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_shared()
    }
}

pub use wl_helpers::*;

pub use ld_so_impl::helpers::*;
use linux_syscall::{
    SYS_close, SYS_getdents64, SYS_mmap, SYS_mremap, SYS_munmap, SYS_open, SYS_openat, SYS_write,
    syscall,
};

pub fn copy_to_slice_head<'a, T: Copy>(dest: &'a mut [T], src: &[T]) -> &'a mut [T] {
    if dest.len() < src.len() {
        panic!()
    }
    for (src, dest) in zip(src, &mut *dest) {
        *dest = *src;
    }

    &mut dest[src.len()..]
}

use core::ffi::c_void;
use linux_syscall::Result as _;

use crate::env::get_cenv;

pub struct NoGlobalAlloc;

unsafe impl GlobalAlloc for NoGlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug("alloc", b"Use MmapAllocator instead of Global");
        crash_unrecoverably()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug("alloc", b"Use MmapAllocator instead of Global");
        crash_unrecoverably()
    }
}

#[global_allocator]
static GLOBAL_ALLOC: NoGlobalAlloc = NoGlobalAlloc;

#[inline(always)]
pub fn safe_zeroed<T: Zeroable>() -> T {
    let mut val = MaybeUninit::<T>::uninit();

    let p = val.as_mut_ptr().cast::<u8>();

    for i in 0..core::mem::size_of::<T>() {
        unsafe { p.add(i).write(0) }
    }

    unsafe { val.assume_init() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const i8, b: *mut i8, len: usize) -> i32 {
    for i in 0..len {
        match (unsafe { a.add(i).read() }) - (unsafe { b.add(i).read() }) {
            0 => continue,
            -128..=-1 => return -1,
            1..=127 => return 1,
        }
    }

    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(ptr: *mut u8, b: i32, len: usize) -> *mut u8 {
    for i in 0..len {
        unsafe { ptr.add(i).write(b as u8) }
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    for i in 0..len {
        unsafe { dest.add(i).write(src.add(i).read()) }
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    for i in 0..len {
        unsafe { dest.add(i).write(src.add(i).read()) }
    }
    dest
}

ld_so_impl::hidden_syms!(memcmp, memset, memmove, memcpy);

pub struct SplitAscii<'a>(&'a [u8], u8);

impl<'a> SplitAscii<'a> {
    pub const fn new(v: &'a str, val: u8) -> Self {
        if val > 0x80 {
            panic!()
        }
        Self(v.as_bytes(), val)
    }

    pub const fn as_str(&self) -> &'a str {
        unsafe { core::str::from_utf8_unchecked(self.0) }
    }

    #[inline]
    pub fn split_once(mut self) -> (&'a str, &'a str) {
        let val = self.next().unwrap_or("");

        (val, self.as_str())
    }

    #[inline]
    pub fn rsplit_once(mut self) -> (&'a str, &'a str) {
        let rval = self.next_back().unwrap_or("");

        (self.as_str(), rval)
    }

    pub fn find(&self) -> Option<usize> {
        for (i, b) in self.0.iter().enumerate() {
            if *b == self.1 {
                return Some(i);
            }
        }
        None
    }
}

impl<'a> Iterator for SplitAscii<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.0.is_empty() {
            return None;
        } else {
            for (i, b) in self.0.iter().enumerate() {
                if *b == self.1 {
                    let v = unsafe { core::str::from_utf8_unchecked(&self.0[..i]) };

                    self.0 = &self.0[(i + 1)..];

                    return Some(v);
                }
            }

            let v = unsafe { core::str::from_utf8_unchecked(self.0) };
            self.0 = &self.0[self.0.len()..];

            Some(v)
        }
    }
}

impl<'a> DoubleEndedIterator for SplitAscii<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        } else {
            for (i, b) in self.0.iter().enumerate().rev() {
                if *b == self.1 {
                    let v = unsafe { core::str::from_utf8_unchecked(&self.0[(i + 1)..]) };

                    self.0 = &self.0[..i];

                    return Some(v);
                }
            }

            let v = unsafe { core::str::from_utf8_unchecked(self.0) };
            self.0 = &self.0[..1];

            Some(v)
        }
    }
}

const DEBUG_PROMPT: &str = "[debug] ";
#[inline]
pub fn debug(src: &str, buf: &[u8]) {
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            DEBUG_PROMPT.as_ptr(),
            DEBUG_PROMPT.len()
        );
    }
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            src.as_ptr(),
            src.len()
        );
    }
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            b": ".as_ptr(),
            2
        );
    }
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            buf.as_ptr(),
            buf.len()
        );
    }
    unsafe {
        let _ = syscall!(
            SYS_write,
            linux_raw_sys::general::STDERR_FILENO,
            &b'\n' as *const u8,
            1
        );
    }
}

static SYSROOT_FD: OnceLock<i32> = OnceLock::new();

pub fn open_sysroot_rdonly(mut at_fd: i32, st: &str) -> crate::io::Result<i32> {
    debug("open_rdonly", st.as_bytes());
    let mut path = safe_zeroed::<[u8; 256]>();
    copy_to_slice_head(&mut path, st.as_bytes())[0] = 0;

    let mut path = &path[..];
    if path[0] == b'/' {
        path = &path[1..];
        at_fd = *(SYSROOT_FD.get_or_try_init(|| {
            let sysroot = get_cenv("WL_SYSROOT").unwrap_or(c"/");

            let ptr = sysroot.as_ptr();

            let fd = unsafe {
                syscall!(
                    SYS_open,
                    ptr,
                    linux_raw_sys::general::O_RDONLY | linux_raw_sys::general::O_DIRECTORY
                )
            };
            fd.check()?;

            Ok(fd.as_usize_unchecked() as i32)
        })?);
    }
    let fd = unsafe {
        syscall!(
            SYS_openat,
            at_fd,
            path.as_ptr(),
            linux_raw_sys::general::O_RDONLY
        )
    };
    fd.check()?;

    Ok(fd.as_usize_unchecked() as i32)
}

#[repr(C)]
struct Dirent64 {
    d_ino: i64,
    d_off: i64,
    d_reclen: u16,
    d_type: u8,
    d_name: [c_char; 0],
}

pub fn has_prefix(long: &[u8], prefix: &[u8]) -> bool {
    if long.len() < prefix.len() {
        false
    } else {
        &long[..prefix.len()] == prefix
    }
}

pub fn has_suffix(long: &[u8], suffix: &[u8]) -> bool {
    if long.len() < suffix.len() {
        false
    } else {
        let n = long.len() - suffix.len();
        &long[n..] == suffix
    }
}

pub fn expand_glob(
    path: &str,
    mut f: impl FnMut(i32, &CStr) -> crate::io::Result<()>,
) -> crate::io::Result<()> {
    let (prefix, suffix) = SplitAscii::new(path, b'*').split_once();

    let (dir, prefix) = SplitAscii::new(prefix, b'/').rsplit_once();

    debug("expand_glob(prefix)", prefix.as_bytes());
    debug("expand_glob(suffix)", suffix.as_bytes());

    let fd = open_sysroot_rdonly(linux_raw_sys::general::AT_FDCWD, dir)?;

    let mut buf = unsafe {
        Box::<MaybeUninit<[u8; 4096]>, _>::assume_init(Box::<[u8; 4096], _>::new_zeroed_in(
            MmapAllocator::new_with_hint(
                crate::ldso::__MMAP_ADDR
                    .get_shared()
                    .0
                    .wrapping_add(4096 * 4),
            ),
        ))
    };

    let res = (|| -> crate::io::Result<()> {
        loop {
            let len = unsafe { syscall!(SYS_getdents64, fd, buf.as_mut_ptr(), 4096) };

            len.check()?;

            let len = len.as_usize_unchecked();
            if len == 0 {
                break Ok(());
            }
            let mut buf = buf.as_ptr();

            let mut n = 0;

            while n < len {
                let ent = buf.cast::<Dirent64>();

                let rlen = unsafe { (*ent).d_reclen } as usize;

                n += rlen;
                buf = unsafe { buf.add(rlen) };

                let name = unsafe { core::ptr::addr_of!((*ent).d_name).cast::<i8>() };

                let name = unsafe { cstr_from_ptr(name) };

                debug("expand_glob", name.to_bytes());

                let name_bytes = name.to_bytes();

                if has_prefix(name_bytes, prefix.as_bytes())
                    && has_suffix(name_bytes, suffix.as_bytes())
                {
                    f(fd, name)?;
                }
            }
        }
    })();

    let _ = unsafe { syscall!(SYS_close, fd) };

    res
}
