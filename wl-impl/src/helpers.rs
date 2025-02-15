use core::ops::Range;
use std::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::AtomicUsize};

use libc::{sigaction, sigset_t};
use lilium_sys::{
    result::{Error, Result},
    sys::kstr::KStrCPtr,
};

mod rt_impls;

/// Converts a slice pointer to a range of element pointers.
///
/// # Safety
/// `p` must point to a slice that is entirely contained within the same allocation.
pub unsafe fn as_ptr_range<T>(p: *const [T]) -> Range<*const T> {
    let len = p.len();
    let base = p.cast::<T>();
    let end = unsafe { base.add(len) };

    Range { start: base, end }
}

/// Converts a slice pointer to a range of element pointers.
///
/// # Safety
/// `p` must point to a slice that is entirely contained within the same allocation.
pub unsafe fn as_ptr_range_mut<T>(p: *mut [T]) -> Range<*mut T> {
    let len = p.len();
    let base = p.cast::<T>();
    let end = unsafe { base.add(len) };

    Range { start: base, end }
}

/// Copies `elems` `T`s from `src` to `dest`, checking if it violates memory access permissions.
///
/// # Safety
/// This will return an error if it accesses any unmapped memory. However, it does not check boundaries of rust objects.
/// Accesses must still not access memory that isn't available to the arguments.
pub unsafe fn copy_nonoverlapping_checked<T>(
    src: *const T,
    dest: *mut T,
    elems: usize,
) -> Result<()> {
    if (src..(src.wrapping_add(elems))).contains(&dest.cast_const())
        || (dest..(dest.wrapping_add(elems))).contains(&src.cast_mut())
    {
        return Err(Error::InvalidMemory);
    }

    unsafe {
        Error::from_code(rt_impls::__checked_memcpy_impl(
            dest.cast(),
            src.cast(),
            elems * core::mem::size_of::<T>(),
        ))
    }
}

pub unsafe fn read_checked<T>(src: *const T) -> Result<T> {
    let mut data = MaybeUninit::uninit();

    unsafe {
        copy_nonoverlapping_checked(src, data.as_mut_ptr(), 1)?;
    }

    Ok(unsafe { data.assume_init() })
}

pub unsafe fn write_checked<T>(dest: *mut T, val: T) -> Result<()> {
    unsafe { copy_nonoverlapping_checked(&val, dest, 1) }
}

pub unsafe fn check_utf8<'a>(kstr: KStrCPtr) -> Result<&'a str> {
    let KStrCPtr {
        mut str_ptr,
        mut len,
    } = kstr;

    let mut cur_char_size = 0;
    let mut cur_char_init_size = 0;
    let mut cur_char_val = 0;
    while len > 0 {
        let mut buf = [0u8; 4 * core::mem::size_of::<usize>()];
        let tlen = len.min(buf.len());
        unsafe { copy_nonoverlapping_checked(str_ptr, buf.as_mut_ptr(), tlen)? }

        for i in &buf[..tlen] {
            match *i {
                v @ (0x80..0xC0) => {
                    if cur_char_size != 0 {
                        cur_char_val <<= 6;
                        cur_char_val |= (v & 0x3F) as u32;
                        cur_char_size -= 1;
                        if cur_char_size == 0 {
                            if char::from_u32(cur_char_val)
                                .ok_or_else(|| Error::InvalidString)?
                                .len_utf8()
                                != cur_char_init_size
                            {
                                return Err(Error::InvalidString);
                            }
                        }
                    } else {
                        return Err(Error::InvalidString);
                    }
                }
                _ if cur_char_size != 0 => return Err(Error::InvalidString),
                0x00..0x80 => {}
                v @ (0xC0..0xE0) => {
                    cur_char_size = 1;
                    cur_char_init_size = 2;
                    cur_char_val = (v & 0x1F) as u32;
                }
                v @ (0xE0..0xF0) => {
                    cur_char_size = 2;
                    cur_char_init_size = 3;
                    cur_char_val = (v & 0xF) as u32;
                }
                v @ (0xF0..0xF8) => {
                    cur_char_size = 3;
                    cur_char_init_size = 4;
                    cur_char_val = (v & 0x7) as u32;
                }
                _ => return Err(Error::InvalidString),
            }
        }

        len -= tlen;
        str_ptr = unsafe { str_ptr.add(tlen) };
    }

    unsafe {
        core::str::from_utf8(core::slice::from_raw_parts(kstr.str_ptr, kstr.len))
            .map_err(|_| Error::InvalidString)
    }
}

pub const fn insert_elems<T: Copy, const N: usize, const R: usize>(
    mut base: [T; N],
    updates: [(usize, T); R],
) -> [T; N] {
    const {
        assert!(R <= N, "Cannot specify more updates than original elements");
    }
    let mut i = 0;
    while i < R {
        let (idx, val) = updates[i];
        i += 1;

        base[idx] = val;
    }

    base
}

pub fn exit_unrecoverably() -> ! {
    unsafe {
        libc::sigaction(
            libc::SIGQUIT,
            &sigaction {
                sa_sigaction: libc::SIG_DFL,
                sa_mask: core::mem::zeroed(),
                sa_flags: 0,
                sa_restorer: None,
            },
            core::ptr::null_mut(),
        );
        libc::raise(libc::SIGQUIT);
    }
    loop {
        unsafe {
            libc::raise(9);
        } // Support dumping core later
    }
}
