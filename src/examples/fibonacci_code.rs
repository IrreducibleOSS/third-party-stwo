// Autogenerated file. To regenerate, please run `FIX_TESTS=1 cargo test
// test_fibonacci_rust_codegen`.

use super::ops::*;
use crate::core::fields::m31::M31;
pub struct Input {
    pub secret: Vec<M31>,
}
pub struct Output {
    pub f: Vec<M31>,
}
pub fn compute(input: &Input) -> Output {
    let secret = &input.secret;
    let mut f = Vec::<M31>::with_capacity(32);
    unsafe {
        f.set_len(32);
    }
    for i in 0..1 {
        let one = M31::from_u32_unchecked(1);
        f[i * 1 + 0] = one;
    }
    for i in 0..1 {
        let f_secret = secret[(i * 1 + 1) % 1];
        f[i * 1 + 1] = f_secret;
    }
    for i in 0..30 {
        let f0 = f[i * 1 + 0];
        let f1 = f[i * 1 + 1];
        let f0sq = mul(f0, f0);
        let f1sq = mul(f1, f1);
        let f_rec = add(f0sq, f1sq);
        f[i * 1 + 2] = f_rec;
    }
    Output { f }
}
