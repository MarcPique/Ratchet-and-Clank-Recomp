# tools/ghidra: Ghidra scripts and name tables

Dev only, never ships. Workflow: `docs/workflows/ghidra.md`. Conventions and the history of the naming runs:
`docs/plan/decomp_workflow.md`.

**Status.** Moved here from `decomp/scripts/` and `decomp/names/` in the repo reorg (2026-09-27). The scripts have
**not been re-run since**: only their paths were updated (each says so in its header). The first run after the reorg
should be a `--dry-run` where a script has one.

## What is here

| Path | What it is |
|---|---|
| `scripts/ghidra_http.py` | GET/POST client for the GhidraMCP plugin's HTTP API (`127.0.0.1:8089`, Ghidra GUI open) |
| `scripts/import_levels.py [NN ...]` | Imports `extracted/boot/SCUS_971.99` as `/SCUS_971.99` (if the project lacks it) and `work/ghidra-import/levelNN.elf` into `/levels`, waits for analysis, applies Lombyte's boot-match names, saves. Skips programs that exist. |
| `scripts/apply_boot_names.py [--dry-run]` | Boot ELF names from `names/boot_functions.csv` (Lombyte), source path as a plate comment |
| `scripts/overlay_diff.py` | Clusters functions of the boot ELF and the 19 overlays by a relocation-tolerant code hash; writes `names/clusters.tsv` and `names/overlay_names.csv` (for naming; to ask whether a function differs between levels, and how, use `rc-trace overlay-diff`: docs/workflows/ghidra.md "Masked overlay diff") |
| `scripts/apply_overlay_names.py [levelNN.elf ...]` | Applies `names/overlay_names.csv` to the level programs |
| `scripts/apply_doc_names.py --dry-run / --apply [--export]` | Applies the names our docs give (`names/doc_names.csv`) and propagates them through `names/clusters.tsv`; log in `work/decomp/doc_names_apply.tsv` |
| `scripts/export_decomp.py <program>` | Decompiled C of every function to `work/decomp/<program>/` (+ `index.tsv`) |
| `scripts/export_overlays.py [ref] [levelNN.elf ...]` | level01 in full, other levels only their non-shared functions, to `work/decomp/levelNN.elf/` |
| `names/*.csv`, `names/clusters.tsv` | The name tables (source of truth for names; committed) |
| `names/census_systems.tsv` | System tags (system, has / partly / missing) of the shared functions the unported classes call; read by `rc-trace class-census` (docs/plan/class_census.md) |

## Inputs and outputs

* **Inputs:** the open Ghidra project (today `~/ratchet1.gpr` + `~/ratchet1.rep`; target home
  `~/PS2/ratchet1/ghidra/`), `names/`, `extracted/boot/SCUS_971.99` and `extracted/levels/NN/overlay.bin` (game data),
  `work/ghidra-import/levelNN.elf` (level ELFs with their original load addresses; today copies of the retired C++
  extractor's `overlay.elf`, later `ratchet_clank-extract export --what code`, a follow-up), Lombyte's names under
  `~/Globals/Lombyte`.
* **Outputs:** renames and comments in the Ghidra project, `names/clusters.tsv` + `names/overlay_names.csv`
  (`overlay_diff.py`, committed), the decompiler export in `work/decomp/` (git-ignored: it is the game's code).

Run from the repo root, e.g. `python3 tools/ghidra/scripts/export_decomp.py SCUS_971.99`.
