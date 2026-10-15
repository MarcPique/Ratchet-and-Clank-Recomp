"""Build the game's controller-prompt atlas from Kenney's CC0 Input Prompts 1.5.

Source: https://kenney.nl/assets/input-prompts
Input: kenney_input-prompts_1.5.zip (pass its path as argv[1])
Output: crates/rc-engine/assets/controller_prompts.rgba, 1024x576 RGBA.
Rows match text_render::PromptStyle; columns match input_map::physical_index.
"""

from io import BytesIO
from pathlib import Path
import sys
from zipfile import ZipFile

from PIL import Image

PS = "PlayStation Series/Default/"
XB = "Xbox Series/Default/"
SW = "Nintendo Switch/Default/"

PS_FACE = ["cross", "circle", "square", "triangle"]
XB_FACE = ["a", "b", "x", "y"]
SW_FACE = ["b", "a", "y", "x"]


def ps(model):
    face = [f"playstation_button_{'color_' if model != 5 else ''}{b}.png" for b in PS_FACE]
    aux = {
        3: ["playstation3_button_start.png", "playstation3_button_select.png"],
        4: ["playstation4_button_options.png", "playstation4_button_share.png"],
        5: ["playstation5_button_options.png", "playstation5_button_create.png"],
    }[model]
    return [PS + n for n in face + [
        "playstation_trigger_l1.png", "playstation_trigger_r1.png",
        "playstation_trigger_l2.png", "playstation_trigger_r2.png",
        *aux, "playstation_button_l3.png", "playstation_button_r3.png",
        "playstation_dpad_up.png", "playstation_dpad_down.png",
        "playstation_dpad_left.png", "playstation_dpad_right.png",
    ]]


def xbox(model):
    aux = ["xbox_button_start.png", "xbox_button_back.png"] if model == 360 else ["xbox_button_menu.png", "xbox_button_view.png"]
    return [XB + n for n in [*[f"xbox_button_color_{b}.png" for b in XB_FACE],
        "xbox_lb.png", "xbox_rb.png", "xbox_lt.png", "xbox_rt.png",
        *aux, "xbox_ls.png", "xbox_rs.png", "xbox_dpad_up.png", "xbox_dpad_down.png",
        "xbox_dpad_left.png", "xbox_dpad_right.png",
    ]]


def switch():
    return [SW + n for n in [*[f"switch_button_{b}.png" for b in SW_FACE],
        "switch_button_l.png", "switch_button_r.png", "switch_button_zl.png", "switch_button_zr.png",
        "switch_button_plus.png", "switch_button_minus.png",
        "switch_stick_l_press.png", "switch_stick_r_press.png",
        "switch_dpad_up.png", "switch_dpad_down.png", "switch_dpad_left.png", "switch_dpad_right.png",
    ]]


ROWS = [ps(4), xbox("series"), switch(), ps(5), ps(4), ps(3), xbox("series"), xbox("one"), xbox(360)]


def main():
    source = Path(sys.argv[1])
    output = Path(__file__).parents[2] / "crates/rc-engine/assets/controller_prompts.rgba"
    atlas = Image.new("RGBA", (16 * 64, len(ROWS) * 64), (0, 0, 0, 0))
    with ZipFile(source) as archive:
        for row, files in enumerate(ROWS):
            assert len(files) == 16
            for col, name in enumerate(files):
                icon = Image.open(BytesIO(archive.read(name))).convert("RGBA")
                assert icon.size == (64, 64), (name, icon.size)
                atlas.alpha_composite(icon, (col * 64, row * 64))
    # The GS atlas expects alpha 0x80 for fully opaque texels.
    rgb = atlas.tobytes()
    out = bytearray(rgb)
    for i in range(3, len(out), 4):
        out[i] = (out[i] + 1) // 2
    output.write_bytes(out)
    print(f"{output}: {atlas.width}x{atlas.height}, {len(out)} bytes")


if __name__ == "__main__":
    main()
