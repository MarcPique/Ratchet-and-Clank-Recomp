#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Applies the names our own notes (docs/plan/*.md, docs/formats/*.md) give to
functions and globals, recorded in tools/ghidra/names/doc_names.csv, to the Ghidra
project, and propagates function names to every overlay through the
function-hash clusters in tools/ghidra/names/clusters.tsv.

    apply_doc_names.py --dry-run          plan only (default); writes the plan log
    apply_doc_names.py --apply            rename / comment, then save the programs
    apply_doc_names.py --apply --export   also re-export the touched functions
    options: --no-propagate, --programs boot level01 ..., --log PATH

Rules (Lombyte names win):
* A function whose current name is a default (FUN_, fun_, func_, thunk_, LAB_)
  is renamed to the doc name and gets a plate line "doc name: <name> (<doc>)".
* A function that already has a name (Lombyte-derived or applied earlier) is
  NOT renamed. If the doc name differs from it, the plate gets
  "doc alias: <name> (<doc>)". Equal names (ignoring case, underscores and a
  C++ mangling suffix) are left alone.
* Plate lines are appended to the existing plate comment, never replace it,
  and are not added twice, so the script is idempotent.
* Cluster propagation: a doc-named function whose hash cluster has at most one
  member per program passes its name to the other members (same rules), unless
  that member has its own row in doc_names.csv.
* Data rows: a DAT_/PTR_/unlabelled address gets a label (create_label; the
  user label becomes the primary name); a named one gets the alias as a plate
  comment. Data is never propagated (addresses differ per program).
* A fn row at an address Ghidra has not made a function (code reached only
  through pointer tables) gets a label, not a new function.

