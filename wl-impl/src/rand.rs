use core::cell::{OnceCell, RefCell};

use crate::{
    helpers::{OnceLock, rand::Gen},
    ministd::Mutex,
};

pub(crate) static GLOBAL_SEED: OnceLock<Mutex<Gen>> = OnceLock::new();

pub fn fast_rand() -> u64 {
    #[thread_local]
    static LOCAL_SEED: OnceCell<RefCell<Gen>> = OnceCell::new();

    let local = LOCAL_SEED.get_or_init(|| {
        let global = GLOBAL_SEED
            .get()
            .expect("Must have called `__wl_impl_setup_process` before calling `fast_rand()`");
        let mut global = global.lock();
        let keys = [global.next(), global.next()];

        RefCell::new(Gen::seed(bytemuck::must_cast(keys)))
    });

    let mut borrowed = local.borrow_mut();
    borrowed.next()
}
