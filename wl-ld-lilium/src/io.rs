use core::str::Utf8Error;

pub use linux_errno::Error;
use linux_errno::{EINTR, ENODATA};
use linux_raw_sys::general::{__kernel_off_t, STDERR_FILENO, STDOUT_FILENO};
use linux_syscall::{Result as _, SYS_lseek, SYS_read, SYS_write, syscall};

use crate::helpers::copy_to_slice_head;

pub type Result<T> = core::result::Result<T, Error>;

pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

#[derive(Debug)]
pub struct BufFdReader {
    fd: i32,
    buf: [u8; 128],
    pos: usize,
    len: usize,
}

impl BufFdReader {
    pub fn new(fd: i32) -> Self {
        Self {
            fd,
            buf: [0u8; 128],
            pos: 0,
            len: 0,
        }
    }

    pub fn buf(&self) -> &[u8] {
        &self.buf[self.pos..self.len]
    }
}

impl BufFdReader {
    pub fn read_line_static<'a>(&mut self, n: &'a mut [u8]) -> Result<Option<&'a mut str>> {
        let mut pos = 0;
        loop {
            let (_, tail) = n.split_at_mut(pos);
            let buf = self.fill_buf()?;

            if buf.len() == 0 {
                return Ok(None);
            }

            let (found, max_len) = match buf.iter().enumerate().find(|(_, p)| **p == b'\n') {
                Some((idx, _)) => (true, idx),
                None => (false, buf.len()),
            };

            let real_len = max_len.min(tail.len());

            copy_to_slice_head(tail, &buf[..real_len]);
            self.consume(real_len + (found as usize));
            pos += real_len;

            if tail.len() < max_len || found {
                break;
            }
        }

        let slice = &mut n[..pos];

        Ok(Some(unsafe { core::str::from_utf8_unchecked_mut(slice) }))
    }
}

impl BufFdReader {
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.pos = self.len;

        let (whence, pos) = match pos {
            SeekFrom::Start(n) => (linux_raw_sys::general::SEEK_SET, n as __kernel_off_t),
            SeekFrom::End(n) => (linux_raw_sys::general::SEEK_END, n as __kernel_off_t),
            SeekFrom::Current(n) => (linux_raw_sys::general::SEEK_CUR, n as __kernel_off_t),
        };

        let res = unsafe { syscall!(SYS_lseek, self.fd, pos, whence) };

        res.check()?;

        Ok(res.as_u64_unchecked())
    }
}

impl BufFdReader {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let this_buf = self.fill_buf()?;

        let len = buf.len().min(this_buf.len());

        copy_to_slice_head(&mut buf[..len], &this_buf[..len]);

        self.consume(len);
        Ok(len)
    }

    pub fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => return Err(ENODATA),
                Ok(n) => buf = &mut buf[n..],
                Err(EINTR) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl BufFdReader {
    pub fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos == self.len {
            let res = unsafe { syscall!(SYS_read, self.fd, self.buf.as_ptr(), self.buf.len()) };

            res.check()?;

            self.len = res.as_usize_unchecked();
            self.pos = 0;
        }

        Ok(self.buf())
    }

    pub fn consume(&mut self, amt: usize) {
        self.pos += amt;
        if self.pos > self.len {
            panic!()
        }
    }
}

pub struct FdFormatter(u32);

impl core::fmt::Write for FdFormatter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let res = unsafe { syscall!(SYS_write, self.0, s.as_ptr(), s.len()) };

        res.check().map_err(|_| core::fmt::Error)
    }
}

pub const STDOUT: FdFormatter = FdFormatter(STDOUT_FILENO);
pub const STDERR: FdFormatter = FdFormatter(STDERR_FILENO);
