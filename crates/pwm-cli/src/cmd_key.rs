//! `key-gen` CLI path.

use rand::RngCore;

pub(crate) fn run_keygen() {
    let mut s = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut s);
    println!("{}", hex::encode(s));
}
