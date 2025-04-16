use lccc_siphash::RawSipHasher;

pub struct Gen(RawSipHasher<2, 4>);

impl Gen {
    pub const fn seed(k: [u8; 16]) -> Self {
        let [k0, k1] = unsafe { core::mem::transmute(k) };
        Self(RawSipHasher::from_keys(k0, k1))
    }

    pub fn next(&mut self) -> u64 {
        self.0.update(0x6a09e667f3bcc908);
        let ret = self.0.finish();
        self.0.update(0xbb67ae8584caa73b);
        ret
    }
}
