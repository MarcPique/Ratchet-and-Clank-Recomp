#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Applies Lombyte-derived function names to the boot ELF program open in Ghidra.

Reads tools/ghidra/names/boot_functions.csv (address, name, path, semantic). Renames
each FUN_ function at `address` to `name` and records the source path and any
semantic name as a plate comment. Existing non-FUN_ names are left alone.
"""
import csv, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

PROGRAM = "SCUS_971.99"
dry = "--dry-run" in sys.argv
limit = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None

funcs = {f["address"].lower(): f["name"] for f in get("list_functions", limit=10000)["functions"]}
rows = list(csv.DictReader(open(os.path.join(os.path.dirname(__file__), "../names/boot_functions.csv"))))
renamed = skipped_missing = skipped_named = failed = 0
for i, r in enumerate(rows[:limit]):
    addr = r["address"].lower()
    cur = funcs.get(addr)
    if cur is None:
        skipped_missing += 1
        continue
    if not cur.startswith("FUN_"):
        skipped_named += 1
        continue
    comment = f"Lombyte: {r['path']}" + (f" ; semantic: {r['semantic']}" if r["semantic"] else "")
    if dry:
        print(f"{addr} {cur} -> {r['name']}  [{comment}]")
        renamed += 1
        continue
    res = post("rename_function", old_name="0x" + addr, new_name=r["name"], program=PROGRAM, strict_mode="off")
    ok = isinstance(res, dict) and (res.get("success") is True or res.get("status") == "success")
    if not ok:
        failed += 1
        print(f"FAIL {addr} -> {r['name']}: {str(res)[:200]}")
        continue
    post("set_comment", address="0x" + addr, comment=comment, type="plate", program=PROGRAM)
    renamed += 1
    if renamed % 100 == 0:
        print(f"...{renamed} renamed")
print(f"renamed {renamed}, no function at address {skipped_missing}, already named {skipped_named}, failed {failed}")
