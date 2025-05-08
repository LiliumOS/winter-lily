use core::{
    alloc::Layout,
    cell::{Cell, UnsafeCell},
    ffi::c_void,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicUsize, Ordering},
};

use alloc::boxed::Box;
use cordyceps::{Linked, MpscQueue, mpsc_queue::Links};

pub struct CancelationHandle<'a>(&'a AtomicPtr<AtomicBool>);

impl<'a> Drop for CancelationHandle<'a> {
    fn drop(&mut self) {
        let ptr = self.0.swap(core::ptr::null_mut(), Ordering::AcqRel);

        if ptr.addr() != !0 {
            while self.0.load(Ordering::Acquire).is_null() {
                core::hint::spin_loop();
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct JobBlock {
    // top bit is 1 if writing, 0 if reading
    fd_and_type: i32,
    uptr: *mut c_void,
    total_len: usize,
    offset: u64,
}

unsafe impl bytemuck::Zeroable for JobBlock {}

pub struct PendingJob {
    next_job: Links<Self>,
    // Lower 5 bits of address is the Job index which is never 31.
    processor_waker: *const AtomicPtr<AtomicBool>,
    job: JobBlock,
}

unsafe impl Send for PendingJob {}
unsafe impl Sync for PendingJob {}

unsafe impl Linked<Links<Self>> for PendingJob {
    type Handle = Box<Self>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        Box::into_non_null(r)
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        unsafe { Box::from_non_null(ptr) }
    }

    unsafe fn links(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Links<Self>> {
        unsafe { NonNull::new_unchecked(&raw mut (*ptr.as_ptr()).next_job) }
    }
}

pub struct IoJobBuffer {
    job_queue: MpscQueue<PendingJob>,
    active_jobs: [AtomicUsize; 31],
}

static STUB: PendingJob = PendingJob {
    next_job: Links::new_stub(),
    processor_waker: core::ptr::null(),
    job: bytemuck::zeroed(),
};

static BUFFER: IoJobBuffer = IoJobBuffer {
    job_queue: unsafe { MpscQueue::new_with_static_stub(&STUB) },
    active_jobs: [const { AtomicUsize::new(!0) }; 31],
};

#[derive(Copy, Clone)]
enum Status {
    Done,
    Waiting,
    Working,
}

#[repr(C, align(32))]
pub struct JobQuery {
    // This particular bool must be 32-byte aligned
    cancel_slot: AtomicBool,
    slot: u8,
    internal_status: Cell<Status>,
    current_job: UnsafeCell<JobBlock>,
}

impl JobQuery {
    pub fn process_one_job(&self) {
        loop {
            match self.internal_status.get() {
                Status::Waiting => {}
                Status::Working => {
                    if self.cancel_slot.load(Ordering::Acquire) {
                        self.cancel_slot.store(true, Ordering::Release);
                        self.internal_status.set(Status::Done);
                        return;
                    }
                }
                Status::Done => {
                    self.internal_status.set(Status::Waiting);
                    return;
                }
            }
        }
    }
}
