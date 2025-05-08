use core::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use lock_api::{
    GuardSend, RawMutex, RawMutexTimed, RawRwLock, RawRwLockDowngrade, RawRwLockUpgrade,
};
use rustix::{thread::futex, thread::sched_yield};

pub struct Futex(AtomicU32);

unsafe impl RawRwLock for Futex {
    const INIT: Self = Self(AtomicU32::new(0));

    type GuardMarker = GuardSend;

    fn lock_exclusive(&self) {
        let mut step = 0u16;

        while let Err(e) =
            self.0
                .compare_exchange_weak(0, 0x80000000, Ordering::Acquire, Ordering::Relaxed)
        {
            if e == 0 {
                core::hint::spin_loop();
                continue;
            }
            match step {
                0..32 => {
                    core::hint::spin_loop();
                }
                32..256 => {
                    sched_yield();
                }
                256.. => {
                    let _ = futex::wait(&self.0, futex::Flags::empty(), e, None);
                }
            }
            step = step.saturating_add(1);
        }
    }

    fn lock_shared(&self) {
        let mut step = 0u16;

        let mut load = 0;

        loop {
            if (load & 0x80000000) == 0 {
                // This can wrap into the exclusive bit, but the effect is to block any future shared locks from being acquire until we drop back down below (1<<31).
                match self.0.compare_exchange_weak(
                    load,
                    load + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(e) => {
                        load = e;
                        core::hint::spin_loop();
                        continue;
                    }
                }
            }

            match step {
                0..32 => {
                    core::hint::spin_loop();
                }
                32..256 => {
                    sched_yield();
                }
                256.. => {
                    let _ = futex::wait(&self.0, futex::Flags::empty(), load, None);
                }
            }
            step = step.saturating_add(1);
        }
    }

    fn is_locked(&self) -> bool {
        self.0.load(Ordering::Relaxed) != 0
    }

    fn is_locked_exclusive(&self) -> bool {
        // This also returns true if there are 0x80000000 shared locks held simultaneously
        (self.0.load(Ordering::Relaxed) & 0x80000000) != 0
    }

    fn try_lock_exclusive(&self) -> bool {
        self.0
            .compare_exchange(0, 0x80000000, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn try_lock_shared(&self) -> bool {
        let mut load = self.0.load(Ordering::Relaxed);

        while (load & 0x80000000) == 0 {
            match self
                .0
                .compare_exchange_weak(load, load + 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return true,
                Err(e) => {
                    load = e;
                    core::hint::spin_loop();
                    continue;
                }
            }
        }
        false
    }

    unsafe fn unlock_exclusive(&self) {
        self.0.store(0, Ordering::Release);
    }

    unsafe fn unlock_shared(&self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

unsafe impl RawRwLockDowngrade for Futex {
    unsafe fn downgrade(&self) {
        self.0.swap(1, Ordering::AcqRel);
    }
}

unsafe impl RawMutex for Futex {
    const INIT: Self = Self(AtomicU32::new(0));

    type GuardMarker = GuardSend;

    fn is_locked(&self) -> bool {
        <Self as RawRwLock>::is_locked(self)
    }

    fn lock(&self) {
        self.lock_exclusive();
    }

    fn try_lock(&self) -> bool {
        self.try_lock_exclusive()
    }

    unsafe fn unlock(&self) {
        // Safety: Assured by preconditions
        unsafe { self.unlock_exclusive() }
    }
}

pub type Mutex<T> = lock_api::Mutex<Futex, T>;
pub type RwLock<T> = lock_api::RwLock<Futex, T>;
