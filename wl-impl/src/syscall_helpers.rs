use bytemuck::Pod;
use lilium_sys::{result::Result, sys::result::SysResult};

#[cfg(target_arch = "x86_64")]
pub type SysCallTyErased = unsafe extern "sysv64" fn(core::convert::Infallible);

#[macro_export]
#[cfg(target_arch = "x86_64")]
macro_rules! export_syscall {
    // (unsafe extern fn $name:ident ($(|$ctx:pat_param|)? $($params:ident : $param_ty:ty),* , $va_name:ident: ...) -> $ret_ty:ty $body:block) => {
    //     #[unsafe(no_mangle)]
    //     #[allow(unreachable_code)]
    //     pub unsafe extern "sysv64" fn $name ($($params: $param_ty),*, $va_name: ...) -> <$ret_ty as $crate::syscall_helpers::SyscallRet>::Sys {
    //         let ret_val: $ret_ty = (move || -> $ret_ty {$body})();

    //         $crate::syscall_helpers::SyscallRet::into_sys(ret_val)
    //     }
    // };
    (unsafe extern fn $name:ident ($(|$ctx:pat_param|)? $($params:ident : $param_ty:ty),* $(,)?) -> $ret_ty:ty $body:block) => {
        #[unsafe(no_mangle)]
        #[allow(unreachable_code)]
        pub unsafe extern "sysv64" fn $name ($($params: $param_ty),*) -> <$ret_ty as $crate::syscall_helpers::SyscallRet>::Sys {
            let ret_val: $ret_ty = (move || -> $ret_ty {$body})();

            $crate::syscall_helpers::SyscallRet::into_sys(ret_val)
        }
    };
}

pub trait SyscallRet {
    type Sys: Copy + Clone;

    fn into_sys(self) -> Self::Sys;
}

impl SyscallRet for ! {
    type Sys = !;

    fn into_sys(self) -> Self::Sys {
        self
    }
}

impl SyscallRet for Result<()> {
    type Sys = SysResult;

    fn into_sys(self) -> Self::Sys {
        match self {
            Ok(()) => 0,
            Err(e) => e.into_code(),
        }
    }
}

impl SyscallRet for Result<usize> {
    type Sys = SysResult;

    fn into_sys(self) -> Self::Sys {
        match self {
            Ok(val) => val as _,
            Err(e) => e.into_code(),
        }
    }
}

#[doc(hidden)]
pub use core as _core;
