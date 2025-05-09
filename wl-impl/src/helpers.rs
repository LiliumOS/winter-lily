use core::ops::Range;
use core::{
    array::from_fn, cell::UnsafeCell, cmp::Ordering, ffi::c_void, iter::FusedIterator,
    marker::PhantomData, mem::MaybeUninit, num::NonZero, ptr::NonNull, sync::atomic::AtomicUsize,
};

use bytemuck::{NoUninit, Zeroable};
use lilium_sys::uuid::Uuid;
use lilium_sys::{
    result::{Error as SysError, Result as SysResult},
    sys::{
        kstr::{KCSlice, KSlice, KStrCPtr, KStrPtr},
        option::ExtendedOptionHead,
    },
};
use linux_errno::{EINTR, EPERM};
use linux_raw_sys::general::{SIGKILL, SIGQUIT, sigaction, sigset_t};
use linux_syscall::{SYS_getpid, SYS_kill, SYS_rt_sigaction, syscall};

mod rt_impls;

pub(crate) use rt_impls::__install_sa_handler;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(usize)]
pub enum AccessType {
    Read = 0,
    Write = 1,
    Overlap = 2,
}

unsafe impl Zeroable for AccessType {}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Zeroable)]
#[repr(C)]
pub struct CheckedAccessError {
    pub ptr: *const c_void,
    pub access_ty: AccessType,
}

impl From<CheckedAccessError> for SysError {
    fn from(value: CheckedAccessError) -> Self {
        SysError::InvalidMemory
    }
}

pub type CheckedAccessResult<T> = Result<T, CheckedAccessError>;

pub const STANDARD_OPTION_MASK: u32 = 0x0001;

pub fn validate_option_head(head: &ExtendedOptionHead, tbits: u16) -> SysResult<()> {
    let expected = ExtendedOptionHead {
        ty: head.ty,
        flags: head.flags,
        ..ExtendedOptionHead::ZERO
    };
    if !bytes_eq(head, &expected) {
        return Err(SysError::InvalidOption);
    }

    let flags_mask = STANDARD_OPTION_MASK | (tbits as u32) << 16;

    if (expected.flags & !flags_mask) != 0 {
        return Err(SysError::InvalidOption);
    }

    Ok(())
}

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
) -> CheckedAccessResult<()> {
    if (src..(src.wrapping_add(elems))).contains(&dest.cast_const()) {
        return Err(CheckedAccessError {
            ptr: dest.cast(),
            access_ty: AccessType::Overlap,
        });
    } else if (dest..(dest.wrapping_add(elems))).contains(&src.cast_mut()) {
        return Err(CheckedAccessError {
            ptr: src.cast_mut().cast(),
            access_ty: AccessType::Overlap,
        });
    }

    let mut err = CheckedAccessError::zeroed();

    if unsafe {
        rt_impls::checked_memcpy_impl(
            dest.cast(),
            src.cast(),
            elems * core::mem::size_of::<T>(),
            &mut err,
        )
    } < 0
    {
        Err(err)
    } else {
        Ok(())
    }
}

pub unsafe fn read_checked<T>(src: *const T) -> CheckedAccessResult<T> {
    let mut data = MaybeUninit::uninit();

    unsafe {
        copy_nonoverlapping_checked(src, data.as_mut_ptr(), 1)?;
    }

    Ok(unsafe { data.assume_init() })
}

pub unsafe fn write_checked<T>(dest: *mut T, val: T) -> CheckedAccessResult<()> {
    unsafe { copy_nonoverlapping_checked(&val, dest, 1) }
}

unsafe fn probe_checked<Ty: CheckedAccessType, T>(
    src: *mut T,
    count: usize,
) -> CheckedAccessResult<()> {
    if count == 0 {
        return Ok(());
    }
    let size = core::mem::size_of::<T>() * count;
    let mut ptr = src.cast::<u8>();

    let mut b = 0u8;

    for i in 0..((4095 + size) / 4096) {
        b = unsafe { read_checked(ptr)? };
        if const { Ty::DO_WRITEBACK } {
            unsafe {
                write_checked(ptr, b)?;
            }
        }
        ptr = unsafe { ptr.add(i * 4096) };
    }
    let ptr = unsafe { src.add(count).sub(1) }.cast::<u8>();
    b = unsafe { read_checked(ptr)? };
    if const { Ty::DO_WRITEBACK } {
        unsafe {
            write_checked(ptr, b)?;
        }
    }

    Ok(())
}

