#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Applies names from tools/ghidra/names/overlay_names.csv (produced by
overlay_diff.py) to the overlay programs in Ghidra. Only FUN_ functions are
renamed; a plate comment records the boot function the name came from."""
import csv, os, sys, collections, functools
print = functools.partial(print, flush=True)
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

rows = list(csv.DictReader(open(os.path.join(os.path.dirname(__file__), "../names/overlay_names.csv"))))
by_prog = collections.defaultdict(list)
for r in rows:
    by_prog[r["program"]].append(r)
only = set(sys.argv[1:])
for prog, prow in sorted(by_prog.items()):
    if only and prog not in only:
        continue
    post("open_program", path=f"/levels/{prog}")
    funcs = {f["address"].lower(): f["name"] for f in get("list_functions", program=prog, limit=20000)["functions"]}
    done = skipped = failed = 0
    for r in prow:
        cur = funcs.get(r["address"].lower(), "")
        if not cur.startswith("FUN_"):
            skipped += 1
            continue
        res = post("rename_function", old_name="0x" + r["address"], new_name=r["name"], program=prog, strict_mode="off")
        if isinstance(res, dict) and (res.get("success") is True or res.get("status") == "success"):
            post("set_comment", address="0x" + r["address"], comment=r["source"], type="plate", program=prog)
            done += 1
        else:
            failed += 1
    post("save_program", program=prog)
    post("close_program", name=prog, save=True)
    print(f"{prog}: renamed {done}, already named {skipped}, failed {failed}")
