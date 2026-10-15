#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Clusters functions across the boot ELF and all 19 level overlays by a
relocation-tolerant hash of their MIPS code, so shared engine code is
identified once and boot-ELF names propagate to every overlay.

Function starts come from Ghidra (list_functions); bodies are taken from the
raw bytes we extracted (boot ELF, level overlay lumps), with the next function start as the end. The hash
masks the fields that differ between links of the same source: jal/j targets,
lui immediates, and $gp-relative offsets.

Outputs (tools/ghidra/names/):
  clusters.tsv      one row per hash: n_programs, boot_name, members
  overlay_names.csv address/name pairs per level to apply to Ghidra
"""
import hashlib, os, struct, sys, csv, collections
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))  # repo root (tools/ghidra/scripts/)
programs = ["SCUS_971.99"] + [f"level{i:02d}.elf" for i in range(19)]

def elf_text(path):
    """Returns {addr: bytes} for every PROGBITS section with an address."""
    d = open(path, "rb").read()
    shoff, = struct.unpack_from("<I", d, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", d, 0x2e)
    out = {}
    for i in range(shnum):
        typ, flags, addr, off, size = struct.unpack_from("<IIIII", d, shoff + i * shentsize + 4)
        if typ == 1 and addr:
            out[addr] = d[off:off + size]
    return out

def overlay_text(path):
    """{addr: bytes} of every PROGBITS section of a raw level overlay lump (extracted/levels/NN/overlay.bin):
    16-byte headers `dest, size, type, entry`, each followed by `size` bytes, until the entry point changes
    (rc_formats::font::parse_overlay_sections)."""
    d = open(path, "rb").read()
    out, pos, entry = {}, 0, None
    while pos + 16 <= len(d):
        dest, size, typ, ep = struct.unpack_from("<IIII", d, pos)
        if entry is None:
            entry = ep
        elif ep != entry:
            break
        if typ == 1 and dest:
            out[dest] = d[pos + 16:pos + 16 + size]
        pos += 16 + size
    return out

def read_words(sections, start, end):
    for base, blob in sections.items():
        if base <= start < base + len(blob):
            end = min(end, base + len(blob))
            return blob[start - base:end - base]
    return b""

MEM_OPS = {0x08, 0x09, 0x0D, 0x0C, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2E,
           0x31, 0x35, 0x39, 0x3D, 0x1A, 0x1B, 0x1E, 0x1F, 0x36, 0x37, 0x3E, 0x3F, 0x33, 0x2F}

def norm_hash(code):
    """Hash of the code with link-dependent fields masked: j/jal targets, lui
    immediates, $gp-relative offsets, and the 16-bit immediate of any I-type
    instruction whose base register was loaded by a lui earlier in the function
    (the classic lui/addiu and lui/lw address pairs)."""
    h = hashlib.sha1()
    hi_regs = set()
    for i in range(0, len(code) - len(code) % 4, 4):
        w, = struct.unpack_from("<I", code, i)
        op = w >> 26
        rs = (w >> 21) & 31
        rt = (w >> 16) & 31
        if op in (2, 3):
            w &= 0xFC000000
        elif op == 0x0F:
            w &= 0xFFFF0000
            hi_regs.add(rt)
        elif op in MEM_OPS:
            if rs == 28 or rs in hi_regs:
                w &= 0xFFFF0000
            if op in (0x08, 0x09, 0x0D, 0x0C) and rt in hi_regs and rs != rt:
                hi_regs.discard(rt)   # register redefined from something else
        elif op == 0 and ((w >> 11) & 31) in hi_regs and (w & 0x3F) not in (0x08, 0x09):
            hi_regs.discard((w >> 11) & 31)  # R-type overwrote the register
        h.update(struct.pack("<I", w))
    return h.hexdigest()

clusters = collections.defaultdict(list)   # hash -> [(program, addr, name, size)]
names_by_prog = {}
for prog in programs:
    if prog == "SCUS_971.99":
        sections = elf_text(os.path.join(ROOT, "extracted/boot/SCUS_971.99"))
    else:
        sections = overlay_text(os.path.join(ROOT, f"extracted/levels/{prog[5:7]}/overlay.bin"))
    if prog != "SCUS_971.99":
        post("open_program", path=f"/levels/{prog}")
    funcs = get("list_functions", program=prog, limit=20000)
    if prog != "SCUS_971.99":
        post("close_program", name=prog, save=False)
    if not isinstance(funcs, dict) or "functions" not in funcs:
        print(f"{prog}: not available ({str(funcs)[:100]}), skipping")
        continue
    fl = sorted((int(f["address"], 16), f["name"]) for f in funcs["functions"])
    names_by_prog[prog] = dict(fl)
    for k, (addr, name) in enumerate(fl):
        end = fl[k + 1][0] if k + 1 < len(fl) else addr + 0x10000
        code = read_words(sections, addr, end)
        if len(code) < 16:
            continue
        clusters[norm_hash(code)].append((prog, addr, name, len(code)))
    print(f"{prog}: {len(fl)} functions hashed")

os.makedirs(os.path.join(ROOT, "tools/ghidra/names"), exist_ok=True)
overlay_names = []
n_prog = collections.Counter()
with open(os.path.join(ROOT, "tools/ghidra/names/clusters.tsv"), "w") as f:
    f.write("hash\tn_programs\tsize\tboot_name\tmembers\n")
    for h, members in sorted(clusters.items(), key=lambda kv: -len({m[0] for m in kv[1]})):
        progs = {m[0] for m in members}
        n_prog[len(progs)] += 1
        boot = [m for m in members if m[0] == "SCUS_971.99" and not m[2].startswith("FUN_")]
        boot_name = boot[0][2] if boot else ""
        f.write(f"{h}\t{len(progs)}\t{members[0][3]}\t{boot_name}\t" + " ".join(f"{m[0]}:{m[1]:08x}" for m in members) + "\n")
        if boot_name and len(boot) == 1:
            for m in members:
                if m[0] != "SCUS_971.99" and m[2].startswith("FUN_"):
                    overlay_names.append({"program": m[0], "address": f"{m[1]:08x}", "name": boot_name, "source": f"boot-hash-match {boot[0][1]:08x}"})
with open(os.path.join(ROOT, "tools/ghidra/names/overlay_names.csv"), "w") as f:
    w = csv.DictWriter(f, fieldnames=["program", "address", "name", "source"]); w.writeheader(); w.writerows(overlay_names)
print("clusters by number of programs sharing them:", sorted(n_prog.items()))
print(f"overlay functions nameable from boot matches: {len(overlay_names)}")
