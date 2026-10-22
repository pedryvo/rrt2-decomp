//! Differential tests for the deterministic `pow` (`rt2_sim::fp::pow`, docs/spec/trains.md
//! section 8): the CRT `_CIpow` at `0x51bab0` run in the emulator, and the same x87 sequence
//! (`fyl2x`, `frndint`, `f2xm1`, `fscale`, control word `0x27f`) run on the host CPU.
//!
//! The emulator (Unicorn, QEMU 5 core) evaluates `fyl2x` and `f2xm1` with host `double` math, so
//! it is only a coarse reference; the host x87 is the precise one. The decisive property is the
//! `ftol` of the game's speed formula, which both must reproduce.

use rt2_oracle::Oracle;
use rt2_sim::fp::{ftol, pow};

fn oracle() -> Option<Oracle> {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    Some(Oracle::load_file(&path).expect("load exe"))
}

struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Input domains of the game's two `pow` calls (trains.md 4.2): `0.98^(eff / power)` and
/// `(20 g + 1)^2.3` with `g` a positive `f32` grade.
fn game_inputs(n: usize) -> Vec<(f64, f64)> {
    let mut r = Rng(0x00c0_ffee);
    let mut v = Vec::new();
    for i in 0..n {
        if i % 2 == 0 {
            let eff = f64::from((r.unit() * 400_000.0) as u32);
            let p = f64::from((1.0 + r.unit() * r.unit() * 30_000.0) as f32);
            v.push((0.98, eff / p));
        } else {
            let g = f64::from((r.unit() * r.unit() * 0.3) as f32).max(1e-6);
            v.push((g * 20.0 + 1.0, 2.3));
        }
    }
    v
}

fn speed_ftol(p: f64, top: u32) -> i32 {
    ftol(p * f64::from(top << 16))
}

#[cfg(target_arch = "x86_64")]
fn host_pow(x: f64, y: f64) -> f64 {
    use std::arch::asm;
    let (mut out, new_cw, mut old_cw) = (0f64, 0x27fu16, 0u16);
    // SAFETY: only touches the x87 stack (cleared by the clobbers) and the locals passed in.
    unsafe {
        asm!(
            "fnstcw word ptr [{old}]",
            "fldcw word ptr [{new}]",
            "fld qword ptr [{x}]",
            "fld qword ptr [{y}]",
            "fxch st(1)",
            "fyl2x",
            "fld st(0)",
            "frndint",
            "fsubr st(1), st(0)",
            "fxch st(1)",
            "fchs",
            "f2xm1",
            "fld1",
            "faddp st(1), st(0)",
            "fscale",
            "fstp st(1)",
            "fstp qword ptr [{out}]",
            "fldcw word ptr [{old}]",
            old = in(reg) &mut old_cw,
            new = in(reg) &new_cw,
            x = in(reg) &x,
            y = in(reg) &y,
            out = in(reg) &mut out,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
        );
    }
    out
}

#[cfg(target_arch = "x86_64")]
#[test]
fn pow_matches_host_x87() {
    let inputs = game_inputs(400_000);
    let (mut bit_diff, mut max_ulp, mut ftol_diff) = (0usize, 0u64, 0usize);
    for &(x, y) in &inputs {
        let (a, b) = (pow(x, y), host_pow(x, y));
        if a.to_bits() != b.to_bits() {
            bit_diff += 1;
            max_ulp = max_ulp.max(a.to_bits().abs_diff(b.to_bits()));
        }
        for top in [10u32, 25, 60, 100, 160] {
            if speed_ftol(a, top) != speed_ftol(b, top) {
                ftol_diff += 1;
            }
        }
    }
    eprintln!(
        "host x87 vs rt2_sim::fp::pow: {} cases, {bit_diff} differ in the 53-bit result \
         (max {max_ulp} ulp), {ftol_diff} differ after ftol",
        inputs.len()
    );
    assert_eq!(ftol_diff, 0);
    assert!(max_ulp <= 1);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn pow_generic_range_close_to_host_x87() {
    let mut r = Rng(7);
    let (mut diff, mut max_ulp) = (0usize, 0u64);
    for _ in 0..200_000 {
        let x = f64::exp2((r.unit() - 0.5) * 60.0);
        let y = (r.unit() - 0.5) * 100.0;
        let (a, b) = (pow(x, y), host_pow(x, y));
        if !(b.is_finite() && b > 1e-290) {
            continue;
        }
        if a.to_bits() != b.to_bits() {
            diff += 1;
            max_ulp = max_ulp.max(a.to_bits().abs_diff(b.to_bits()));
        }
    }
    eprintln!("generic range: {diff} bit differences, max {max_ulp} ulp");
    assert!(max_ulp <= 1);
}

const CRT_POW: u32 = 0x0051_bab0; // _CIpow: ST1 = x, ST0 = y -> ST0

/// Writes `fld [x]; fld [y]; call _CIpow; fstp [out]; ret` over the (never called) CRT entry
/// point and runs it.
fn oracle_pow(o: &mut Oracle, stub: u32, io: u32, x: f64, y: f64) -> f64 {
    o.write_mem(io, &x.to_le_bytes()).unwrap();
    o.write_mem(io + 8, &y.to_le_bytes()).unwrap();
    let uc = o.uc();
    use unicorn_engine::RegisterX86;
    uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    uc.reg_write(RegisterX86::FPSW, 0).unwrap();
    uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
    o.call_cdecl(stub, &[]).expect("run stub");
    f64::from_le_bytes(o.read_mem(io + 16, 8).unwrap().try_into().unwrap())
}

fn install_stub(o: &mut Oracle, io: u32) -> u32 {
    let stub = o.image().image_base + o.image().entry_rva;
    let mut c = vec![0xdd, 0x05];
    c.extend(io.to_le_bytes()); // fld qword [x]
    c.extend([0xdd, 0x05]);
    c.extend((io + 8).to_le_bytes()); // fld qword [y]
    let call_at = stub + c.len() as u32;
    c.push(0xe8);
    c.extend(CRT_POW.wrapping_sub(call_at + 5).to_le_bytes());
    c.extend([0xdd, 0x1d]);
    c.extend((io + 16).to_le_bytes()); // fstp qword [out]
    c.push(0xc3);
    o.write_mem(stub, &c).unwrap();
    stub
}

#[test]
fn pow_matches_original_crt_after_ftol() {
    let Some(mut o) = oracle() else { return };
    let io = o.alloc(32).unwrap();
    let stub = install_stub(&mut o, io);
    // Sanity on exact cases through the emulator.
    assert_eq!(oracle_pow(&mut o, stub, io, 2.0, 10.0), 1024.0);
    let inputs = game_inputs(100_000);
    let (mut bit_diff, mut max_ulp, mut ftol_diff) = (0usize, 0u64, 0usize);
    for &(x, y) in &inputs {
        let (a, b) = (pow(x, y), oracle_pow(&mut o, stub, io, x, y));
        if a.to_bits() != b.to_bits() {
            bit_diff += 1;
            max_ulp = max_ulp.max(a.to_bits().abs_diff(b.to_bits()));
        }
        for top in [10u32, 25, 60, 100, 160] {
            if speed_ftol(a, top) != speed_ftol(b, top) {
                ftol_diff += 1;
            }
        }
    }
    eprintln!(
        "original _CIpow in Unicorn vs rt2_sim::fp::pow: {} cases, {bit_diff} differ in the \
         53-bit result (max {max_ulp} ulp), {ftol_diff} differ after ftol",
        inputs.len()
    );
    assert_eq!(ftol_diff, 0);
}
