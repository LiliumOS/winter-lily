use core::{
    cell::{Cell, UnsafeCell},
    ffi::c_char,
    mem::MaybeUninit,
};

use alloc::{ffi::CString, string::ToString, vec::Vec};

use rustix::{
    fd::{AsFd, BorrowedFd, IntoRawFd},
    net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg, sendmsg,
    },
    pipe::PipeFlags,
    process::{PidfdFlags, WaitId, WaitIdOptions, getpid, pidfd_open, waitid},
    thread::Pid,
};
use wl_impl::{
    catch_signals::sig_to_except,
    eprintln, export_syscall,
    handle_base::{Handle, insert_handle},
    helpers::{exit_unrecoverably, linux_error_to_lilium},
    libc::{EINVAL, Error, close, execve, exit_group, fork},
    ministd::AsRawFd,
    println,
};

use lilium_sys::{
    result::{Error as LiliumError, Result},
    sys::{
        except::{self, ExceptionStatusInfo},
        fs::FileHandle,
        handle::{self, HANDLE_TYPE_PROC, HandlePtr},
        kstr::{KCSlice, KStrCPtr},
        option::OPTION_FLAG_IGNORE,
        process::{self as sys, CreateProcessOption, ProcessHandle},
        thread::{JoinStatus, JoinStatusExit},
    },
};

unsafe extern "C" {
    safe static __environ: Cell<*const *const c_char>;
}

#[repr(C, align(16))]
struct Align16<T>(T);

