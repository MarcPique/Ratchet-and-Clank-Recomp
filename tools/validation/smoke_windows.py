"""Windows release smoke test: every level plus representative graphics settings.

Runs the actual packaged game at 3840x2160, keeps saves and the user's settings
untouched, checks that each process exits and that a nonblank PNG was rendered.
Python 3 is required; Pillow adds a pixel-level blank-screen check when present.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path


GRAPHICS = {
    "original": {"anti_aliasing": "original", "post_aa": "off", "hud": "original"},
    "enhanced": {"anti_aliasing": "off", "post_aa": "smaa", "textures": "smooth", "detail": "far", "anisotropy": "16x"},
    "msaa_2x": {"anti_aliasing": "2x"},
    "msaa_4x": {"anti_aliasing": "4x"},
    "msaa_8x": {"anti_aliasing": "8x"},
    "fxaa": {"anti_aliasing": "off", "post_aa": "fxaa"},
    "sharp": {"textures": "sharp", "sharpness": "high", "hud": "sharp"},
    "farther": {"detail": "farther", "shadows": "off"},
    "scaled_75": {"render_scale": "75"},
    "scaled_125": {"render_scale": "125"},
}


def nonblank(path: Path) -> bool:
    if not path.is_file() or path.stat().st_size < 100_000:
        return False
    try:
        from PIL import Image, ImageStat

        with Image.open(path) as im:
            im = im.convert("RGB").resize((160, 90))
            return max(ImageStat.Stat(im).stddev) > 3.0
    except ImportError:
        return True


def run_case(exe: Path, data: Path, root: Path, name: str, level: int, overrides: dict[str, str], delay: float, timeout: float, extra_env: dict[str, str] | None = None) -> dict:
    case = root / name
    case.mkdir(parents=True, exist_ok=True)
    settings = case / "settings.toml"
    values = {
        "language": "4", "aspect": "16:9", "resolution": "2160p",
        "window_mode": "borderless", "render_scale": "100", "fps_limit": "60",
        "vsync": "1", **overrides,
    }
    settings.write_text("".join(f"{key} = {value}\n" for key, value in values.items()), encoding="utf-8")
    screenshot = case / "screen.png"
    env = os.environ.copy()
    env.update({
        "RC_SETTINGS_FILE": str(settings), "RC_SAVE_DIR": "0", "RC_LEVEL": str(level),
        "RC_FRONTEND": "0", "RC_SCENE": "0", "RC_AUDIO": "0",
        "RC_SCREENSHOT": str(screenshot), "RC_SCREENSHOT_DELAY": str(delay),
    })
    for key in ("RC_PLAY_SCRIPT", "RC_SCREENSHOT_FRAME", "RC_DETERMINISTIC"):
        env.pop(key, None)
    env.update(extra_env or {})
    try:
        result = subprocess.run(
            [str(exe), "--data-dir", str(data)], cwd=exe.parent, env=env,
            capture_output=True, text=True, errors="replace", timeout=timeout,
        )
        (case / "stdout.log").write_text(result.stdout, encoding="utf-8")
        (case / "stderr.log").write_text(result.stderr, encoding="utf-8")
        good = result.returncode == 0 and nonblank(screenshot)
        reason = "ok" if good else f"exit {result.returncode}, screenshot {'nonblank' if nonblank(screenshot) else 'blank/missing'}"
    except subprocess.TimeoutExpired as error:
        (case / "timeout.txt").write_text(str(error), encoding="utf-8")
        good, reason = False, "timeout"
    print(f"{'PASS' if good else 'FAIL'} {name}: {reason}", flush=True)
    return {"name": name, "level": level, "pass": good, "reason": reason}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("work/smoke-windows"))
    parser.add_argument("--quick", action="store_true", help="Four levels and three graphics profiles")
    parser.add_argument("--live-switch-only", action="store_true", help="Exercise a live MSAA change")
    parser.add_argument("--accessibility-only", action="store_true", help="Boot a subtitled scene with accessibility settings")
    parser.add_argument("--delay", type=float, default=4.0)
    parser.add_argument("--timeout", type=float, default=45.0)
    args = parser.parse_args()
    exe, data, root = args.exe.resolve(), args.data_dir.resolve(), args.output.resolve()
    if not exe.is_file() or not data.is_dir():
        parser.error("--exe must be a file and --data-dir an extracted game folder")
    root.mkdir(parents=True, exist_ok=True)
    if args.live_switch_only:
        result = run_case(exe, data, root, "live_msaa_switch", 1, {"anti_aliasing": "off"}, max(args.delay, 6.0), args.timeout, {"RC_MSAA_SWITCH": "120:4"})
        (root / "report.json").write_text(json.dumps([result], indent=2) + "\n", encoding="utf-8")
        return 0 if result["pass"] else 1
    if args.accessibility_only:
        result = run_case(exe, data, root, "accessible_scene", 1,
                          {"subtitle_size": "extra", "subtitle_contrast": "high", "camera_shake": "off"},
                          max(args.delay, 6.0), args.timeout, {"RC_SCENE": "5"})
        (root / "report.json").write_text(json.dumps([result], indent=2) + "\n", encoding="utf-8")
        return 0 if result["pass"] else 1
    levels = (0, 1, 9, 18) if args.quick else range(19)
    profiles = ("original", "enhanced", "scaled_125") if args.quick else GRAPHICS
    results = [run_case(exe, data, root, f"level_{i:02}", i, {}, args.delay, args.timeout) for i in levels]
    results += [run_case(exe, data, root, f"graphics_{name}", 1, GRAPHICS[name], args.delay, args.timeout) for name in profiles]
    if not args.quick:
        results.append(run_case(exe, data, root, "live_msaa_switch", 1, {"anti_aliasing": "off"}, max(args.delay, 6.0), args.timeout, {"RC_MSAA_SWITCH": "120:4"}))
    (root / "report.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print(f"{sum(r['pass'] for r in results)}/{len(results)} passed; report: {root / 'report.json'}")
    return 0 if all(r["pass"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
