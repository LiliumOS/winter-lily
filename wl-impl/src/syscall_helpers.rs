use bytemuck::Pod;
use lilium_sys::{
    misc::MaybeValid,
    result::Result,
    sys::{
        handle::HandlePtr,
        kstr::{KCSlice, KSlice, KStrCPtr},
        result::SysResult,
    },
    uuid::Uuid,
};

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

        const _: () = {
            pub const fn __test_param<__T: $crate::syscall_helpers::SyscallParam>() {}
            pub fn __test_params() {
                $(__test_param::<$param_ty>();)*
            }
        };
    };
}

pub trait SyscallRet {
    type Sys: Copy + Clone + SysretTy;

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

pub unsafe trait SysretTy {}

unsafe impl SysretTy for usize {}
unsafe impl SysretTy for isize {}
unsafe impl SysretTy for u32 {}
unsafe impl SysretTy for i32 {}
#[cfg(not(target_pointer_width = "32"))]
unsafe impl SysretTy for u64 {}
#[cfg(not(target_pointer_width = "32"))]
unsafe impl SysretTy for i64 {}

unsafe impl SysretTy for () {}
unsafe impl SysretTy for ! {}

pub unsafe trait SyscallParam {}

unsafe impl SyscallParam for usize {}
unsafe impl SyscallParam for isize {}
unsafe impl SyscallParam for u32 {}
unsafe impl SyscallParam for i32 {}
#[cfg(not(target_pointer_width = "32"))]
unsafe impl SyscallParam for u64 {}
#[cfg(not(target_pointer_width = "32"))]
unsafe impl SyscallParam for i64 {}

unsafe impl<H> SyscallParam for HandlePtr<H> {}
unsafe impl<T> SyscallParam for *mut T {}
unsafe impl<T> SyscallParam for *const T {}

unsafe impl SyscallParam for Uuid {}
unsafe impl<T> SyscallParam for KCSlice<T> {}
unsafe impl<T> SyscallParam for KSlice<T> {}
unsafe impl SyscallParam for KStrCPtr {}

unsafe impl<T: SyscallParam> SyscallParam for MaybeValid<T> {}

macro_rules! def_fn_tys {
    ($($ty:ident),*) => {
        unsafe impl<__R: SysretTy, $($ty: SyscallParam),*> SyscallParam for extern "system" fn($($ty),*)->__R{}
        unsafe impl<__R: SysretTy, $($ty: SyscallParam),*> SyscallParam for extern "system-unwind" fn($($ty),*)->__R{}
        unsafe impl<__R: SysretTy, $($ty: SyscallParam),*> SyscallParam for Option<extern "system" fn($($ty),*)->__R>{}
        unsafe impl<__R: SysretTy, $($ty: SyscallParam),*> SyscallParam for Option<extern "system-unwind" fn($($ty),*)->__R>{}
    };
}

def_fn_tys!();
def_fn_tys!(A);
def_fn_tys!(A, B);
def_fn_tys!(A, B, C);
def_fn_tys!(A, B, C, D);
def_fn_tys!(A, B, C, D, E);
def_fn_tys!(A, B, C, D, E, F);

#[doc(hidden)]
pub use core as _core;
