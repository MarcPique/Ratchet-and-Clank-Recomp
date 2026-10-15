//! A small R5900 (EE) disassembler for the tools' reports (`overlay-diff` prints only the instructions that differ
//! between two overlays). Covers the integer, COP1 (single / word) and load/store instructions the game code uses;
//! anything else (MMI, COP0, COP2 macro ops) prints as its group and the raw word. Text only: nothing decodes back.

const GPR: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8",
    "t9", "k0", "k1", "gp", "sp", "fp", "ra",
];

fn r(i: u32) -> &'static str { GPR[(i & 31) as usize] }
fn simm(w: u32) -> i32 { (w & 0xffff) as u16 as i16 as i32 }
fn hex(v: i32) -> String { if v < 0 { format!("-0x{:x}", -(v as i64)) } else { format!("0x{v:x}") } }

/// The branch or jump target of `w` at `pc`, if `w` is one (not `jr` / `jalr`).
pub fn target(w: u32, pc: u32) -> Option<u32> {
    let op = w >> 26;
    let rs = (w >> 21) & 31;
    match op {
        2 | 3 => Some((pc.wrapping_add(4) & 0xf000_0000) | ((w & 0x03ff_ffff) << 2)),
        1 | 4..=7 | 0x14..=0x17 => Some(pc.wrapping_add(4).wrapping_add((simm(w) << 2) as u32)),
        0x11 if rs == 8 => Some(pc.wrapping_add(4).wrapping_add((simm(w) << 2) as u32)),
        _ => None,
    }
}

