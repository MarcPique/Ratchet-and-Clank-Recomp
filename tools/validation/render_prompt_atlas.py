"""Render the game's nine 64 px controller-prompt rows as a review sheet."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROWS = (
    "PlayStation",
    "Xbox",
    "Switch",
    "DualSense",
    "DualShock 4",
    "DualShock 3",
    "Xbox Series",
    "Xbox One",
    "Xbox 360",
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).parents[2]
    raw = (root / "crates/rc-engine/assets/controller_prompts.rgba").read_bytes()
    atlas = Image.frombytes("RGBA", (1024, 576), raw)
    # The renderer stores GS alpha in 0..128; PNG needs conventional 0..255.
    alpha = atlas.getchannel("A").point(lambda value: min(255, value * 2))
    atlas.putalpha(alpha)
    width, height = 1220, 860
    sheet = Image.new("RGB", (width, height), (18, 24, 34))
    draw = ImageDraw.Draw(sheet)
    regular = Path("C:/Windows/Fonts/segoeui.ttf")
    bold = Path("C:/Windows/Fonts/segoeuib.ttf")
    font = ImageFont.truetype(str(regular), 23) if regular.is_file() else ImageFont.load_default()
    title = ImageFont.truetype(str(bold), 34) if bold.is_file() else font
    draw.text((24, 17), "Iconos HD de los mandos", font=title, fill=(237, 242, 248))
    for row, label in enumerate(ROWS):
        top = 70 + row * 85
        draw.rounded_rectangle((12, top, width - 12, top + 78), radius=9, fill=(31, 41, 55))
        draw.text((25, top + 25), label, font=font, fill=(237, 242, 248))
        icons = atlas.crop((0, row * 64, 1024, (row + 1) * 64))
        sheet.paste(icons, (182, top + 7), icons)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(args.output)
    print(args.output)


if __name__ == "__main__":
    main()
