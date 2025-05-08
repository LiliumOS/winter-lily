use core::{
    cell::{Cell, OnceCell, UnsafeCell},
    ffi::c_void,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::atomic::{AtomicPtr, AtomicU32},
};

use alloc::{boxed::Box, sync::Arc};
use alloc::{string::String, sync::Weak};
use bytemuck::zeroed;
use lilium_sys::sys::thread::{JoinStatus, JoinStatusExit};
use linux_raw_sys::general::__kernel_pid_t;
use rustix::{
    fd::{AsFd, FromRawFd, OwnedFd},
    process::{WaitId, WaitIdOptions, WaitIdStatus, WaitStatus, waitid},
};
use wl_helpers::OnceLock;

use crate::libc::{__rtld_get_thread_ptr, Result, getpid, pidfd_open};

pub enum ThreadKind {
    Winter,
    User,
}

pub struct ThreadInfo {
    tid: *const c_void,
    pid: __kernel_pid_t,
    pfd: OwnedFd,
    tptr: AtomicPtr<c_void>,
    wake_addr: AtomicU32,
    name: String,
    tkind: ThreadKind,
    exit_status: Cell<JoinStatus>,
}

unsafe impl Sync for ThreadInfo {}

#[thread_local]
static TH_INFO: OnceCell<Arc<ThreadInfo>> = OnceCell::new();

pub struct Thread {
    inner: Arc<ThreadInfo>,
}

impl Thread {
    pub fn name(&self) -> &str {
        &self.inner.name
    }
}

pub struct ThreadJoinResult(JoinStatus);

impl ThreadJoinResult {
    pub fn raw_status(&self) -> &JoinStatus {
        &self.0
    }
    pub fn into_raw(self) -> JoinStatus {
        self.0
    }
}

pub struct JoinHandle(Thread);

impl JoinHandle {
    pub fn thread(&self) -> &Thread {
        &self.0
    }

    pub fn join(self) -> Result<ThreadJoinResult> {
        waitid(
            WaitId::PidFd(self.0.inner.pfd.as_fd()),
            WaitIdOptions::EXITED,
        )
        .map_err(|e| unsafe { linux_errno::Error::new_unchecked(e.raw_os_error() as u16) })?;

        Ok(ThreadJoinResult(self.0.inner.exit_status.get()))
    }
}

impl Drop for JoinHandle {
    fn drop(&mut self) {
        waitid(
            WaitId::PidFd(self.0.inner.pfd.as_fd()),
            WaitIdOptions::EXITED,
        )
        .unwrap();
    }
}

pub(crate) fn __setup_init_thread() -> Result<Thread> {
    let a = Arc::clone(TH_INFO.get_or_try_init(|| {
        let pid = unsafe { getpid() }?;
        let pidfd = unsafe { pidfd_open(pid, 0) }?;

        let pfd = unsafe { OwnedFd::from_raw_fd(pidfd) };

        Ok(Arc::new_cyclic(|v: &Weak<ThreadInfo>| {
            let tid = v.as_ptr().cast::<c_void>();

            ThreadInfo {
                tid,
                pid,
                pfd,
                tptr: AtomicPtr::new(__rtld_get_thread_ptr()),
                wake_addr: AtomicU32::new(0),
                name: alloc::format!("Main Thread"),
                exit_status: Cell::new(zeroed()),
                tkind: ThreadKind::User,
            }
        }))
    })?);

    Ok(Thread { inner: a })
}
