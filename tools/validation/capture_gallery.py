"""Capture real 4K menu and controller screenshots with the game window hidden.

Example:
    python tools/validation/capture_gallery.py --exe "F:/Ratchet & Clank PC/Ratchet & Clank.exe" --output work/captures

The executable needs its adjacent Data folder from a locally extracted disc.
Only PNGs are intended for publication; logs and temporary settings stay in output.
"""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

BASE_MENU = (
    ["60:press START"]
    + [f"{tick}:press DOWN" for tick in (100, 118, 136, 154, 172)]
    + ["210:press X"]
    + [f"{tick}:press DOWN" for tick in (250, 268, 286, 304, 322, 340)]
    + ["375:press X"]
)

# label, kind, prompt family, number of DOWN presses after opening Advanced
# Options. The custom resolution height row is hidden at 2160p.
CASES = (
    ("graficos-perfil-4k", "menu", "dualsense", 0),
    ("graficos-antialiasing-4k", "menu", "dualsense", 1),
    ("graficos-texturas-4k", "menu", "dualsense", 6),
    ("graficos-resolucion-4k", "menu", "dualsense", 10),
    ("graficos-pantalla-4k", "menu", "dualsense", 14),
    ("graficos-rendimiento-4k", "menu", "dualsense", 18),
    ("mandos-dualsense-4k", "menu", "dualsense", 20),
    ("mandos-xbox-series-4k", "menu", "xboxseries", 20),
    ("mandos-switch-4k", "menu", "nintendo", 20),
    ("accesibilidad-4k", "menu", "dualsense", 24),
    ("iconos-dualsense-4k", "prompts", "dualsense", 0),
    ("iconos-dualshock4-4k", "prompts", "dualshock4", 0),
    ("iconos-dualshock3-4k", "prompts", "dualshock3", 0),
    ("iconos-xbox-series-4k", "prompts", "xboxseries", 0),
    ("iconos-xbox-one-4k", "prompts", "xboxone", 0),
    ("iconos-xbox-360-4k", "prompts", "xbox360", 0),
    ("iconos-switch-4k", "prompts", "nintendo", 0),
    ("iconos-playstation-generico-4k", "prompts", "playstation", 0),
    ("iconos-xbox-generico-4k", "prompts", "xbox", 0),
)


def capture(exe: Path, output: Path, case: tuple[str, str, str, int]) -> None:
    name, kind, family, row = case
    folder = output / name
    folder.mkdir(parents=True, exist_ok=True)
    settings = folder / "settings.toml"
    settings.write_text(
        "language = 4\naspect = 16:9\nresolution = 2160p\n"
        "window_mode = windowed\nrender_scale = 100\nfps_limit = 60\n"
        "vsync = 1\nhud = high\n"
        f"button_prompts = {family}\n",
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update({
        "RC_TEST_HIDDEN": "1",
        "RC_TEST_WINDOW": "3840x2160",
        "RC_SETTINGS_FILE": str(settings),
        "RC_SAVE_DIR": "0",
        "RC_LEVEL": "1",
        "RC_FRONTEND": "0",
        "RC_SCENE": "0",
        "RC_AUDIO": "0",
        "RC_MENU_TRACE": "1",
        "RC_SCREENSHOT": str(folder / "screen.png"),
    })
    for key in ("RC_HUD_TEXT", "RC_HUD_TEXT_WINDOW", "RC_PLAY_SCRIPT", "RC_SCREENSHOT_DELAY"):
        env.pop(key, None)
    if kind == "menu":
        steps = BASE_MENU + [f"{420 + 18 * i}:press DOWN" for i in range(row)]
        env["RC_PLAY_SCRIPT"] = ",".join(steps)
        env["RC_SCREENSHOT_FRAME"] = str(max(430, 420 + 18 * (row - 1) + 28))
    else:
        # Original in-game prompt control codes, rendered through the game's HUD.
        env["RC_HUD_TEXT"] = "  ".join(chr(i) for i in range(0x10, 0x1A))
        env["RC_HUD_TEXT_WINDOW"] = "1"
        env["RC_SCREENSHOT_FRAME"] = "150"
    result = subprocess.run(
        [str(exe)],
        cwd=exe.parent,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        timeout=120,
    )
    (folder / "stdout.log").write_text(result.stdout, encoding="utf-8")
    (folder / "stderr.log").write_text(result.stderr, encoding="utf-8")
    shot = folder / "screen.png"
    if result.returncode or not shot.is_file() or shot.stat().st_size < 100_000:
        raise RuntimeError(f"{name}: exit {result.returncode}, PNG {shot.exists()}; see {folder}")
    print(f"PASS {name}: {shot.stat().st_size:,} bytes", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--only", nargs="*", help="Capture only these case names")
    args = parser.parse_args()
    exe = args.exe.resolve()
    if not exe.is_file() or not (exe.parent / "Data").is_dir():
        parser.error("--exe must be an installed game with an adjacent Data folder")
    known = {case[0] for case in CASES}
    if args.only and (unknown := set(args.only) - known):
        parser.error(f"Unknown cases: {', '.join(sorted(unknown))}")
    for case in CASES:
        if not args.only or case[0] in args.only:
            capture(exe, args.output.resolve(), case)


if __name__ == "__main__":
    main()
