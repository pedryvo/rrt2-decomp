//! Differential-testing harness: runs individual functions of the original
//! executable (`RT2_PLAT.EXE`, PE32) inside the Unicorn x86 emulator so that
//! their results can be compared bit-for-bit with the Rust implementations.
//! See `docs/oracle.md`.

mod pe;

use std::collections::HashMap;

use unicorn_engine::{Arch, Mode, Prot, RegisterX86, Unicorn};

pub use pe::{Import, PeImage, Section};

pub type Uc = Unicorn<'static, State>;
/// Emulator handle as seen by import hooks (any lifetime).
pub type HookUc<'a> = Unicorn<'a, State>;

#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    #[error("invalid PE image: {0}")]
    Pe(String),
    #[error("unicorn error: {0}")]
    Unicorn(String),
    #[error("emulated code called unhooked import {dll}!{name}")]
    ImportCalled { dll: String, name: String },
    #[error("instruction limit ({0}) reached before the function returned")]
    InstructionLimit(u64),
    #[error("emulated heap exhausted")]
    HeapExhausted,
}

pub type Result<T> = std::result::Result<T, OracleError>;

fn uc(e: unicorn_engine::uc_error) -> OracleError {
    OracleError::Unicorn(format!("{e:?}"))
}

const PAGE: u64 = 0x1000;
const STACK_BASE: u64 = 0x7000_0000;
const STACK_SIZE: u64 = 0x10_0000;
const STUB_BASE: u64 = 0x7100_0000;
const STUB_SLOT: u64 = 16;
const HEAP_BASE: u64 = 0x2000_0000;
const HEAP_SIZE: u64 = 0x100_0000;
/// Return address pushed for every call; never executed, emulation stops on it.
pub const SENTINEL: u32 = 0x7200_0000;
/// Default instruction budget per call.
pub const DEFAULT_LIMIT: u64 = 10_000_000;
/// x87 control word of the game's main thread: 53-bit precision, round to nearest.
pub const GAME_FPCW: u64 = 0x27f;

fn align_up(v: u64) -> u64 {
    v.div_ceil(PAGE) * PAGE
}

/// Rust implementation of an imported OS function. Read stack arguments with
/// [`stack_arg`]; the returned value is placed in EAX. The stub then executes
/// `ret N` (`N` = `arg_bytes`, 0 for cdecl imports).
pub type ImportHook = Box<dyn for<'a, 'b> FnMut(&'a mut HookUc<'b>) -> u32>;

struct Hook {
    f: ImportHook,
}

/// Emulator-side state shared with hooks.
#[derive(Default)]
pub struct State {
    imports: Vec<(String, String)>,
    hooks: HashMap<usize, Hook>,
    fault: Option<OracleError>,
}

/// Reads the `n`-th (0-based) 32-bit stack argument inside an import hook.
pub fn stack_arg(uc: &HookUc<'_>, n: u32) -> u32 {
    let esp = uc.reg_read(RegisterX86::ESP).unwrap_or(0);
    let mut b = [0u8; 4];
    let _ = uc.mem_read(esp + 4 + 4 * u64::from(n), &mut b);
    u32::from_le_bytes(b)
}

pub struct Oracle {
    uc: Uc,
    image: PeImage,
    limit: u64,
    heap_next: u64,
    snapshot: Vec<(u32, Vec<u8>)>,
    ctx_regs: [u64; 8],
}

const GPRS: [RegisterX86; 8] = [
    RegisterX86::EAX,
    RegisterX86::ECX,
    RegisterX86::EDX,
    RegisterX86::EBX,
    RegisterX86::ESP,
    RegisterX86::EBP,
    RegisterX86::ESI,
    RegisterX86::EDI,
];

