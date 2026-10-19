//! Windows: fast maths for the audio engine.
//!
//! The windows-gnu (mingw-w64) runtime implements `sinf`, `tanf`, `floorf`
//! and friends with x87 code and forwards `expf`/`powf` to msvcrt's double
//! versions; they are 10–30x slower than on Linux/macOS (e.g. ~130 ns per
//! `sinf` against ~4 ns). A heavy song (the Chrome Throat demo) then needs
//! more than 100 % of a core to render, so the audio thread underruns until
//! the latency stops growing at its 331 ms ceiling.
//!
//! Defining the C symbols here makes the linker take these (Rust's `libm`,
//! a port of musl) instead of the runtime's, so every `f32::sin`, `exp`,
//! `powf`... in the engine (and in the C code we link) uses them. Results are
//! accurate to within an ulp, so Windows output can differ from earlier
//! Windows builds in the last bit; Linux and macOS are untouched.
macro_rules! unary {
    ($($n:ident: $t:ty),* $(,)?) => {
        $(
            #[no_mangle]
            pub extern "C" fn $n(x: $t) -> $t {
                libm::$n(x)
            }
        )*
    };
}

macro_rules! binary {
    ($($n:ident: $t:ty),* $(,)?) => {
        $(
            #[no_mangle]
            pub extern "C" fn $n(x: $t, y: $t) -> $t {
                libm::$n(x, y)
            }
        )*
    };
}

unary!(
    sinf: f32, cosf: f32, tanf: f32, asinf: f32, acosf: f32, atanf: f32,
    sinhf: f32, coshf: f32, tanhf: f32,
    expf: f32, exp2f: f32, expm1f: f32, logf: f32, log2f: f32, log10f: f32, log1pf: f32,
    cbrtf: f32, floorf: f32, ceilf: f32, roundf: f32, truncf: f32,
    sin: f64, cos: f64, tan: f64, asin: f64, acos: f64, atan: f64,
    sinh: f64, cosh: f64, tanh: f64,
    exp: f64, exp2: f64, expm1: f64, log: f64, log2: f64, log10: f64, log1p: f64,
    cbrt: f64, floor: f64, ceil: f64, round: f64, trunc: f64,
);

binary!(
    powf: f32, atan2f: f32, hypotf: f32, fmodf: f32,
    pow: f64, atan2: f64, hypot: f64, fmod: f64,
);

#[cfg(test)]
mod tests {
    /// std's float maths must land on these (the override is linked in)
    #[test]
    fn std_maths_use_libm() {
        for i in 0..2000 {
            let x = (i as f32 - 1000.0) * 0.0137;
            assert_eq!(x.sin().to_bits(), libm::sinf(x).to_bits());
            assert_eq!(x.tan().to_bits(), libm::tanf(x).to_bits());
            assert_eq!(x.exp().to_bits(), libm::expf(x).to_bits());
            // (LLVM turns powf(2, x) into exp2f(x))
            assert_eq!(2f32.powf(x).to_bits(), libm::exp2f(x).to_bits());
            let b = std::hint::black_box(10f32);
            assert_eq!(b.powf(x).to_bits(), libm::powf(10.0, x).to_bits());
            assert_eq!(x.floor().to_bits(), libm::floorf(x).to_bits());
            assert_eq!((x % 4.0).to_bits(), libm::fmodf(x, 4.0).to_bits());
            let d = x as f64;
            assert_eq!(d.sin().to_bits(), libm::sin(d).to_bits());
            assert_eq!(d.exp().to_bits(), libm::exp(d).to_bits());
        }
        assert!(std::hint::black_box(0.0f32).ln().is_infinite());
    }
}