export_syscall! {
    unsafe extern fn CreateProcess(hdl_out: *mut HandlePtr<ProcessHandle>, resolution_base: HandlePtr<FileHandle>, path: *const KStrCPtr, options: *const KCSlice<CreateProcessOption>) -> Result<()> {

        let path = unsafe { (*path).as_str()};

        let exec_path = if resolution_base == HandlePtr::null() || path.starts_with('/') {
            unsafe { path.to_string() }
        } else {
            let fhdl = unsafe { Handle::try_deref(resolution_base.cast())? };
            fhdl.check_type(handle::HANDLE_SUBTYPE_IO_FILE as usize, 0)?;

            let fd = fhdl.borrow_fd()
                .ok_or(LiliumError::UnsupportedOperation)?
                .as_raw_fd();

            alloc::format!("/proc/self/fd/{fd}/{path}")
        };

        let exec_path = CString::new(exec_path).map_err(|_| LiliumError::InvalidString)?;

        let mut args = Vec::new();

        let mut args_specified = false;

        for opt in unsafe{ (*options).as_slice() } {
            match unsafe { opt.head.ty } {
                sys::CREATE_PROCESS_OPTION_ARGS => {
                    let provided_args = unsafe {&opt.args};
                    args_specified = true;

                    for arg in unsafe { provided_args.arguments.as_slice() } {
                        args.push(CString::new(unsafe { arg.as_str() }).map_err(|_| LiliumError::InvalidString)?)
                    }
                }
                _ => {
                    if (unsafe { opt.head.flags } & OPTION_FLAG_IGNORE) == 0 {
                        return Err(LiliumError::InvalidOption)
                    }
                }
            }
        }

        if !args_specified {
            args.push(CString::new(path).unwrap())
        }


        let mut argv = args.iter()
            .map(|v| v.as_ptr())
            .collect::<Vec<_>>();
        argv.push(core::ptr::null());

        let (read, write) = rustix::net::socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, SocketFlags::CLOEXEC, None)
            .unwrap();
        let _ = rustix::net::shutdown(&read, rustix::net::Shutdown::Write);
        let _ = rustix::net::shutdown(&write, rustix::net::Shutdown::Read);
        const SPACE_NEEDED: usize = rustix::cmsg_space!(ScmRights(1));
        let mut buf = Align16([const { MaybeUninit::uninit() }; SPACE_NEEDED]);

        let hdl = Handle {ty: HANDLE_TYPE_PROC as usize, blob1: core::ptr::null_mut(), blob2: core::ptr::null_mut(), fd: -1};

        let ptr = insert_handle(hdl)?;

        let mut hdl = unsafe { Handle::deref_unchecked(ptr) };
        match unsafe { fork() } {
            Ok(0) => {
                let fd = pidfd_open(getpid(), PidfdFlags::empty()).unwrap_or_else(|e| { let _ = rustix::io::write(&write, bytemuck::bytes_of(&(e.raw_os_error() as u16))); exit_unrecoverably(None)});
                let msg = SendAncillaryMessage::ScmRights(&[fd.as_fd()]);
                let mut buf = SendAncillaryBuffer::new(&mut buf.0);
                if !buf.push(msg) {
                    let _ = rustix::io::write(&write, bytemuck::bytes_of(&EINVAL.get()));
                    exit_unrecoverably(None)
                }
                sendmsg(&write, &[], &mut buf, SendFlags::empty()).unwrap_or_else(|e| { let _ = rustix::io::write(&write, bytemuck::bytes_of(&(e.raw_os_error() as u16))); exit_unrecoverably(None)});
                let err = unsafe { execve(exec_path.as_ptr(), argv.as_ptr(), __environ.get())}.into_err();

                let errno = err.get();

                let _ = rustix::io::write(write, bytemuck::bytes_of(&errno));
                exit_unrecoverably(None)
            }
            Ok(pid) => {
                let mut n = 0u16;
                let mut buf = RecvAncillaryBuffer::new(&mut buf.0);
                let mut waittarg = WaitId::Pid(unsafe { Pid::from_raw_unchecked(pid) });
                let _ = recvmsg(&read, &mut [], &mut buf, RecvFlags::CMSG_CLOEXEC)
                    .map_err(|e| {hdl.close(false); let _ = waitid(waittarg.clone(), WaitIdOptions::EXITED); linux_error_to_lilium(unsafe { Error::new_unchecked(e.raw_os_error() as u16) })})?;

                let msg = buf.drain().next();


                let pidfd = match msg {
                    Some(RecvAncillaryMessage::ScmRights(mut fd)) => {
                        let pidfd = fd.next().unwrap();
                        pidfd
                    },
                    _ => {hdl.close(false); let _ = waitid(waittarg, WaitIdOptions::EXITED); lilium_sys::result::Error::from_code(-0x802)?; unreachable!()}
                };
                let rawfd = pidfd.into_raw_fd();
                waittarg = WaitId::PidFd(unsafe { BorrowedFd::borrow_raw(rawfd)});
                hdl.fd = rawfd as i64;
                hdl.blob2 = core::ptr::without_provenance_mut(pid as usize);
                drop(write);
                match rustix::io::read(read, bytemuck::bytes_of_mut(&mut n)) {
                    Ok(1..) => {
                        hdl.close(false);
                        let _ = waitid(waittarg, WaitIdOptions::EXITED);
                        return Err(Error::new(n).map_or(lilium_sys::result::Error::ResourceLimitExhausted, linux_error_to_lilium))
                    }
                    Ok(0) | Err(_) => {

                        unsafe { hdl_out.write(ptr.cast()); }
                        Ok(())
                    }
                }
            }
            Err(e) => {
                Err(linux_error_to_lilium(e))
            }
        }

    }
}

export_syscall! {
    unsafe extern fn JoinProcess(hdl: HandlePtr<ProcessHandle>, status_out: *mut JoinStatus) -> Result<()> {
        let mut hdl = unsafe { Handle::try_deref(hdl.cast())? };

        hdl.check_type(HANDLE_TYPE_PROC as usize, 0)?;



        let fd = hdl.borrow_fd().unwrap();

        let status = waitid(WaitId::PidFd(fd), WaitIdOptions::EXITED)
            .map_err(|e| linux_error_to_lilium(unsafe { Error::new_unchecked(e.raw_os_error() as u16) }))?
            .unwrap();
        hdl.close(false);
        let status = if let Some(sig) = status.terminating_signal() {
            let except = sig_to_except(sig as u32);
            if let Some(except) = except {
                JoinStatus{exit_exception: ExceptionStatusInfo{except_code: except, except_info: 0, except_reason: 0}}
            } else {
                JoinStatus{exit_code: JoinStatusExit { discriminant: !0, ..bytemuck::zeroed() }}
            }
        } else {
            let status = status.exit_status().unwrap();
            JoinStatus{exit_code: JoinStatusExit{exit_code: status as u64, ..bytemuck::zeroed()}}
        };

        unsafe { status_out.write(status); }

        Ok(())
    }
}