impl Oracle {
    /// Loads a PE32 executable image from disk.
    pub fn load_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let data = std::fs::read(path.as_ref())
            .map_err(|e| OracleError::Pe(format!("{}: {e}", path.as_ref().display())))?;
        Self::load(&data)
    }

    pub fn load(data: &[u8]) -> Result<Self> {
        let image = PeImage::parse(data)?;
        let mut uc =
            Unicorn::new_with_data(Arch::X86, Mode::MODE_32, State::default()).map_err(self::uc)?;

        // Headers, read-only.
        let hdr = align_up(u64::from(image.size_of_headers));
        uc.mem_map(u64::from(image.image_base), hdr, Prot::ALL)
            .map_err(self::uc)?;
        uc.mem_write(
            u64::from(image.image_base),
            &data[..image.size_of_headers as usize],
        )
        .map_err(self::uc)?;

        for s in &image.sections {
            let size = align_up(u64::from(s.virtual_size.max(s.raw_size)));
            let addr = u64::from(image.image_base) + u64::from(s.rva);
            uc.mem_map(addr, size, Prot::ALL).map_err(self::uc)?;
            let raw = data
                .get(s.raw_offset as usize..(s.raw_offset + s.raw_size) as usize)
                .ok_or_else(|| OracleError::Pe(format!("section {} out of file", s.name)))?;
            let n = raw.len().min(s.virtual_size as usize);
            uc.mem_write(addr, &raw[..n]).map_err(self::uc)?;
        }

        // Stack, heap, import stubs.
        uc.mem_map(STACK_BASE, STACK_SIZE, Prot::READ | Prot::WRITE)
            .map_err(self::uc)?;
        uc.mem_map(HEAP_BASE, HEAP_SIZE, Prot::READ | Prot::WRITE)
            .map_err(self::uc)?;
        let stub_size = align_up(image.imports.len() as u64 * STUB_SLOT).max(PAGE);
        uc.mem_map(STUB_BASE, stub_size, Prot::ALL)
            .map_err(self::uc)?;
        for (i, imp) in image.imports.iter().enumerate() {
            let stub = STUB_BASE + i as u64 * STUB_SLOT;
            uc.mem_write(stub, &[0xC3]).map_err(self::uc)?; // ret
            let iat = u64::from(image.image_base) + u64::from(imp.iat_rva);
            uc.mem_write(iat, &(stub as u32).to_le_bytes())
                .map_err(self::uc)?;
            uc.get_data_mut()
                .imports
                .push((imp.dll.clone(), imp.name.clone()));
        }
        let end = STUB_BASE + stub_size;
        uc.add_code_hook(STUB_BASE, end, move |uc, addr, _| {
            let idx = ((addr - STUB_BASE) / STUB_SLOT) as usize;
            if let Some(mut h) = uc.get_data_mut().hooks.remove(&idx) {
                let r = (h.f)(uc);
                let _ = uc.reg_write(RegisterX86::EAX, u64::from(r));
                uc.get_data_mut().hooks.insert(idx, h);
            } else {
                let (dll, name) = uc.get_data().imports[idx].clone();
                uc.get_data_mut().fault = Some(OracleError::ImportCalled { dll, name });
                let _ = uc.emu_stop();
            }
        })
        .map_err(self::uc)?;

        // Final permissions.
        uc.mem_protect(u64::from(image.image_base), hdr, Prot::READ)
            .map_err(self::uc)?;
        for s in &image.sections {
            let size = align_up(u64::from(s.virtual_size.max(s.raw_size)));
            let addr = u64::from(image.image_base) + u64::from(s.rva);
            let mut p = Prot::NONE;
            if s.characteristics & 0x4000_0000 != 0 {
                p |= Prot::READ;
            }
            if s.characteristics & 0x8000_0000 != 0 {
                p |= Prot::WRITE;
            }
            if s.characteristics & 0x2000_0000 != 0 {
                p |= Prot::EXEC | Prot::READ;
            }
            uc.mem_protect(addr, size, p).map_err(self::uc)?;
        }

        // The game's main thread runs x87 at 53-bit precision (measured under Wine, see
        // docs/dynamic-analysis.md); Unicorn starts with extended precision.
        uc.reg_write(RegisterX86::FPCW, GAME_FPCW)
            .map_err(self::uc)?;

        let mut o = Self {
            uc,
            image,
            limit: DEFAULT_LIMIT,
            heap_next: HEAP_BASE,
            snapshot: Vec::new(),
            ctx_regs: [0; 8],
        };
        o.snapshot();
        Ok(o)
    }

    pub fn image(&self) -> &PeImage {
        &self.image
    }

    /// Sets the per-call instruction budget.
    pub fn set_instruction_limit(&mut self, n: u64) {
        self.limit = n;
    }

    /// Registers a Rust hook for an import, matched by function name (a
    /// unique one; use `"DLL!name"` to disambiguate). `arg_bytes` is the
    /// stdcall cleanup (4 * args), 0 for cdecl.
    pub fn hook_import(&mut self, name: &str, arg_bytes: u16, f: ImportHook) -> Result<()> {
        let idxs: Vec<usize> = self
            .uc
            .get_data()
            .imports
            .iter()
            .enumerate()
            .filter(|(_, (d, n))| n == name || format!("{d}!{n}") == name)
            .map(|(i, _)| i)
            .collect();
        let [i] = idxs[..] else {
            return Err(OracleError::Pe(format!(
                "import {name} matched {} slots",
                idxs.len()
            )));
        };
        let stub = STUB_BASE + i as u64 * STUB_SLOT;
        let ret_n = [0xC2, arg_bytes as u8, (arg_bytes >> 8) as u8];
        let code = if arg_bytes == 0 {
            &[0xC3][..]
        } else {
            &ret_n[..]
        };
        self.uc.mem_write(stub, code).map_err(uc)?;
        self.uc.get_data_mut().hooks.insert(i, Hook { f });
        Ok(())
    }

    /// Direct access to the emulator for advanced use.
    pub fn uc(&mut self) -> &mut Uc {
        &mut self.uc
    }

    pub fn read_mem(&self, addr: u32, len: usize) -> Result<Vec<u8>> {
        self.uc.mem_read_as_vec(u64::from(addr), len).map_err(uc)
    }

    pub fn write_mem(&mut self, addr: u32, bytes: &[u8]) -> Result<()> {
        self.uc.mem_write(u64::from(addr), bytes).map_err(uc)
    }

    pub fn read_u32(&self, addr: u32) -> Result<u32> {
        let b = self.read_mem(addr, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn write_u32(&mut self, addr: u32, v: u32) -> Result<()> {
        self.write_mem(addr, &v.to_le_bytes())
    }

    /// Bump-allocates `len` zeroed bytes (16-aligned) in the emulated heap,
    /// e.g. for struct arguments. Never freed.
    pub fn alloc(&mut self, len: u32) -> Result<u32> {
        let size = u64::from(len).div_ceil(16) * 16;
        if self.heap_next + size > HEAP_BASE + HEAP_SIZE {
            return Err(OracleError::HeapExhausted);
        }
        let p = self.heap_next;
        self.heap_next += size;
        Ok(p as u32)
    }

    /// Allocates and fills a buffer.
    pub fn alloc_bytes(&mut self, bytes: &[u8]) -> Result<u32> {
        let p = self.alloc(bytes.len() as u32)?;
        self.write_mem(p, bytes)?;
        Ok(p)
    }

    pub fn eax(&self) -> u32 {
        self.uc.reg_read(RegisterX86::EAX).unwrap_or(0) as u32
    }

    pub fn edx(&self) -> u32 {
        self.uc.reg_read(RegisterX86::EDX).unwrap_or(0) as u32
    }

    /// Value of x87 ST(0) (top of the FPU stack) after a call, converted from
    /// 80-bit extended to `f64`.
    pub fn st0(&self) -> Result<f64> {
        let raw = self.uc.reg_read_long(RegisterX86::ST0).map_err(uc)?;
        Ok(f80_to_f64(&raw))
    }

    /// 64-bit return value (EDX:EAX).
    pub fn edx_eax(&self) -> u64 {
        (u64::from(self.edx()) << 32) | u64::from(self.eax())
    }

    /// Saves every writable section (`.data` and friends) and the general
    /// registers; [`Oracle::restore`] puts them back.
    pub fn snapshot(&mut self) {
        let base = u64::from(self.image.image_base);
        let mut snap = Vec::new();
        for s in &self.image.sections {
            if s.characteristics & 0x8000_0000 != 0 {
                let addr = (base + u64::from(s.rva)) as u32;
                let len = s.virtual_size.max(s.raw_size) as usize;
                if let Ok(b) = self.uc.mem_read_as_vec(u64::from(addr), len) {
                    snap.push((addr, b));
                }
            }
        }
        self.snapshot = snap;
        for (i, r) in GPRS.iter().enumerate() {
            self.ctx_regs[i] = self.uc.reg_read(*r).unwrap_or(0);
        }
    }

    /// Restores the writable sections saved by the last [`Oracle::snapshot`]
    /// (taken at load time by default). The heap and FPU state are not reset.
    pub fn restore(&mut self) -> Result<()> {
        for (addr, b) in &self.snapshot {
            self.uc.mem_write(u64::from(*addr), b).map_err(uc)?;
        }
        Ok(())
    }

    /// `cdecl`: args pushed right to left, caller cleans up.
    pub fn call_cdecl(&mut self, addr: u32, args: &[u32]) -> Result<u32> {
        self.run(addr, args, 0, 0)
    }

    /// `stdcall`: args pushed right to left, callee cleans up.
    pub fn call_stdcall(&mut self, addr: u32, args: &[u32]) -> Result<u32> {
        self.run(addr, args, 0, 0)
    }

    /// MSVC `thiscall`: ECX = `this`, remaining args on the stack.
    pub fn call_thiscall(&mut self, addr: u32, this: u32, args: &[u32]) -> Result<u32> {
        self.run(addr, args, this, 0)
    }

    /// `fastcall`-style: ECX and EDX set, remaining args on the stack.
    pub fn call_fastcall(&mut self, addr: u32, ecx: u32, edx: u32, args: &[u32]) -> Result<u32> {
        self.run(addr, args, ecx, edx)
    }

    fn run(&mut self, addr: u32, args: &[u32], ecx: u32, edx: u32) -> Result<u32> {
        let top = STACK_BASE + STACK_SIZE - 0x100;
        let mut sp = top;
        for a in args.iter().rev() {
            sp -= 4;
            self.uc.mem_write(sp, &a.to_le_bytes()).map_err(uc)?;
        }
        sp -= 4;
        self.uc.mem_write(sp, &SENTINEL.to_le_bytes()).map_err(uc)?;
        for r in GPRS {
            self.uc.reg_write(r, 0).map_err(uc)?;
        }
        self.uc
            .reg_write(RegisterX86::ECX, u64::from(ecx))
            .map_err(uc)?;
        self.uc
            .reg_write(RegisterX86::EDX, u64::from(edx))
            .map_err(uc)?;
        self.uc.reg_write(RegisterX86::ESP, sp).map_err(uc)?;
        self.uc.reg_write(RegisterX86::EBP, top).map_err(uc)?;
        self.uc.get_data_mut().fault = None;
        let res = self
            .uc
            .emu_start(u64::from(addr), u64::from(SENTINEL), 0, self.limit as usize);
        if let Some(f) = self.uc.get_data_mut().fault.take() {
            return Err(f);
        }
        res.map_err(uc)?;
        let pc = self.uc.pc_read().map_err(uc)? as u32;
        if pc != SENTINEL {
            // Unicorn returns Ok on the count limit; faults are handled above.
            return Err(OracleError::InstructionLimit(self.limit));
        }
        Ok(self.eax())
    }
}

/// Converts an x87 80-bit extended value (little-endian, at least 10 bytes).
pub fn f80_to_f64(raw: &[u8]) -> f64 {
    let mant = u64::from_le_bytes(raw[0..8].try_into().unwrap());
    let se = u16::from_le_bytes([raw[8], raw[9]]);
    let sign = if se & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from(se & 0x7fff);
    if exp == 0 && mant == 0 {
        return sign * 0.0;
    }
    if exp == 0x7fff {
        return if mant << 1 == 0 {
            sign * f64::INFINITY
        } else {
            f64::NAN
        };
    }
    // value = mant * 2^(exp - 16383 - 63), with mant as an integer
    sign * (mant as f64) * 2f64.powi(exp - 16383 - 63)
}