pub enum CheckUtfError {
    Access(CheckedAccessError),
    InvalidUtf8,
}

pub unsafe fn check_utf8<'a>(kstr: KStrCPtr) -> Result<&'a str, CheckUtfError> {
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
        unsafe {
            copy_nonoverlapping_checked(str_ptr, buf.as_mut_ptr(), tlen)
                .map_err(CheckUtfError::Access)?
        }

        for i in &buf[..tlen] {
            match *i {
                v @ (0x80..0xC0) => {
                    if cur_char_size != 0 {
                        cur_char_val <<= 6;
                        cur_char_val |= (v & 0x3F) as u32;
                        cur_char_size -= 1;
                        if cur_char_size == 0 {
                            if char::from_u32(cur_char_val)
                                .ok_or_else(|| CheckUtfError::InvalidUtf8)?
                                .len_utf8()
                                != cur_char_init_size
                            {
                                return Err(CheckUtfError::InvalidUtf8);
                            }
                        }
                    } else {
                        return Err(CheckUtfError::InvalidUtf8);
                    }
                }
                _ if cur_char_size != 0 => return Err(CheckUtfError::InvalidUtf8),
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
                _ => return Err(CheckUtfError::InvalidUtf8),
            }
        }

        len -= tlen;
        str_ptr = unsafe { str_ptr.add(tlen) };
    }

    unsafe {
        Ok(core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            kstr.str_ptr,
            kstr.len,
        )))
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

mod strexcept;

pub fn exit_unrecoverably(except: Option<Uuid>) -> ! {
    if let Some(except) = except {
        eprintln!(
            "Crashing with unhandled exception: {:#}",
            strexcept::strexcept(except)
        );
    }
    let pid = unsafe { syscall!(SYS_getpid).as_u64_unchecked() };
    unsafe {
        let _ = syscall!(
            SYS_rt_sigaction,
            SIGQUIT,
            &sigaction {
                sa_handler: linux_raw_sys::signal_macros::SIG_DFL,
                sa_mask: core::mem::zeroed(),
                sa_flags: 0,
                sa_restorer: None,
            },
            core::ptr::null_mut::<sigaction>(),
            core::mem::size_of::<linux_raw_sys::general::sigset_t>()
        );

        let _ = syscall!(SYS_kill, pid, SIGQUIT);
    }
    loop {
        unsafe {
            // This never returns
            let _ = syscall!(SYS_kill, pid, SIGKILL);
        } // Support dumping core later
    }
}

pub fn bytes_eq<T: NoUninit>(v: &T, u: &T) -> bool {
    bytemuck::bytes_of(v) == bytemuck::bytes_of(u)
}

pub fn bytes_cmp<T: NoUninit>(v: &T, u: &T) -> Ordering {
    core::cmp::Ord::cmp(bytemuck::bytes_of(v), bytemuck::bytes_of(u))
}

trait CheckedAccessType {
    const DO_WRITEBACK: bool = false;
}

struct Read;
struct Write;

impl CheckedAccessType for Read {}
impl CheckedAccessType for Write {
    const DO_WRITEBACK: bool = true;
}

struct RawCheckedSliceIter<T, Ty> {
    pos: NonNull<T>,
    end: NonNull<T>,
    ty: PhantomData<Ty>,
}

impl<T, Ty> Clone for RawCheckedSliceIter<T, Ty> {
    fn clone(&self) -> Self {
        Self {
            pos: self.pos,
            end: self.end,
            ty: PhantomData,
        }
    }
}

