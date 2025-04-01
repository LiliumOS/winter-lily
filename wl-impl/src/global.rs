use alloc::alloc::{AllocError, Allocator};

use core::alloc::Layout;
use core::{ffi::c_void, ptr::NonNull};

use crate::ministd::*;

use crate::libc::{close, ftruncate, memfd_create, mmap, mremap};

/// An allocator that is backed by shared memory objects (and thus can be passed to other processes).
///
/// By default, the objects are isolated
pub struct ShmemAlloc {
    shm_fd: RwLock<HashMap<NonNull<c_void>, OwnedFd>>,
}

impl ShmemAlloc {
    pub fn new() -> Self {
        Self {
            shm_fd: RwLock::new(HashMap::new()),
        }
    }

    ///
    /// Returns the file descriptor for the backing shmem object (if any) that controls the allocation.
    ///
    /// This returns none if `addr` does not correspond to an original allocation (IE. from [`ShmemAlloc::allocate`]/[`ShmemAlloc::allocate_zeroed`]).
    /// This is also the case if `addr` does not correspond to an allocation (including it it points into an allocation, but is not the starting address of the allocation).
    ///
    /// # Safety
    /// The return value must not be used after `addr` is deallocated.
    /// However, it can be cloned and the result used after `addr` is deallocated.
    ///  Note that `deallocated` means exactly the [`ShmemAlloc::deallocate`] function (or [`ShmemAlloc::shrink`] to a zero-size new_layout)
    ///
    /// It is valid to hold (and use) the return value past any allocator call that affects an allocation unrelated to `addr`.
    /// In addition, it is valid to hold (and use) the return value past any call to [`ShmemAlloc::grow`] or [`ShmemAlloc::shrink`]
    ///
    pub unsafe fn fd_for_alloc(&self, addr: NonNull<u8>) -> Option<BorrowedFd> {
        let shm_mem = self.shm_fd.read();
        shm_mem
            .get(&addr.cast())
            .as_deref()
            .map(|v| v.as_raw_fd())
            // SAFETY:
            // This is valid because the fd's remain open for the lifetime of the process unless the allocation is deallocated (which is assured by the invariant
            .map(|v| unsafe { BorrowedFd::borrow_raw(v) })
    }
}

unsafe impl Allocator for ShmemAlloc {
    fn allocate(
        &self,
        layout: alloc::alloc::Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        self.allocate_zeroed(layout)
    }

    fn allocate_zeroed(
        &self,
        layout: alloc::alloc::Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        if layout.size() == 0 {
            return Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0));
        }

        if layout.align() > 4096 {
            return Err(AllocError);
        }

        let Ok(fd) =
            (unsafe { memfd_create(c"/winter-lily/shemalloc".as_ptr(), crate::libc::MFD_CLOEXEC) })
        else {
            return Err(AllocError);
        };
        if fd < 0 {
            return Err(AllocError);
        }

        if unsafe { ftruncate(fd, layout.size() as crate::libc::__kernel_loff_t) }.is_err() {
            let _ = unsafe { close(fd) };
            return Err(AllocError);
        }

        let Ok(ptr) = (unsafe {
            mmap(
                core::ptr::null_mut(),
                layout.size(),
                crate::libc::PROT_READ | crate::libc::PROT_WRITE,
                crate::libc::MAP_SHARED_VALIDATE | crate::libc::MAP_SYNC,
                fd,
                0,
            )
        }) else {
            let _ = unsafe { close(fd) };
            return Err(AllocError);
        };

        let nn = NonNull::new(ptr).expect("Can't allocate addr 0");
        let mut shm_fd = self.shm_fd.write();
        shm_fd.insert(nn, unsafe { OwnedFd::from_raw_fd(fd) });

        Ok(NonNull::slice_from_raw_parts(nn.cast(), layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: alloc::alloc::Layout) {
        if layout.size() == 0 {
            return;
        }
        let ptr = ptr.cast::<c_void>();

        let _ = unsafe { crate::libc::munmap(ptr.as_ptr(), layout.size()) };
        let mut shm_fd = self.shm_fd.write();
        shm_fd.remove(&ptr);
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: alloc::alloc::Layout,
        new_layout: alloc::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: alloc::alloc::Layout,
        new_layout: alloc::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if new_layout.align() > 4096 {
            return Err(AllocError);
        }

        if old_layout.size() == 0 {
            return self.allocate_zeroed(new_layout);
        }

        let fd = unsafe { self.fd_for_alloc(ptr) };
        let addr = ptr.cast();

        if let Some(fd) = fd {
            if unsafe { ftruncate(fd.as_raw_fd(), new_layout.size() as i64) }.is_err() {
                return Err(AllocError);
            }

            let Ok(new_addr) = (unsafe {
                mremap(
                    addr.as_ptr(),
                    old_layout.size(),
                    new_layout.size(),
                    crate::libc::MREMAP_MAYMOVE,
                )
            }) else {
                return Err(AllocError);
            };

            if (new_addr.addr() as isize) < 0 {
                return Err(AllocError);
            }

            let mut shm_fd = self.shm_fd.write();
            let fd = shm_fd.remove(&addr).unwrap();

            let new_addr = NonNull::new(new_addr).expect("Can't allocate addr 0");

            shm_fd.insert(new_addr, fd);

            Ok(NonNull::slice_from_raw_parts(
                new_addr.cast(),
                new_layout.size(),
            ))
        } else {
            todo!()
        }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if new_layout.size() == 0 {
            unsafe {
                self.deallocate(ptr, old_layout);
            }

            Ok(NonNull::slice_from_raw_parts(new_layout.dangling(), 0))
        } else {
            // We don't actually touch the allocation in this case, just unmap the excess pages.
            let Ok(new_addr) =
                (unsafe { mremap(ptr.as_ptr().cast(), old_layout.size(), new_layout.size(), 0) })
            else {
                return Err(AllocError);
            };

            Ok(NonNull::slice_from_raw_parts(
                NonNull::new(new_addr)
                    .expect("Expected a NonNull address")
                    .cast(),
                new_layout.size(),
            ))
        }
    }
}

mod malloc;
