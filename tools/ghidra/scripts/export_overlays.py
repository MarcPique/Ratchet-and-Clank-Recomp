#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Exports decompiled C for the level overlays without decompiling the shared
engine nineteen times.

* The reference overlay (default level01.elf, Novalis) is exported in full.
* Every other overlay exports only the functions whose cluster (from
  tools/ghidra/names/clusters.tsv) has no member in the reference overlay.

Output: work/decomp/<program>/ as with export_decomp.py.
"""
import os, sys, re, functools
print = functools.partial(print, flush=True)
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))  # repo root (tools/ghidra/scripts/)
ref = sys.argv[1] if len(sys.argv) > 1 else "level01.elf"
only = set(sys.argv[2:])

# addr -> covered-by-reference, per program
covered = {}
for line in open(os.path.join(ROOT, "tools/ghidra/names/clusters.tsv")).read().splitlines()[1:]:
    members = line.split("\t")[4].split()
    in_ref = any(m.startswith(ref + ":") for m in members)
    for m in members:
        prog, addr = m.split(":")
        covered[(prog, addr)] = in_ref

def export(prog, selector):
    out = os.path.join(ROOT, "work/decomp", prog)
    os.makedirs(out, exist_ok=True)
    post("open_program", path=f"/levels/{prog}")
    funcs = get("list_functions", program=prog, limit=20000)["functions"]
    chosen = [f for f in funcs if selector(f["address"].lower())]
    index = []
    for i, f in enumerate(chosen):
        addr, name = f["address"].lower(), f["name"]
        res = get("decompile_function", address="0x" + addr, program=prog, timeout=120)
        text = res.get("decompiled") if isinstance(res, dict) else None
        if not text:
            text = "// decompile failed: " + str(res)[:300]
        fname = f"{addr}_{re.sub(r'[^A-Za-z0-9_]', '_', name)}.c"
        with open(os.path.join(out, fname), "w") as fh:
            fh.write(f"// {prog} {addr} {name}\n{text}\n")
        index.append(f"{addr}\t{name}\t{fname}")
        if (i + 1) % 200 == 0:
            print(f"  {prog}: {i + 1}/{len(chosen)}")
    with open(os.path.join(out, "index.tsv"), "w") as fh:
        fh.write("\n".join(index) + "\n")
    post("close_program", name=prog, save=False)
    print(f"{prog}: exported {len(chosen)} of {len(funcs)} functions")

progs = [f"level{i:02d}.elf" for i in range(19)]
if not only or ref in only:
    export(ref, lambda a: True)
for prog in progs:
    if prog == ref or (only and prog not in only):
        continue
    export(prog, lambda a, p=prog: not covered.get((p, a), False))
