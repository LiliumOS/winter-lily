use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, Range},
    str::Utf8Error,
    sync::atomic::{AtomicUsize, Ordering},
};

use core::ptr::NonNull;
use std::{
    alloc::{AllocError, Allocator},
    iter::zip,
    mem::ManuallyDrop,
    ops::{ControlFlow, FromResidual, Residual, Try},
};

use bytemuck::{NoUninit, Zeroable};
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
        core::str::from_utf8(self.as_slice())
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

pub struct OnceLock<T> {
    lock: AtomicUsize,
    val: UnsafeCell<MaybeUninit<T>>,
}

const LOCK: usize = 0x0001;
const INIT: usize = 0x0002;

unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}

struct Success<T>(T);

impl<T> FromResidual<Success<core::convert::Infallible>> for Success<T> {
    fn from_residual(residual: Success<core::convert::Infallible>) -> Self {
        match residual.0 {}
    }
}

impl<T> Try for Success<T> {
    type Output = T;
    type Residual = Success<core::convert::Infallible>;

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        ControlFlow::Continue(self.0)
    }

    fn from_output(output: Self::Output) -> Self {
        Self(output)
    }
}

impl<O> Residual<O> for Success<core::convert::Infallible> {
    type TryType = Success<O>;
}

impl<T> Drop for OnceLock<T> {
    fn drop(&mut self) {
        if self.is_init_non_atomic() {
            unsafe { self.val.get_mut().assume_init_drop() }
        }
    }
}

impl<T> OnceLock<T> {
    fn is_init_non_atomic(&mut self) -> bool {
        (*self.lock.get_mut()) & INIT != 0
    }

    fn is_init_atomic(&self) -> bool {
        self.lock.load(Ordering::Acquire) & INIT != 0
    }

    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(0),
            val: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub const fn new_init(val: T) -> Self {
        Self {
            lock: AtomicUsize::new(INIT),
            val: UnsafeCell::new(MaybeUninit::new(val)),
        }
    }

    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }

    pub fn take(&mut self) -> Option<T> {
        if self.is_init_non_atomic() {
            *self.lock.get_mut() = 0;
            Some(unsafe { self.val.get_mut().assume_init_read() })
        } else {
            None
        }
    }

    pub fn force_unlock(&mut self) {
        *self.lock.get_mut() &= !LOCK;
    }

    fn try_get_or_init<'a, R: Try<Output = T>>(
        &'a self,
        f: impl FnOnce() -> R,
    ) -> <R::Residual as Residual<&'a T>>::TryType
    where
        R::Residual: Residual<&'a T>,
    {
        if self.is_init_atomic() {
            return Try::from_output(unsafe { &*self.val.get().cast::<T>() });
        }

        let mut m = self.lock.fetch_or(LOCK, Ordering::Relaxed);

        while (m & LOCK) != 0 {
            m = self.lock.fetch_or(LOCK, Ordering::Relaxed);
            core::hint::spin_loop();
        }
        if (m & INIT) != 0 {
            core::sync::atomic::fence(Ordering::Acquire);
            return Try::from_output(unsafe { &*self.val.get().cast::<T>() });
        }

        match Try::branch(f()) {
            ControlFlow::Continue(val) => {
                unsafe {
                    self.val.get().cast::<T>().write(val);
                }
                self.lock.store(INIT, Ordering::Release);
                Try::from_output(unsafe { &*self.val.get().cast::<T>() })
            }
            ControlFlow::Break(r) => {
                self.lock.store(0, Ordering::Relaxed);
                FromResidual::from_residual(r)
            }
        }
    }

    fn try_get_or_init_mut<'a, R: Try<Output = T>>(
        &'a mut self,
        f: impl FnOnce() -> R,
    ) -> <R::Residual as Residual<&'a mut T>>::TryType
    where
        R::Residual: Residual<&'a mut T>,
    {
        let val = *self.lock.get_mut();

        if (val & INIT) != 0 {
            return Try::from_output(unsafe { self.val.get_mut().assume_init_mut() });
        }

        if (val & LOCK) != 0 {
            panic!("Detected Reentrant Initialization of `OnceLock`")
        }

        *self.lock.get_mut() = LOCK;

        match Try::branch(f()) {
            ControlFlow::Continue(val) => {
                *self.lock.get_mut() = INIT;
                return Try::from_output(self.val.get_mut().write(val));
            }
            ControlFlow::Break(r) => {
                *self.lock.get_mut() = 0;
                return FromResidual::from_residual(r);
            }
        }
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        self.try_get_or_init(move || Success(f())).0
    }

    pub fn set(&self, val: T) -> Result<(), T> {
        let mut val = Some(val);

        self.try_get_or_init(|| val.take());

        match val {
            Some(val) => Err(val),
            None => Ok(()),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.try_get_or_init(|| None)
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.try_get_or_init_mut(|| None)
    }
}

pub use ld_so_impl::helpers::*;
use linux_syscall::{SYS_mmap, SYS_mremap, SYS_munmap, syscall};

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

#[derive(Copy, Clone, Debug)]
pub struct MmapAllocator {
    hint_base_addr: *mut c_void,
}

impl MmapAllocator {
    pub const fn new_with_hint(hint: *mut c_void) -> Self {
        Self {
            hint_base_addr: hint,
        }
    }
}

unsafe impl Allocator for MmapAllocator {
    fn allocate(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        self.allocate_zeroed(layout)
    }
    fn allocate_zeroed(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        if layout.align() > 4096 {
            return Err(AllocError);
        }

        let size = layout.size().next_multiple_of(4096);

        if size == 0 {
            return Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0));
        }

        let res = unsafe {
            syscall!(
                SYS_mmap,
                self.hint_base_addr,
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
                -1i32,
                0u64
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, size)).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: std::alloc::Layout) {
        let size = layout.size().next_multiple_of(4096);
        if size != 0 {
            let _ = unsafe { syscall!(SYS_munmap, ptr.as_ptr(), size) };
        }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if new_layout.align() > 4096 {
            return Err(AllocError);
        }

        let new_size = new_layout.size().next_multiple_of(4096);

        let old_size = old_layout.size().next_multiple_of(4096);

        if new_size == 0 {
            unsafe {
                self.deallocate(ptr, old_layout);
            }
            return Ok(NonNull::slice_from_raw_parts(new_layout.dangling(), 0));
        }

        if new_size == old_size {
            return Ok(NonNull::slice_from_raw_parts(ptr, new_size));
        }

        let res = unsafe {
            syscall!(
                SYS_mremap,
                ptr.as_ptr(),
                old_size,
                new_size,
                libc::MREMAP_MAYMOVE
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, new_size)).ok_or(AllocError)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if new_layout.align() > 4096 {
            return Err(AllocError);
        }

        let new_size = new_layout.size().next_multiple_of(4096);

        let old_size = old_layout.size().next_multiple_of(4096);

        if old_size == 0 {
            return self.allocate_zeroed(new_layout);
        }

        if new_size == old_size {
            return Ok(NonNull::slice_from_raw_parts(ptr, new_size));
        }

        let res = unsafe {
            syscall!(
                SYS_mremap,
                ptr.as_ptr(),
                old_size,
                new_size,
                libc::MREMAP_MAYMOVE
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, new_size)).ok_or(AllocError)
    }
}
