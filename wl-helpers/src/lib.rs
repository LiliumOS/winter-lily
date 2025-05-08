#![no_std]
#![feature(try_trait_v2, try_trait_v2_residual, allocator_api, alloc_layout_extra)]

use core::{
    alloc::{AllocError, Allocator},
    cell::{Cell, UnsafeCell},
    ffi::c_void,
    mem::MaybeUninit,
    ops::{ControlFlow, Deref, DerefMut, FromResidual, Residual, Try},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use linux_syscall::{Result as _, SYS_mmap, SYS_mremap, SYS_munmap, syscall};

pub struct OnceLock<T> {
    lock: AtomicUsize,
    val: UnsafeCell<MaybeUninit<T>>,
}

const LOCK: usize = 0x0001;
const INIT: usize = 0x0002;
const POISONED: usize = 1 << (usize::BITS - 1);

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

struct PoisonOnDrop<'a>(&'a AtomicUsize);

impl<'a> Drop for PoisonOnDrop<'a> {
    fn drop(&mut self) {
        self.0.store(POISONED, Ordering::Relaxed);
    }
}

impl<T> OnceLock<T> {
    fn is_init_non_atomic(&mut self) -> bool {
        (*self.lock.get_mut()) & INIT != 0
    }

    fn is_init_atomic(&self) -> bool {
        self.lock.load(Ordering::Acquire) & INIT != 0
    }

    fn check_poison_atomic(&self) {
        if self.lock.load(Ordering::Relaxed) & POISONED != 0 {
            panic!("Previous Initializer Panicked")
        }
    }

    fn check_poison_nonatomic(&mut self) {
        if (*self.lock.get_mut()) & POISONED != 0 {
            panic!("Previous Initializer Panicked")
        }
    }

    /// Constructs a new uninitialized [`OnceLock<T>`]
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(0),
            val: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Constructs a new [`OnceLock<T>`] that is already initialized to `val`
    /// This is useful when you have a generic API
    pub const fn new_init(val: T) -> Self {
        Self {
            lock: AtomicUsize::new(INIT),
            val: UnsafeCell::new(MaybeUninit::new(val)),
        }
    }

    /// Takes the inner value of the [`OnceLock<T>`] if any.
    ///
    /// Safety guaranteed by taking ownership of `self`
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }

    /// Takes the inner value of the [`OnceLock<T>`], if any, reseting it to an uninitialized state.
    ///
    /// Safety guaranteed by taking ownership of `self`.
    ///
    /// Note: This is equivalent to `core::mem::replace(self, OnceLock::new()).into_inner()`
    pub fn take(&mut self) -> Option<T> {
        if self.is_init_non_atomic() {
            *self.lock.get_mut() = 0;
            Some(unsafe { self.val.get_mut().assume_init_read() })
        } else {
            None
        }
    }

    /// Forceably unlocks the [`OnceLock`].
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

        self.check_poison_atomic();

        let mut m = self.lock.fetch_or(LOCK, Ordering::Relaxed);

        while (m & LOCK) != 0 {
            m = self.lock.fetch_or(LOCK, Ordering::Relaxed);
            core::hint::spin_loop();
        }
        self.check_poison_atomic();
        if (m & INIT) != 0 {
            core::sync::atomic::fence(Ordering::Acquire);
            return Try::from_output(unsafe { &*self.val.get().cast::<T>() });
        }

        let bomb = PoisonOnDrop(&self.lock);

        match Try::branch(f()) {
            ControlFlow::Continue(val) => {
                core::mem::forget(bomb);
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
        self.check_poison_nonatomic();
        let val = *self.lock.get_mut();

        if (val & INIT) != 0 {
            return Try::from_output(unsafe { self.val.get_mut().assume_init_mut() });
        }

        if (val & LOCK) != 0 {
            panic!("Detected Reentrant Initialization of `OnceLock`")
        }

        *self.lock.get_mut() = LOCK;
        let bomb = PoisonOnDrop(&self.lock);
        match Try::branch(f()) {
            ControlFlow::Continue(val) => {
                core::mem::forget(bomb);
                *self.lock.get_mut() = INIT;
                return Try::from_output(self.val.get_mut().write(val));
            }
            ControlFlow::Break(r) => {
                core::mem::forget(bomb);
                *self.lock.get_mut() = 0;
                return FromResidual::from_residual(r);
            }
        }
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        self.try_get_or_init(move || Success(f())).0
    }

    pub fn get_or_init_mut(&mut self, f: impl FnOnce() -> T) -> &mut T {
        self.try_get_or_init_mut(move || Success(f())).0
    }

    pub fn get_or_try_init<E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<&T, E> {
        self.try_get_or_init(f)
    }

    pub fn get_or_try_init_mut<E>(
        &mut self,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<&mut T, E> {
        self.try_get_or_init_mut(f)
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

pub struct LazyLock<T, F = fn() -> T> {
    once_lock: OnceLock<T>,
    init_fn: Cell<Option<F>>,
}

unsafe impl<T: Send + Sync, F: Send> Sync for LazyLock<T, F> {}

impl<T, F> LazyLock<T, F> {
    pub const fn new(val: F) -> Self {
        Self {
            once_lock: OnceLock::new(),
            init_fn: Cell::new(Some(val)),
        }
    }

    pub const fn new_init(val: T) -> Self {
        Self {
            once_lock: OnceLock::new_init(val),
            init_fn: Cell::new(None),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.once_lock.get()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.once_lock.get_mut()
    }
}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    pub fn force(&self) -> &T {
        self.once_lock.get_or_init(|| {
            let init_fn = self
                .init_fn
                .replace(None)
                .expect("Previous Initializer Panicked");
            init_fn()
        })
    }

    pub fn force_mut(&mut self) -> &mut T {
        self.once_lock.get_or_init_mut(|| {
            let init_fn = self
                .init_fn
                .get_mut()
                .take()
                .expect("Previous Initializer Panicked");
            init_fn()
        })
    }
}

impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.force()
    }
}

impl<T, F: FnOnce() -> T> DerefMut for LazyLock<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.force_mut()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct MmapAllocator {
    hint_base_addr: *mut c_void,
}

unsafe impl Send for MmapAllocator {}
unsafe impl Sync for MmapAllocator {}

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
        layout: core::alloc::Layout,
    ) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        self.allocate_zeroed(layout)
    }
    fn allocate_zeroed(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
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
                linux_raw_sys::general::PROT_READ | linux_raw_sys::general::PROT_WRITE,
                linux_raw_sys::general::MAP_ANONYMOUS | linux_raw_sys::general::MAP_PRIVATE,
                -1i32,
                0u64
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, size)).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: core::alloc::Layout) {
        let size = layout.size().next_multiple_of(4096);
        if size != 0 {
            let _ = unsafe { syscall!(SYS_munmap, ptr.as_ptr(), size) };
        }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
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
                linux_raw_sys::general::MREMAP_MAYMOVE
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, new_size)).ok_or(AllocError)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
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
                linux_raw_sys::general::MREMAP_MAYMOVE
            )
        };

        res.check().map_err(|_| AllocError)?;

        let ptr = core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked());

        NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, new_size)).ok_or(AllocError)
    }
}

pub mod rand;

pub mod sync;
