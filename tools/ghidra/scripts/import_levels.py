#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): renamed from decomp/scripts/import_overlays.py, paths updated
# (reads work/ghidra-import/levelNN.elf and extracted/boot/SCUS_971.99). See tools/ghidra/README.md.
import functools; print = functools.partial(print, flush=True)  # unbuffered progress when run in the background
"""Imports the boot ELF (extracted/boot/SCUS_971.99, as /SCUS_971.99, when the
project does not have it yet) and every level ELF (work/ghidra-import/levelNN.elf)
into the open Ghidra project, analyzes them, applies Lombyte's boot-match names
to the levels, and saves. Idempotent: skips programs that already exist.

    import_levels.py [NN ...]      default: all 19 levels (00..18)

Programs are named levelNN.elf under /levels. Overlay addresses overlap the
boot ELF by design, so each overlay is its own program. The level ELFs are the
level overlays with their original load addresses: today copies of the retired
C++ extractor's overlay.elf files; later `ratchet_clank-extract export --what code`
(a follow-up, docs/plan/repo_reorg.md).
"""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))  # repo root (tools/ghidra/scripts/)
IMPORT_DIR = os.path.join(ROOT, "work/ghidra-import")
BOOT_ELF = os.path.join(ROOT, "extracted/boot/SCUS_971.99")
NAMES = os.path.expanduser("~/Globals/Lombyte/config/overlays/us/names")
levels = [int(a) for a in sys.argv[1:]] or list(range(19))

def existing_programs():
    files = get("list_project_files", folder="/levels")
    return {f["name"] for f in files.get("files", [])} if isinstance(files, dict) else set()

def apply_names(program, level):
    path = os.path.join(NAMES, f"level-{level:02d}.json")
    if not os.path.exists(path):
        return 0
    funcs = {f["address"].lower(): f["name"] for f in get("list_functions", program=program, limit=10000)["functions"]}
    applied = 0
    for row in json.load(open(path))["functions"]:
        ev = row.get("name_evidence") or {}
        name = ev.get("name")
        addr = f"{row['addr']:08x}"
        if not name or funcs.get(addr, "").startswith("FUN_") is False:
            continue
        res = post("rename_function", old_name="0x" + addr, new_name=name, program=program, strict_mode="off")
        if isinstance(res, dict) and (res.get("success") is True or res.get("status") == "success"):
            post("set_comment", address="0x" + addr, comment=f"Lombyte {ev.get('method')}: boot 0x{ev.get('boot_addr', 0):08x}", type="plate", program=program)
            applied += 1
    return applied

def wait_for_analysis(name):
    while True:
        st = get("analysis_status", program=name)
        if isinstance(st, dict) and not st.get("analyzing", st.get("is_analyzing", False)):
            return
        time.sleep(5)

root_files = get("list_project_files", folder="/")
if "SCUS_971.99" not in ({f["name"] for f in root_files.get("files", [])} if isinstance(root_files, dict) else set()):
    t = time.time()
    res = post("import_file", file_path=BOOT_ELF, project_folder="/", auto_analyze=True)  # no language: see below
    if isinstance(res, dict) and res.get("success"):
        wait_for_analysis("SCUS_971.99")
        post("save_program", program="SCUS_971.99")
        print(f"SCUS_971.99: imported and analyzed in {time.time() - t:.0f}s")
    else:
        print(f"SCUS_971.99: import failed: {str(res)[:300]}")

have = existing_programs()
for lv in levels:
    name = f"level{lv:02d}.elf"
    if name in have:
        print(f"{name}: already in project, skipping import")
    else:
        t = time.time()
        res = post("import_file", file_path=os.path.join(IMPORT_DIR, name), project_folder="/levels", auto_analyze=True)  # no language: forces the ELF loader; the EE plugin picks r5900 from e_flags
        if not (isinstance(res, dict) and res.get("success")):
            print(f"{name}: import failed: {str(res)[:300]}")
            continue
        wait_for_analysis(name)
        print(f"{name}: imported and analyzed in {time.time() - t:.0f}s")
    n = apply_names(name, lv)
    post("save_program", program=name)
    post("close_program", name=name, save=True)
    print(f"{name}: applied {n} names, saved and closed")
print("done")
