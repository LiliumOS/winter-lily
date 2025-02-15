use std::collections::HashMap;

use memchr::memchr;

const KEY_TERM: u8 = 0xFE;
const ENTRY_TERM: u8 = 0xFF;

const EMPTY: [u8; 2] = [0xFE, 0xFF];

const fn entry_len(k: &str, v: &str) -> usize {
    core::mem::size_of::<usize>() + 2 + k.len() + v.len()
}

///
///
/// ## Format
/// The entire map is represented in the the environment map content.
///
/// An entry is given as the following psuedo-Rust struct:
/// ```rust,ignore
/// struct Entry {
///     len: usize,
///     key: str,
///     key_term: u8,
///     value: str,
///     entry_term: u8,
///     tail: [u8],
/// }
/// ```
///
/// Where key and value are the respective parts of the entry inlined into the structure.
/// `len` is the total length of the entry, and `key_term` and `entry_term` are 0xFE and 0xFF respectively.
/// These values are chosen because they are invalid UTF-8 and thus cannot appear in either a key or a value.
/// `tail` is arbitrary tail bytes that pad the entry to `len` bytes total.
///
/// An empty (free) entry is represented by a length followed by the bytes `0xFE 0xFF` (That is, an empty key and empty value).
/// An Empty key is never a valid environment variable, so it is fine for this to be a sentinel.
///
pub struct EnvMap {
    env: Vec<u8>,
    env_map: HashMap<String, usize>,
    freelist: Vec<usize>,
}

impl<K: Into<String> + AsRef<str>, V: AsRef<str>> FromIterator<(K, V)> for EnvMap {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut env = Vec::new();
        let mut env_map = HashMap::new();

        for (key, val) in iter {
            let s_key = key.as_ref();
            let val = val.as_ref();
            if env_map.contains_key(s_key) {
                panic!("Cannot insert duplicate key via `FromIterator`")
            }

            let len = entry_len(s_key, val);

            let off = env.len();
            env.reserve(len);
            env.extend_from_slice(&len.to_ne_bytes());
            env.extend_from_slice(s_key.as_bytes());
            env.push(KEY_TERM);
            env.extend_from_slice(val.as_bytes());
            env.push(ENTRY_TERM);
            env_map.insert(key.into(), off);
        }

        Self {
            env,
            env_map,
            freelist: Vec::new(),
        }
    }
}

impl<K: AsRef<str>, V: AsRef<str>> Extend<(K, V)> for EnvMap {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (key, val) in iter {
            self.set_var(key.as_ref(), val.as_ref());
        }
    }
}

impl EnvMap {
    pub fn from_posix_env() -> Self {
        std::env::vars().collect()
    }

    pub fn from_posix_env_lossy() -> Self {
        std::env::vars_os()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy_owned(k.into_encoded_bytes()),
                    String::from_utf8_lossy_owned(v.into_encoded_bytes()),
                )
            })
            .collect()
    }

    pub fn iter(&self) -> Iter {
        Iter(&self.env)
    }
    pub fn var(&self, key: &str) -> Option<&str> {
        if let Some(&var) = self.env_map.get(key) {
            let ptr = &self.env[(var + core::mem::size_of::<usize>())..];
            let ptr = &ptr[(key.len() + 1)..];
            let end = memchr(ENTRY_TERM, ptr).expect("Strings are terminated by 0xFF");
            Some(
                core::str::from_utf8(&ptr[..end])
                    .expect("Only valid UTF-8 is expected to be present"),
            )
        } else {
            None
        }
    }

    pub fn remove_var(&mut self, key: &str) {
        if let Some(var) = self.env_map.remove(key) {
            let ptr = &mut self.env[(var + core::mem::size_of::<usize>())..];
            ptr[..2].copy_from_slice(&EMPTY);
        }
    }

    pub fn set_var(&mut self, key: &str, val: &str) {
        let elen = entry_len(key, val);
        if let Some(&var) = self.env_map.get(key) {
            let ptr = &mut self.env[var..];

            let tlen = ptr
                .first_chunk()
                .copied()
                .map(|v| usize::from_ne_bytes(v))
                .expect("There is supposed to be a prefix here");

            if elen <= tlen {
                let off = core::mem::size_of::<usize>() + 1 + key.len();
                let ptr = &mut ptr[off..];
                let len = val.len();
                ptr[..len].copy_from_slice(val.as_bytes());
                ptr[len] = ENTRY_TERM;
                return;
            }
        }

        for (i, &elem) in self.freelist.iter().enumerate() {
            let ptr = &self.env[elem..];
            let tlen = ptr
                .first_chunk()
                .copied()
                .map(|v| usize::from_ne_bytes(v))
                .expect("There is supposed to be a prefix here");

            if tlen <= elen {
                if let Some((key_owned, old_off)) = self.env_map.remove_entry(key) {
                    let old_ptr = &mut self.env[(old_off + core::mem::size_of::<usize>())..];
                    old_ptr[..2].copy_from_slice(&EMPTY);
                    self.env_map.insert(key_owned, elem);
                    self.freelist[i] = old_off;
                } else {
                    self.freelist.swap_remove(i);
                    self.env_map.insert(key.to_string(), elem);
                }

                let ptr = &mut self.env[..elem];

                let off = core::mem::size_of::<usize>() + key.len();
                ptr[core::mem::size_of::<usize>()..off].copy_from_slice(key.as_bytes());
                ptr[off] = KEY_TERM;
                let ptr = &mut ptr[(off + 1)..];
                let len = val.len();
                ptr[..len].copy_from_slice(val.as_bytes());
                ptr[len] = ENTRY_TERM;
                return;
            }
        }

        self.env.reserve(elen);
        let elem = self.env.len();
        self.env.extend_from_slice(&elen.to_ne_bytes());
        self.env.extend_from_slice(key.as_bytes());
        self.env.push(KEY_TERM);
        self.env.extend_from_slice(val.as_bytes());
        self.env.push(ENTRY_TERM);

        if let Some((key_owned, old_off)) = self.env_map.remove_entry(key) {
            let old_ptr = &mut self.env[(old_off + core::mem::size_of::<usize>())..];
            old_ptr[..2].copy_from_slice(&EMPTY);
            self.env_map.insert(key_owned, elem);
            self.freelist.push(old_off);
        } else {
            self.env_map.insert(key.to_string(), elem);
        }
    }
}

pub struct Iter<'a>(&'a [u8]);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (len, rest) = self
            .0
            .split_first_chunk()
            .map(|(a, b)| (usize::from_ne_bytes(*a), b))?;

        self.0 = &self.0[len..];

        let off = memchr(KEY_TERM, rest).expect("There's supposed to be a terminator here");

        let (key, rest) = rest.split_at(off);

        let end = memchr(ENTRY_TERM, rest).expect("There's supposed to be a terminator here");

        let val = &rest[1..end];

        Some((
            core::str::from_utf8(key).unwrap(),
            core::str::from_utf8(val).unwrap(),
        ))
    }
}
