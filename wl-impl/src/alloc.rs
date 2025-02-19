use std::{
    alloc::{AllocError, Allocator},
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd},
    ptr::NonNull,
};

use dashmap::{DashMap, mapref::multiple::RefMutMulti};
use libc::{close, ftruncate, memfd_create, mmap, mremap};

/// An allocator that is backed by shared memory objects (and thus can be passed to other processes).
///
/// By default, the objects are isolated
pub struct ShmemAlloc {
    shm_fd: DashMap<NonNull<c_void>, OwnedFd>,
}

impl ShmemAlloc {
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
        self.shm_fd
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
        layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        self.allocate_zeroed(layout)
    }

    fn allocate_zeroed(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        if layout.size() == 0 {
            return Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0));
        }

        if layout.align() > 4096 {
            return Err(AllocError);
        }

        let mut fd = unsafe { memfd_create(c"/winter-lily/shemalloc".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return Err(AllocError);
        }

        let mut res = unsafe { ftruncate(fd, layout.size() as libc::off_t) };
        if fd < 0 {
            unsafe {
                close(fd);
            }
            return Err(AllocError);
        }

        let mut ptr = unsafe {
            mmap(
                core::ptr::null_mut(),
                layout.size(),
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED_VALIDATE | libc::MAP_SYNC,
                fd,
                0,
            )
        };

        if (ptr.addr() as isize) < 0 {
            unsafe {
                close(fd);
            }
            return Err(AllocError);
        }

        let nn = NonNull::new(ptr).expect("Can't allocate addr 0");

        self.shm_fd.insert(nn, unsafe { OwnedFd::from_raw_fd(fd) });

        Ok(NonNull::slice_from_raw_parts(nn.cast(), layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: std::alloc::Layout) {
        if layout.size() == 0 {
            return;
        }
        let ptr = ptr.cast::<c_void>();

        unsafe {
            libc::munmap(ptr.as_ptr(), layout.size());
        }
        self.shm_fd.remove(&ptr);
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
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

        if old_layout.size() == 0 {
            return self.allocate_zeroed(new_layout);
        }

        let fd = unsafe { self.fd_for_alloc(ptr) };
        let addr = ptr.cast();

        if let Some(fd) = fd {
            if unsafe { ftruncate(fd.as_raw_fd(), new_layout.size() as i64) } < 0 {
                return Err(AllocError);
            }

            let new_addr = unsafe {
                mremap(
                    addr.as_ptr(),
                    old_layout.size(),
                    new_layout.size(),
                    libc::MREMAP_MAYMOVE,
                )
            };

            if (new_addr.addr() as isize) < 0 {
                return Err(AllocError);
            }

            let (_, fd) = self.shm_fd.remove(&addr).unwrap();

            let new_addr = NonNull::new(new_addr).expect("Can't allocate addr 0");

            self.shm_fd.insert(new_addr, fd);

            Ok(NonNull::slice_from_raw_parts(
                new_addr.cast(),
                new_layout.size(),
            ))
        } else {
            todo!()
        }
    }
}
