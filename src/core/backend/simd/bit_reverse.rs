use std::array;
use std::mem::transmute;
use std::simd::Swizzle;

use super::utils::{LoHiInterleaveHiHi, LoLoInterleaveHiLo};
use super::PackedBaseField;
use crate::core::utils::bit_reverse_index;

const VEC_BITS: u32 = 4;

const W_BITS: u32 = 3;

pub const MIN_LOG_SIZE: u32 = 2 * W_BITS + VEC_BITS;

/// Bit reverses packed M31 values.
///
/// Given an array `A[0..2^n)`, computes `B[i] = A[bit_reverse(i)]`.
pub fn bit_reverse_m31(data: &mut [PackedBaseField]) {
    assert!(data.len().is_power_of_two());
    assert!(data.len().ilog2() >= MIN_LOG_SIZE);

    // Indices in the array are of the form v_h w_h a w_l v_l, with
    // |v_h| = |v_l| = VEC_BITS, |w_h| = |w_l| = W_BITS, |a| = n - 2*W_BITS - VEC_BITS.
    // The loops go over a, w_l, w_h, and then swaps the 16 by 16 values at:
    //   * w_h a w_l *   <->   * rev(w_h a w_l) *.
    // These are 1 or 2 chunks of 2^W_BITS contiguous AVX512 vectors.

    let log_size = data.len().ilog2();
    let a_bits = log_size - 2 * W_BITS - VEC_BITS;

    // TODO(spapini): when doing multithreading, do it over a.
    for a in 0u32..(1 << a_bits) {
        for w_l in 0u32..(1 << W_BITS) {
            let w_l_rev = w_l.reverse_bits() >> (u32::BITS - W_BITS);
            for w_h in 0..(w_l_rev + 1) {
                let idx = ((((w_h << a_bits) | a) << W_BITS) | w_l) as usize;
                let idx_rev = bit_reverse_index(idx, log_size - VEC_BITS);

                // In order to not swap twice, only swap if idx <= idx_rev.
                if idx > idx_rev {
                    continue;
                }

                // Read first chunk.
                // TODO(spapini): Think about optimizing a_bits.
                let chunk0 = array::from_fn(|i| unsafe {
                    *data.get_unchecked(idx + (i << (2 * W_BITS + a_bits)))
                });
                let values0 = bit_reverse16(chunk0);

                if idx == idx_rev {
                    // Palindrome index. Write into the same chunk.
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..16 {
                        unsafe {
                            *data.get_unchecked_mut(idx + (i << (2 * W_BITS + a_bits))) =
                                values0[i];
                        }
                    }
                    continue;
                }

                // Read bit reversed chunk.
                let chunk1 = array::from_fn(|i| unsafe {
                    *data.get_unchecked(idx_rev + (i << (2 * W_BITS + a_bits)))
                });
                let values1 = bit_reverse16(chunk1);

                for i in 0..16 {
                    unsafe {
                        *data.get_unchecked_mut(idx + (i << (2 * W_BITS + a_bits))) = values1[i];
                        *data.get_unchecked_mut(idx_rev + (i << (2 * W_BITS + a_bits))) =
                            values0[i];
                    }
                }
            }
        }
    }
}

/// Bit reverses 256 M31 values, packed in 16 words of 16 elements each.
fn bit_reverse16(data: [PackedBaseField; 16]) -> [PackedBaseField; 16] {
    let mut data = data.map(PackedBaseField::into_simd);

    // Denote the index of each element in the 16 packed M31 words as abcd:0123,
    // where abcd is the index of the packed word and 0123 is the index of the element in the word.
    // Bit reversal is achieved by applying the following permutation to the index for 4 times:
    //   abcd:0123 => 0abc:123d
    // This is how it looks like at each iteration.
    //   abcd:0123
    //   0abc:123d
    //   10ab:23dc
    //   210a:3dcb
    //   3210:dcba
    for _ in 0..4 {
        // Apply the abcd:0123 => 0abc:123d permutation.
        // `LoLoInterleaveHiLo` allows us to interleave the first half of 2 words.
        // For example, the second call interleaves 0010:0xyz (low half of register 2) with
        // 0011:0xyz (low half of register 3), and stores the result in register 1 (0001).
        // This results in
        //    0001:xyz0 (even indices of register 1) <= 0010:0xyz (low half of register2), and
        //    0001:xyz1 (odd indices of register 1)  <= 0011:0xyz (low half of register 3)
        // or 0001:xyzw <= 001w:0xyz.
        data = [
            LoLoInterleaveHiLo::concat_swizzle(data[0], data[1]),
            LoLoInterleaveHiLo::concat_swizzle(data[2], data[3]),
            LoLoInterleaveHiLo::concat_swizzle(data[4], data[5]),
            LoLoInterleaveHiLo::concat_swizzle(data[6], data[7]),
            LoLoInterleaveHiLo::concat_swizzle(data[8], data[9]),
            LoLoInterleaveHiLo::concat_swizzle(data[10], data[11]),
            LoLoInterleaveHiLo::concat_swizzle(data[12], data[13]),
            LoLoInterleaveHiLo::concat_swizzle(data[14], data[15]),
            LoHiInterleaveHiHi::concat_swizzle(data[0], data[1]),
            LoHiInterleaveHiHi::concat_swizzle(data[2], data[3]),
            LoHiInterleaveHiHi::concat_swizzle(data[4], data[5]),
            LoHiInterleaveHiHi::concat_swizzle(data[6], data[7]),
            LoHiInterleaveHiHi::concat_swizzle(data[8], data[9]),
            LoHiInterleaveHiHi::concat_swizzle(data[10], data[11]),
            LoHiInterleaveHiHi::concat_swizzle(data[12], data[13]),
            LoHiInterleaveHiHi::concat_swizzle(data[14], data[15]),
        ];
    }

    unsafe { transmute(data) }
}

#[cfg(test)]
mod tests {
    use std::array;
    use std::mem::transmute;

    use aligned::{Aligned, A64};

    use super::bit_reverse16;
    use crate::core::backend::simd::bit_reverse::bit_reverse_m31;
    use crate::core::backend::simd::column::BaseFieldVec;
    use crate::core::backend::Column;
    use crate::core::fields::m31::BaseField;
    use crate::core::utils::bit_reverse as ground_truth_bit_reverse;

    #[test]
    fn test_bit_reverse16() {
        let data: Aligned<A64, [u32; 256]> = Aligned(array::from_fn(|i| i as u32));
        let mut expected: Aligned<A64, [u32; 256]> = data;
        ground_truth_bit_reverse(&mut *expected);

        let res = unsafe { transmute::<_, [u32; 256]>(bit_reverse16(transmute(data))) };

        assert_eq!(res, *expected);
    }

    #[test]
    fn test_bit_reverse() {
        const SIZE: usize = 1 << 15;
        let data: Vec<_> = (0..SIZE).map(BaseField::from).collect();
        let mut expected = data.clone();
        ground_truth_bit_reverse(&mut expected);

        let mut res: BaseFieldVec = data.into_iter().collect();
        bit_reverse_m31(&mut res.data[..]);

        assert_eq!(res.to_vec(), expected);
    }
}
