// vim: tw=80
use std::mem;

use rand::{RngCore, SeedableRng};  

// This stuff could be upstreamed to the libc create
mod ffi {
    use libc::{c_uint, c_char, c_long};

    extern "C" {
        pub(super) fn initstate(seed: c_uint, state: *mut c_char, n: usize)
            -> *mut c_char;
        pub(super) fn random() -> c_long;
        pub(super) fn setstate(state: *mut c_char) -> *mut c_char;
    }
}

// TODO: since the state is actually global, enforce that an OsPRng must be a
// singleton.
pub(crate) struct OsPRng {
    // Actually, could be any size u32 array.
    state: Box<[u32; 64]>
}

#[cfg(target_pointer_width = "64")]
impl RngCore for OsPRng {
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // It would be more efficient to fill 4 or 8 bytes at a time.  However,
        // filling one byte at a time is what the C-based fsx does.
        for b in dest.iter_mut() {
            *b = (unsafe{ ffi::random() } & 0xFF) as u8;
        }
    }

    fn next_u32(&mut self) -> u32 {
        // Safety: inherently safe.
        (unsafe{ ffi::random() } & 0xFFFFFFFF) as u32
    }

    fn next_u64(&mut self) -> u64 {
        // Safety: inherently safe.
        (unsafe{ ffi::random() }) as u64
    }

    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
        unimplemented!()
    }

}

impl SeedableRng for OsPRng {
    type Seed = [u8; 4];

    fn from_seed(seed: Self::Seed) -> Self {
        let mut self_ = OsPRng {
            state: Box::new([0u32; 64])
        };
        let s = u32::from_ne_bytes(seed);
        unsafe {
            ffi::initstate(
                s,
                //u32::from_ne_bytes(seed),
                (self_.state).as_mut_ptr() as *mut _,
                mem::size_of_val(&*self_.state)
            );
            ffi::setstate((self_.state).as_mut_ptr() as *mut _);
       }
        self_
    }
    
}