Every program is addressed by its project path (/SCUS_971.99,
/levels/levelNN.elf): the project also holds stale raw imports under
/overlays/ with the same file names. Programs this script opens are closed
again, and the active program is restored.
"""
import argparse, collections, csv, functools, json, os, re, sys
print = functools.partial(print, flush=True)
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))  # repo root (tools/ghidra/scripts/)
DEFAULT_PREFIXES = ("FUN_", "fun_", "func_", "thunk_", "LAB_")
CONF_RANK = {"verified": 0, "inferred": 1, "suggested": 2}

ap = argparse.ArgumentParser()
ap.add_argument("--apply", action="store_true")
ap.add_argument("--dry-run", action="store_true")
ap.add_argument("--export", action="store_true")
ap.add_argument("--no-propagate", action="store_true")
ap.add_argument("--sync-export", action="store_true",
                help="only re-export work/decomp entries whose name no longer matches Ghidra, then exit")
ap.add_argument("--programs", nargs="*", default=None, help="boot, level01, ... (default: all)")
ap.add_argument("--csv", default=os.path.join(ROOT, "tools/ghidra/names/doc_names.csv"))
ap.add_argument("--log", default=os.path.join(ROOT, "work/decomp/doc_names_apply.tsv"))
args = ap.parse_args()
APPLY = args.apply and not args.dry_run


def path_of(prog):
    """doc_names.csv program ('boot', 'level01') or cluster program
    ('SCUS_971.99', 'level01.elf') -> Ghidra project path."""
    if prog in ("boot", "SCUS_971.99"):
        return "/SCUS_971.99"
    m = re.fullmatch(r"level(\d\d)(\.elf)?", prog)
    return f"/levels/level{m.group(1)}.elf" if m else None


def short(path):
    return "boot" if path == "/SCUS_971.99" else path.split("/")[-1].replace(".elf", "")


def export_dir(path):
    return os.path.join(ROOT, "work/decomp", "SCUS_971.99" if path == "/SCUS_971.99" else path.split("/")[-1])


def norm(name):
    name = re.sub(r"__F\w*$", "", name)
    return re.sub(r"[^a-z0-9]", "", name.lower())


def snake(name):
    if "__F" in name or re.fullmatch(r"[a-z0-9_]+", name):
        return name
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)
    return re.sub(r"_+", "_", s.lower())


def convert(row, target):
    """Name for `target` following that program's convention: snake_case in
    the boot ELF (Lombyte's file names), the doc's PascalCase in overlays."""
    if target == "/SCUS_971.99" and row["program"] != target:
        return snake(row["new"])
    if target != "/SCUS_971.99" and row["program"] == "/SCUS_971.99":
        m = re.match(r"doc name (\S+)", row.get("note", ""))
        if m:
            return m.group(1)
    return row["new"]


def is_default(name):
    return name.startswith(DEFAULT_PREFIXES) or re.fullmatch(r"(DAT|PTR|UNK|s|u)_[0-9A-Fa-f]+.*", name or "") is not None


def ok(res):
    return isinstance(res, dict) and (res.get("success") is True or res.get("status") == "success")


def src_label(row):
    return row["source_doc"].split(":")[0]


def reexport(path, addrs, full):
    """Re-decompile `addrs` ({addr: current name}) of `path` into its
    work/decomp directory, replacing the old file and index line."""
    out = export_dir(path)
    idx_path = os.path.join(out, "index.tsv")
    if not os.path.exists(idx_path):
        return 0
    index = [l.split("\t") for l in open(idx_path).read().splitlines() if l]
    pos = {row[0]: i for i, row in enumerate(index)}
    n = 0
    for a, new in sorted(addrs.items()):
        if a not in pos and not full:
            continue          # selective overlay export: shared code lives in level01's export
        res = get("decompile_function", address="0x" + a, program=path, timeout=120)
        text = res.get("decompiled") or res.get("code") if isinstance(res, dict) else res
        if not isinstance(text, str) or not text.strip():
            text = "// decompile failed: " + str(res)[:300]
        fname = f"{a}_{re.sub(r'[^A-Za-z0-9_]', '_', new)}.c"
        if a in pos:
            oldf = index[pos[a]][2]
            if oldf != fname and os.path.exists(os.path.join(out, oldf)):
                os.remove(os.path.join(out, oldf))
            index[pos[a]] = [a, new, fname]
        else:
            index.append([a, new, fname])
        with open(os.path.join(out, fname), "w") as fh:
            fh.write(f"// {os.path.basename(out)} {a} {new}\n{text}\n")
        n += 1
    index.sort(key=lambda r: r[0])
    with open(idx_path, "w") as fh:
        fh.write("\n".join("\t".join(r) for r in index) + "\n")
    return n


if args.sync_export:
    current_before = get("get_current_program_info").get("path")
    open_now = {p["path"] for p in get("list_open_programs")["programs"]}
    for path in ["/SCUS_971.99"] + [f"/levels/level{i:02d}.elf" for i in range(19)]:
        idx = os.path.join(export_dir(path), "index.tsv")
        if not os.path.exists(idx):
            continue
        if path not in open_now:
            get("open_program", path=path)
        cur = {f["address"].lower(): f["name"] for f in get("list_functions", program=path, limit=20000)["functions"]}
        stale = {}
        for l in open(idx).read().splitlines():
            a, name = l.split("\t")[:2]
            if a in cur and cur[a] != name:
                stale[a] = cur[a]
        n = reexport(path, stale, False)
        if path not in open_now:
            post("close_program", name=path, save=True)
        print(f"{path}: {len(stale)} stale export entries, re-exported {n}")
    if current_before:
        get("switch_program", program=current_before)
    sys.exit(0)

# ---------------------------------------------------------------- inputs
rows = [r for r in csv.DictReader(open(args.csv)) if r["program"] != "unknown"]
want = {path_of(p) for p in args.programs} if args.programs else None

clusters = {}        # (path, addr) -> list of (path, addr)
cluster_id = {}
for line in open(os.path.join(ROOT, "tools/ghidra/names/clusters.tsv")).read().splitlines()[1:]:
    h, _, _, _, members = line.split("\t")
    mem = [(path_of(m.split(":")[0]), m.split(":")[1]) for m in members.split()]
    per_prog = collections.Counter(p for p, _ in mem)
    if max(per_prog.values()) > 1:
        continue           # same code twice in one program: too generic to name by hash
    for m in mem:
        clusters[m] = mem
        cluster_id[m] = h[:8]

open_before = {p["path"] for p in get("list_open_programs")["programs"]}
current_before = get("get_current_program_info").get("path")
opened = []
funcs = {}


def functions(path):
    if path not in funcs:
        if path not in open_before and path not in opened:
            r = get("open_program", path=path)
            if not ok(r):
                print(f"cannot open {path}: {str(r)[:200]}")
                funcs[path] = None
                return None
            opened.append(path)
        res = get("list_functions", program=path, limit=20000)
        funcs[path] = {f["address"].lower(): f["name"] for f in res["functions"]}
    return funcs[path]


# ---------------------------------------------------------------- plan
plan = []            # dicts: program, address, kind, old, new, action, comment, source, detail
explicit = set()
for r in rows:
    path = path_of(r["program"])
    if path is None or (want and path not in want):
        continue
    addr = r["address"].lower().zfill(8)
    explicit.add((path, addr))
    base = dict(program=path, address=addr, kind=r["kind"], new=r["name"], source=src_label(r),
                confidence=r["confidence"], note=r["note"], origin="doc")
    if r["kind"] == "fn":
        fl = functions(path)
        if fl is None:
            continue
        if addr not in fl:
            info = get("get_function_by_address", address="0x" + addr, program=path)
            if isinstance(info, dict) and "name" in info:
                plan.append(dict(base, old="", action="not-fn-start", detail=f"inside {info['name']}@{info['address']}"))
            else:   # code Ghidra never made a function (reached only through pointer tables): label it
                plan.append(dict(base, old="", action="label", data_type="undefined", detail="no function defined here",
                                 comment=f"doc name: {base['new']} ({base['source']}, {base['confidence']}; code not yet a function in Ghidra)"))
            continue
        base["old"] = fl[addr]
    else:
        info = get("can_rename_at_address", address="0x" + addr, program=path)
        mem = get("read_memory", address="0x" + addr, length=1, program=path)
        if not (isinstance(mem, dict) and "data" in mem):
            plan.append(dict(base, old="", action="no-memory", detail="address not mapped in this program"))
            continue
        if isinstance(info, dict) and info.get("type") == "function":
            base["kind"] = "fn"
        base["old"] = (info or {}).get("current_name", "") if isinstance(info, dict) else ""
        base["data_type"] = info.get("type") if isinstance(info, dict) else "?"
    plan.append(base)

# cluster propagation
if not args.no_propagate:
    cand = collections.defaultdict(list)
    for p in plan:
        if p["kind"] != "fn" or p.get("action") == "not-fn-start":
            continue
        for (p2, a2) in clusters.get((p["program"], p["address"]), []):
            if (p2, a2) == (p["program"], p["address"]) or (p2, a2) in explicit:
                continue
            if want and p2 not in want:
                continue
            cand[(p2, a2)].append(p)
    for (p2, a2), srcs in sorted(cand.items()):
        names = {convert(s, p2) for s in srcs}
        srcs.sort(key=lambda s: (CONF_RANK.get(s["confidence"], 3), s["program"] != "/levels/level01.elf", s["program"]))
        best = srcs[0]
        fl = functions(p2)
        if fl is None or a2 not in fl:
            continue
        detail = f"cluster {cluster_id[(p2, a2)]} from {short(best['program'])} 0x{best['address']}"
        if len(names) > 1:
            detail += " ; other doc names: " + ", ".join(sorted(names - {convert(best, p2)}))
        plan.append(dict(program=p2, address=a2, kind="fn", old=fl[a2], new=convert(best, p2), source=best["source"],
                         confidence=best["confidence"], note=best["note"], origin="cluster", detail=detail))

# decide actions
for p in plan:
    if p.get("action"):
        continue
    old, new = p["old"], p["new"]
    lombyte_wrong = "Lombyte name believed wrong" in p.get("note", "")
    via = f"; via {p['detail']}" if p["origin"] == "cluster" else ""
    if old and not is_default(old):
        if norm(old) == norm(new):
            p["action"] = "same"
        else:
            p["action"] = "alias"
            p["comment"] = f"doc alias: {new} ({p['source']}{via})" + (" -- Lombyte name believed wrong" if lombyte_wrong else "")
    else:
        p["action"] = "rename" if p["kind"] == "fn" else ("label" if p.get("data_type") == "undefined" else "rename-data")
        p["comment"] = f"doc name: {new} ({p['source']}, {p['confidence']}{via})"

# ---------------------------------------------------------------- execute
os.makedirs(os.path.dirname(args.log), exist_ok=True)
touched = collections.defaultdict(dict)    # path -> {addr: (old, new_name_now)}
counts = collections.Counter()


def append_plate(path, addr, line):
    cur = get("get_comment", address="0x" + addr, program=path)
    plate = cur.get("plate") if isinstance(cur, dict) else None
    if plate and line in plate:
        return True
    text = (plate + "\n" + line) if plate else line
    return ok(post("set_comment", address="0x" + addr, comment=text, type="plate", program=path))


for p in plan:
    a, path, act = p["address"], p["program"], p["action"]
    res = "planned"
    if APPLY and act in ("rename", "rename-data", "label", "alias"):
        good = True
        if act == "rename":
            r = post("rename_function", old_name="0x" + a, new_name=p["new"], program=path, strict_mode="off")
            good = ok(r)
        else:   # label / rename-data: a user label outranks the DAT_/PTR_ default name.
            # (rename_symbol would run the plugin's Hungarian-prefix gate, which has no strict_mode override.)
            r = post("create_label", address="0x" + a, name=p["new"], program=path)
            good = ok(r) or "already exists" in str(r)
        if good:
            good = append_plate(path, a, p["comment"])
            res = "done" if good else "comment-failed"
        else:
            res = "failed: " + str(r)[:150]
        if good and p["kind"] == "fn" and act in ("rename", "alias"):
            touched[path][a] = (p["old"], p["new"] if act == "rename" else p["old"])
    counts[(short(path), p["origin"], act, res.split(":")[0])] += 1
    p["result"] = res
    if APPLY and sum(counts.values()) % 500 == 0:
        print(f"...{sum(counts.values())}/{len(plan)}")

with open(args.log, "w") as fh:
    cols = ["program", "address", "kind", "old", "new", "action", "origin", "source", "confidence", "detail", "comment", "result"]
    fh.write("\t".join(cols) + "\n")
    for p in sorted(plan, key=lambda p: (p["program"], p["address"], p["origin"])):
        fh.write("\t".join(str(p.get(c, "")).replace("\t", " ").replace("\n", " ") for c in cols) + "\n")

if not APPLY:
    for p in sorted(plan, key=lambda p: (p["program"], p["address"])):
        if p["action"] in ("rename", "rename-data", "label", "alias", "not-fn-start", "no-memory"):
            print(f"{short(p['program']):8} {p['address']} {p['old'] or '-':32} {p['action']:12} {p['new']}  [{p.get('detail', p['source'])}]")

print("\nsummary (program, origin, action, result): count")
for k, v in sorted(counts.items()):
    print("  ", *k, v)

if APPLY:
    for path in sorted({p["program"] for p in plan if p.get("result") == "done"}):
        r = post("save_program", program=path)
        print(f"save {path}: {'ok' if ok(r) else r}")

    if args.export:
        for path, addrs in sorted(touched.items()):
            n = reexport(path, {a: new for a, (old, new) in addrs.items()}, path in ("/SCUS_971.99", "/levels/level01.elf"))
            print(f"export {path}: re-exported {n} functions")

for path in opened:
    post("close_program", name=path, save=True)
if current_before:
    get("switch_program", program=current_before)
print(f"log: {args.log}")