impl<T, Ty> RawCheckedSliceIter<T, Ty> {
    pub unsafe fn new_unchecked(base: NonNull<T>, len: usize) -> Self {
        const {
            if core::mem::size_of::<T>() == 0 {
                panic!(
                    "`CheckedSliceIter` cannot be used with a slice of element size 0 (instead it's safe to use a nor slice iterator)"
                )
            }
        }

        let pos = base;
        let end = unsafe { pos.map_addr(|n| n.unchecked_add(len * core::mem::size_of::<usize>())) };

        Self {
            pos,
            end,
            ty: PhantomData,
        }
    }
}
impl<T, Ty: CheckedAccessType> Iterator for RawCheckedSliceIter<T, Ty> {
    type Item = CheckedAccessResult<NonNull<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.pos;

        if self.pos == self.end {
            return None;
        }

        self.pos = unsafe { pos.map_addr(|n| n.unchecked_add(core::mem::size_of::<usize>())) };

        Some((unsafe { probe_checked::<Ty, T>(pos.as_ptr(), 1) }).map(|_| pos))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }

    fn count(self) -> usize {
        self.len()
    }

    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let len = self.len();
        if n <= len {
            self.pos = unsafe {
                self.pos
                    .map_addr(|a| a.unchecked_add(n * core::mem::size_of::<usize>()))
            };
            Ok(())
        } else {
            Err(unsafe { NonZero::new_unchecked(n - len) })
        }
    }

    fn next_chunk<const N: usize>(
        &mut self,
    ) -> Result<[Self::Item; N], core::array::IntoIter<Self::Item, N>> {
        let pos = self.pos;
        if self.len() >= N {
            self.pos =
                unsafe { pos.map_addr(|n| n.unchecked_add(N * core::mem::size_of::<usize>())) };

            Ok((unsafe { probe_checked::<Ty, T>(pos.as_ptr(), N) })
                .map(|_| core::array::from_fn(|i| Ok(unsafe { pos.add(i) })))
                .map_err(|e| core::array::from_fn(|_| Err(e)))
                .map_or_else(|e| e, |e| e))
        } else {
            let len = self.len();

            self.pos = self.end;
            let arr = (unsafe { probe_checked::<Ty, T>(pos.as_ptr(), len) })
                .map(|_| {
                    core::array::from_fn(|i| {
                        if i < len {
                            MaybeUninit::new(Ok(unsafe { pos.add(i) }))
                        } else {
                            MaybeUninit::uninit()
                        }
                    })
                })
                .map_err(|e| core::array::from_fn(|_| Err(e)))
                .map_or_else(|e| e.map(MaybeUninit::new), |e| e);

            Err(unsafe { core::array::IntoIter::new_unchecked(arr, 0..len) })
        }
    }
}

impl<T, Ty: CheckedAccessType> ExactSizeIterator for RawCheckedSliceIter<T, Ty> {
    fn len(&self) -> usize {
        let addr = self.pos.addr().get();
        let eaddr = self.end.addr().get();

        unsafe { eaddr.unchecked_sub(addr) / core::mem::size_of::<T>() }
    }

    fn is_empty(&self) -> bool {
        self.pos == self.end
    }
}

impl<T, Ty: CheckedAccessType> DoubleEndedIterator for RawCheckedSliceIter<T, Ty> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.pos == self.end {
            return None;
        }

        let pos = unsafe { self.end.sub(1) };

        self.end = pos;

        Some((unsafe { probe_checked::<Ty, T>(pos.as_ptr(), 1) }).map(|_| pos))
    }
}

impl<T, Ty: CheckedAccessType> FusedIterator for RawCheckedSliceIter<T, Ty> {}

pub struct CheckedSliceIter<'a, T>(RawCheckedSliceIter<T, Read>, PhantomData<&'a [T]>);

impl<'a, T> CheckedSliceIter<'a, T> {
    pub unsafe fn from_kslice_unchecked(kslice: KCSlice<T>) -> Self {
        let ptr = kslice.arr_ptr;
        let len = kslice.len;

        Self(
            unsafe {
                RawCheckedSliceIter::new_unchecked(NonNull::new_unchecked(ptr.cast_mut()), len)
            },
            PhantomData,
        )
    }
}

