#![feature(
    array_chunks,
    iter_array_chunks,
    exact_size_is_empty,
    is_sorted,
    new_uninit,
    get_many_mut,
    int_roundings,
    slice_flatten,
    assert_matches,
    portable_simd,
    stdarch_x86_avx512,
)]
pub mod constraint_framework;
pub mod core;
pub mod examples;
pub mod math;
pub mod trace_generation;