/// `w` at `pc` as text (`mnemonic operands`).
pub fn disasm(w: u32, pc: u32) -> String {
    let op = w >> 26;
    let (rs, rt, rd, sa, f) = ((w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, (w >> 6) & 31, w & 63);
    let tgt = || format!("0x{:x}", target(w, pc).unwrap_or(0));
    let imm = simm(w);
    let uimm = w & 0xffff;
    let mem = |m: &str| format!("{m} {}, {}({})", r(rt), hex(imm), r(rs));
    let fmem = |m: &str| format!("{m} $f{rt}, {}({})", hex(imm), r(rs));
    match op {
        0 => {
            if w == 0 { return "nop".into(); }
            let name = match f {
                0x00 => return format!("sll {}, {}, {sa}", r(rd), r(rt)),
                0x02 => return format!("srl {}, {}, {sa}", r(rd), r(rt)),
                0x03 => return format!("sra {}, {}, {sa}", r(rd), r(rt)),
                0x38 => return format!("dsll {}, {}, {sa}", r(rd), r(rt)),
                0x3a => return format!("dsrl {}, {}, {sa}", r(rd), r(rt)),
                0x3b => return format!("dsra {}, {}, {sa}", r(rd), r(rt)),
                0x3c => return format!("dsll32 {}, {}, {sa}", r(rd), r(rt)),
                0x3e => return format!("dsrl32 {}, {}, {sa}", r(rd), r(rt)),
                0x3f => return format!("dsra32 {}, {}, {sa}", r(rd), r(rt)),
                0x08 => return format!("jr {}", r(rs)),
                0x09 => return format!("jalr {}, {}", r(rd), r(rs)),
                0x0c => return "syscall".into(),
                0x0d => return "break".into(),
                0x0f => return "sync".into(),
                0x10 => return format!("mfhi {}", r(rd)),
                0x12 => return format!("mflo {}", r(rd)),
                0x11 => return format!("mthi {}", r(rs)),
                0x13 => return format!("mtlo {}", r(rs)),
                0x18..=0x1b => {
                    let m = ["mult", "multu", "div", "divu"][(f - 0x18) as usize];
                    return if rd != 0 { format!("{m} {}, {}, {}", r(rd), r(rs), r(rt)) } else { format!("{m} {}, {}", r(rs), r(rt)) };
                }
                0x04 => "sllv",
                0x06 => "srlv",
                0x07 => "srav",
                0x14 => "dsllv",
                0x16 => "dsrlv",
                0x17 => "dsrav",
                0x0a => "movz",
                0x0b => "movn",
                0x20 => "add",
                0x21 | 0x2d | 0x25 if rt == 0 => return format!("move {}, {}", r(rd), r(rs)),
                0x21 => "addu",
                0x22 => "sub",
                0x23 => "subu",
                0x24 => "and",
                0x25 => "or",
                0x26 => "xor",
                0x27 => "nor",
                0x2a => "slt",
                0x2b => "sltu",
                0x2c => "dadd",
                0x2d => "daddu",
                0x2e => "dsub",
                0x2f => "dsubu",
                _ => return format!("special.{f:#x} 0x{w:08x}"),
            };
            if matches!(f, 0x04 | 0x06 | 0x07 | 0x14 | 0x16 | 0x17) { format!("{name} {}, {}, {}", r(rd), r(rt), r(rs)) } else { format!("{name} {}, {}, {}", r(rd), r(rs), r(rt)) }
        }
        1 => {
            let m = match rt { 0 => "bltz", 1 => "bgez", 2 => "bltzl", 3 => "bgezl", 0x10 => "bltzal", 0x11 => "bgezal", _ => return format!("regimm 0x{w:08x}") };
            format!("{m} {}, {}", r(rs), tgt())
        }
        2 => format!("j {}", tgt()),
        3 => format!("jal {}", tgt()),
        4 if rs == 0 && rt == 0 => format!("b {}", tgt()),
        4 | 5 | 0x14 | 0x15 => format!("{} {}, {}, {}", ["beq", "bne"][(op & 1) as usize].to_string() + if op >= 0x14 { "l" } else { "" }, r(rs), r(rt), tgt()),
        6 | 7 | 0x16 | 0x17 => format!("{} {}, {}", ["blez", "bgtz"][(op & 1) as usize].to_string() + if op >= 0x16 { "l" } else { "" }, r(rs), tgt()),
        8 => format!("addi {}, {}, {}", r(rt), r(rs), hex(imm)),
        9 => format!("addiu {}, {}, {}", r(rt), r(rs), hex(imm)),
        0x18 => format!("daddi {}, {}, {}", r(rt), r(rs), hex(imm)),
        0x19 => format!("daddiu {}, {}, {}", r(rt), r(rs), hex(imm)),
        0x0a => format!("slti {}, {}, {}", r(rt), r(rs), hex(imm)),
        0x0b => format!("sltiu {}, {}, {}", r(rt), r(rs), hex(imm)),
        0x0c => format!("andi {}, {}, 0x{uimm:x}", r(rt), r(rs)),
        0x0d => format!("ori {}, {}, 0x{uimm:x}", r(rt), r(rs)),
        0x0e => format!("xori {}, {}, 0x{uimm:x}", r(rt), r(rs)),
        0x0f => format!("lui {}, 0x{uimm:x}", r(rt)),
        0x20 => mem("lb"),
        0x21 => mem("lh"),
        0x22 => mem("lwl"),
        0x23 => mem("lw"),
        0x24 => mem("lbu"),
        0x25 => mem("lhu"),
        0x26 => mem("lwr"),
        0x27 => mem("lwu"),
        0x28 => mem("sb"),
        0x29 => mem("sh"),
        0x2a => mem("swl"),
        0x2b => mem("sw"),
        0x2c => mem("sdl"),
        0x2d => mem("sdr"),
        0x2e => mem("swr"),
        0x1a => mem("ldl"),
        0x1b => mem("ldr"),
        0x1e => mem("lq"),
        0x1f => mem("sq"),
        0x37 => mem("ld"),
        0x3f => mem("sd"),
        0x2f => format!("cache 0x{rt:x}, {}({})", hex(imm), r(rs)),
        0x31 => fmem("lwc1"),
        0x39 => fmem("swc1"),
        0x36 => format!("lqc2 $vf{rt}, {}({})", hex(imm), r(rs)),
        0x3e => format!("sqc2 $vf{rt}, {}({})", hex(imm), r(rs)),
        0x11 => cop1(w, pc),
        0x10 => format!("cop0 0x{w:08x}"),
        0x12 => format!("cop2 0x{w:08x}"),
        0x1c => format!("mmi.{f:#x} {}, {}, {} (0x{w:08x})", r(rd), r(rs), r(rt)),
        _ => format!(".word 0x{w:08x}"),
    }
}

fn cop1(w: u32, pc: u32) -> String {
    let (fmt, ft, fs, fd, f) = ((w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, (w >> 6) & 31, w & 63);
    match fmt {
        0 => format!("mfc1 {}, $f{fs}", r(ft)),
        2 => format!("cfc1 {}, $fcr{fs}", r(ft)),
        4 => format!("mtc1 {}, $f{fs}", r(ft)),
        6 => format!("ctc1 {}, $fcr{fs}", r(ft)),
        8 => {
            let m = ["bc1f", "bc1t", "bc1fl", "bc1tl"][(ft & 3) as usize];
            format!("{m} 0x{:x}", target(w, pc).unwrap_or(0))
        }
        0x10 => {
            let three = |m: &str| format!("{m} $f{fd}, $f{fs}, $f{ft}");
            let two = |m: &str| format!("{m} $f{fd}, $f{fs}");
            let acc = |m: &str| format!("{m} $f{fs}, $f{ft}");
            match f {
                0x00 => three("add.s"),
                0x01 => three("sub.s"),
                0x02 => three("mul.s"),
                0x03 => three("div.s"),
                0x04 => format!("sqrt.s $f{fd}, $f{ft}"),
                0x05 => two("abs.s"),
                0x06 => two("mov.s"),
                0x07 => two("neg.s"),
                0x16 => three("rsqrt.s"),
                0x18 => acc("adda.s"),
                0x19 => acc("suba.s"),
                0x1a => acc("mula.s"),
                0x1c => three("madd.s"),
                0x1d => three("msub.s"),
                0x1e => acc("madda.s"),
                0x1f => acc("msuba.s"),
                0x24 => two("cvt.w.s"),
                0x28 => three("max.s"),
                0x29 => three("min.s"),
                0x30 => acc("c.f.s"),
                0x32 => acc("c.eq.s"),
                0x34 => acc("c.lt.s"),
                0x36 => acc("c.le.s"),
                _ => format!("cop1.s.{f:#x} 0x{w:08x}"),
            }
        }
        0x14 if f == 0x20 => format!("cvt.s.w $f{fd}, $f{fs}"),
        _ => format!("cop1 0x{w:08x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_instructions() {
        assert_eq!(disasm(0x27bd_ffd0, 0), "addiu sp, sp, -0x30");
        assert_eq!(disasm(0x3c02_0014, 0), "lui v0, 0x14");
        assert_eq!(disasm(0x8c42_13d4, 0), "lw v0, 0x13d4(v0)");
        assert_eq!(disasm(0x0c08_c7e6, 0x230000), "jal 0x231f98");
        assert_eq!(disasm(0x1040_0003, 0x1000), "beq v0, zero, 0x1010");
        assert_eq!(disasm(0x0080_102d, 0), "move v0, a0");
        assert_eq!(disasm(0x4600_0802, 0), "mul.s $f0, $f1, $f0");
        assert_eq!(disasm(0xc7a0_0010, 0), "lwc1 $f0, 0x10(sp)");
        assert_eq!(target(0x4501_0002, 0x100), Some(0x10c));
    }
}
