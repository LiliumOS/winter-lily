use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
    marker::PhantomData,
};

use alloc::alloc::{Allocator, Global};
use lccc_siphash::SipHasher;

#[derive(Clone, Debug)]
pub struct RandomState {
    k0: u64,
    k1: u64,
}

impl RandomState {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for RandomState {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildHasher for RandomState {
    type Hasher = SipHasher<2, 4>;

    fn build_hasher(&self) -> Self::Hasher {
        SipHasher::new_with_keys(self.k0, self.k1)
    }
}

pub struct HashMap<K, V, S = RandomState, A: Allocator = Global>(hashbrown::HashMap<K, V, S, A>);

impl<K, V> HashMap<K, V> {
    pub fn new() -> Self {
        Self::with_hasher_in(RandomState::new(), Global)
    }

    pub fn with_capacity(n: usize) -> Self {
        Self::with_capacity_and_hasher_in(n, RandomState::new(), Global)
    }
}

impl<K, V, A: Allocator> HashMap<K, V, RandomState, A> {
    pub fn new_in(alloc: A) -> Self {
        Self::with_hasher_in(RandomState::new(), alloc)
    }

    pub fn with_capacity_in(cap: usize, alloc: A) -> Self {
        Self::with_capacity_and_hasher_in(cap, RandomState::new(), alloc)
    }
}

impl<K, V, S> HashMap<K, V, S> {
    pub const fn with_hasher(hasher: S) -> Self {
        Self::with_hasher_in(hasher, Global)
    }

    pub fn with_capacity_and_hasher(cap: usize, hasher: S) -> Self {
        Self::with_capacity_and_hasher_in(cap, hasher, Global)
    }
}

impl<K, V, S, A: Allocator> HashMap<K, V, S, A> {
    pub const fn with_hasher_in(hasher: S, alloc: A) -> Self {
        Self(hashbrown::HashMap::with_hasher_in(hasher, alloc))
    }

    pub fn with_capacity_and_hasher_in(cap: usize, hasher: S, alloc: A) -> Self {
        Self(hashbrown::HashMap::with_capacity_and_hasher_in(
            cap, hasher, alloc,
        ))
    }
}

impl<K, V, S, A: Allocator> core::ops::Deref for HashMap<K, V, S, A> {
    type Target = hashbrown::HashMap<K, V, S, A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V, S, A: Allocator> core::ops::DerefMut for HashMap<K, V, S, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

use linux_syscall::{SYS_write, syscall};
pub use rustix::fd::*;

pub mod raw_mutex;

pub type Mutex<T> = lock_api::Mutex<raw_mutex::Futex, T>;
pub type RwLock<T> = lock_api::RwLock<raw_mutex::Futex, T>;

pub struct Stdio(BorrowedFd<'static>);

impl core::fmt::Write for Stdio {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        use linux_syscall::ResultSize;
        let mut b = s.as_bytes();

        while !b.is_empty() {
            match unsafe {
                syscall!(SYS_write, self.0.as_raw_fd(), b.as_ptr(), b.len()).try_usize()
            } {
                Ok(0) => return Err(core::fmt::Error),
                Ok(v) => b = &b[v..],
                Err(_) => return Err(core::fmt::Error),
            }
        }

        Ok(())
    }
}

pub use core::fmt;

pub use core::{write, writeln};

pub fn stdout() -> Stdio {
    Stdio(unsafe { rustix::stdio::stdout() })
}

pub fn stderr() -> Stdio {
    Stdio(unsafe { rustix::stdio::stderr() })
}

#[macro_export]
macro_rules! println {
    ($($args:tt)*) => {
        {
            use $crate::ministd::fmt::Write as _;
            $crate::ministd::writeln!($crate::ministd::stdout(), $($args)*).unwrap();
        }
    };
}

#[macro_export]
macro_rules! print {
    ($($args:tt)*) => {
        {
            use $crate::ministd::fmt::Write as _;
            $crate::ministd::write!($crate::ministd::stdout(), $($args)*).unwrap();
        }
    }
}

#[macro_export]
macro_rules! eprintln {
    ($($args:tt)*) => {
        {
            use $crate::ministd::fmt::Write;
            $crate::ministd::writeln!($crate::ministd::stderr(), $($args)*).unwrap();
        }
    };
}

#[macro_export]
macro_rules! eprint {
    ($($args:tt)*) => {
        {
            use $crate::ministd::fmt::Write as _;
            $crate::ministd::write!($crate::ministd::stderr(), $($args)*).unwrap();
        }
    }
}
