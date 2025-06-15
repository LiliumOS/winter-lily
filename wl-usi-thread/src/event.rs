use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use lilium_sys::{
    result::{Error, Result},
    sys::{
        event::{AwaitAddrOption, NotifyAddressOption},
        kstr::KCSlice,
    },
};
use rustix::thread::futex::{self, Flags};
use wl_impl::{export_syscall, helpers::linux_error_to_lilium};

export_syscall! {
    unsafe extern fn AwaitAddress(addr: *mut usize, current: *mut usize, ignore_mask: usize, options: KCSlice<AwaitAddrOption>) -> Result<()> {
        todo!()
    }
}

export_syscall! {
    unsafe extern fn NotifyAddress(addr: *mut usize, count: usize, wake_mask: usize, options: KCSlice<NotifyAddressOption>) -> Result<usize> {
        todo!()
    }
}
