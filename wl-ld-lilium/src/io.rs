use std::{io::BufRead, str::Utf8Error};

use linux_errno::Error as SysError;
use linux_syscall::{Result as _, SYS_lseek, SYS_read, syscall};

use crate::helpers::copy_to_slice_head;

pub fn linux_err_into_io_err(e: SysError) -> std::io::Error {
    std::io::Error::from_raw_os_error(e.get() as i32)
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
    pub fn read_line_static<'a>(
        &mut self,
        n: &'a mut [u8],
    ) -> std::io::Result<Option<&'a mut str>> {
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

impl std::io::Seek for BufFdReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.pos = self.len;

        let (whence, pos) = match pos {
            std::io::SeekFrom::Start(n) => (libc::SEEK_SET, n as libc::off_t),
            std::io::SeekFrom::End(n) => (libc::SEEK_END, n as libc::off_t),
            std::io::SeekFrom::Current(n) => (libc::SEEK_CUR, n as libc::off_t),
        };

        let res = unsafe { syscall!(SYS_lseek, self.fd, pos, whence) };

        res.check().map_err(linux_err_into_io_err)?;

        Ok(res.as_u64_unchecked())
    }
}

impl std::io::Read for BufFdReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let this_buf = self.fill_buf()?;

        let len = buf.len().min(this_buf.len());

        copy_to_slice_head(&mut buf[..len], &this_buf[..len]);

        self.consume(len);
        Ok(len)
    }
}

impl std::io::BufRead for BufFdReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos == self.len {
            let res = unsafe { syscall!(SYS_read, self.fd, self.buf.as_ptr(), self.buf.len()) };

            res.check().map_err(linux_err_into_io_err)?;

            self.len = res.as_usize_unchecked();
            self.pos = 0;
        }

        Ok(self.buf())
    }

    fn consume(&mut self, amt: usize) {
        self.pos += amt;
        if self.pos > self.len {
            panic!()
        }
    }
}