impl<'a, T> Clone for CheckedSliceIter<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<'a, T> Iterator for CheckedSliceIter<'a, T> {
    type Item = CheckedAccessResult<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| v.map(|v| unsafe { v.as_ref() }))
    }

    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.count()
    }
}

impl<'a, T> ExactSizeIterator for CheckedSliceIter<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a, T> DoubleEndedIterator for CheckedSliceIter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|v| v.map(|v| unsafe { v.as_ref() }))
    }

    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_back_by(n)
    }
}

impl<'a, T> FusedIterator for CheckedSliceIter<'a, T> {}

pub struct CheckedSliceIterMut<'a, T>(RawCheckedSliceIter<T, Read>, PhantomData<&'a mut [T]>);

impl<'a, T> CheckedSliceIterMut<'a, T> {
    pub unsafe fn from_kslice_unchecked(kslice: KSlice<T>) -> Self {
        let ptr = kslice.arr_ptr;
        let len = kslice.len;

        Self(
            unsafe { RawCheckedSliceIter::new_unchecked(NonNull::new_unchecked(ptr), len) },
            PhantomData,
        )
    }
}

impl<'a, T> Iterator for CheckedSliceIterMut<'a, T> {
    type Item = CheckedAccessResult<&'a mut T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| v.map(|mut v| unsafe { v.as_mut() }))
    }

    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.count()
    }
}

impl<'a, T> ExactSizeIterator for CheckedSliceIterMut<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a, T> DoubleEndedIterator for CheckedSliceIterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|v| v.map(|mut v| unsafe { v.as_mut() }))
    }

    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_back_by(n)
    }
}

impl<'a, T> FusedIterator for CheckedSliceIterMut<'a, T> {}

pub unsafe fn iter_checked<'a, T>(slice: KCSlice<T>) -> CheckedSliceIter<'a, T> {
    unsafe { CheckedSliceIter::from_kslice_unchecked(slice) }
}

pub unsafe fn iter_mut_checked<'a, T>(slice: KSlice<T>) -> CheckedSliceIterMut<'a, T> {
    unsafe { CheckedSliceIterMut::from_kslice_unchecked(slice) }
}

pub unsafe fn fill_str(kstr: &mut KStrPtr, st: &str) -> SysResult<()> {
    let len = kstr.len.min(st.len());

    unsafe {
        copy_nonoverlapping_checked(st.as_ptr(), kstr.str_ptr, len)?;
    }

    if core::mem::replace(&mut kstr.len, len) < st.len() {
        Err(SysError::InsufficientLength)
    } else {
        Ok(())
    }
}

pub const fn const_parse_u32(v: &str, radix: u32) -> u32 {
    if radix < 2 || radix > 36 {
        panic!("Invalid radix")
    }

    let mut val = 0u32;

    let b = v.as_bytes();

    let mut i = 0;

    while i < b.len() {
        let b = b[i];

        let d = match b {
            b'0'..=b'9' => b - b'0',
            b'A'..=b'Z' => (b - b'A') + 10,
            b'a'..=b'z' => (b - b'a') + 10,
            _ => panic!("Expected a digit"),
        } as u32;

        if d < radix {
            let v = match val.checked_mul(radix) {
                Some(v) => v.checked_add(d),
                None => None,
            };

            match v {
                Some(v) => val = v,
                None => panic!("Out of Range value"),
            }
        } else {
            panic!("Out of Range digit")
        }
        i += 1;
    }
    val
}

pub use wl_helpers::*;

use crate::eprintln;

pub fn linux_error_to_lilium(errno: linux_errno::Error) -> lilium_sys::result::Error {
    match errno {
        EINTR => lilium_sys::result::Error::Interrupted,
        EPERM => lilium_sys::result::Error::Permission,
        _ => lilium_sys::result::Error::from_code(-0x800).unwrap_err(), // Error 8:0 in winter-lily is the winter-lily unknown error
    }
}
