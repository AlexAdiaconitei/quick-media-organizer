"""Renders the app icon: the header's diamond mark on the app's dark surface.

Run with `python scripts/generate-icon.py`, then feed the result to
`npx tauri icon docs/icon-source.png` to regenerate every platform size.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
SS = 4  # supersampling factor, for clean diagonals
BG_TOP = (28, 27, 36)  # violet-tinted top, matching the app's body gradient
BG_BOTTOM = (7, 7, 8)  # --bg
MARK = (245, 243, 239)  # --text
MARK_DIM = (155, 149, 144)  # --muted
ROOT = Path(__file__).resolve().parent.parent


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), radius, fill=255)
    return mask


def vertical_gradient(size: int, top: tuple, bottom: tuple) -> Image.Image:
    gradient = Image.new("RGB", (1, size))
    for y in range(size):
        t = y / (size - 1)
        gradient.putpixel(
            (0, y),
            tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3)),
        )
    return gradient.resize((size, size), Image.BILINEAR)


def diamond(center: float, radius_x: float, radius_y: float) -> list:
    return [
        (center, center - radius_y),
        (center + radius_x, center),
        (center, center + radius_y),
        (center - radius_x, center),
    ]


def build() -> Image.Image:
    size = SIZE * SS
    canvas = vertical_gradient(size, BG_TOP, BG_BOTTOM).convert("RGBA")

    # Soft highlight in the upper-left, so the flat square reads as a surface.
    glow = Image.new("L", (size, size), 0)
    ImageDraw.Draw(glow).ellipse(
        (-size * 0.35, -size * 0.55, size * 0.85, size * 0.45), fill=42
    )
    glow = glow.filter(ImageFilter.GaussianBlur(size * 0.06))
    canvas = Image.composite(Image.new("RGBA", (size, size), (120, 118, 150, 255)), canvas, glow)

    draw = ImageDraw.Draw(canvas)
    center = size / 2
    outer = size * 0.30
    stroke = size * 0.035

    # ◈ is a diamond outline around a filled diamond.
    draw.polygon(diamond(center, outer, outer * 1.18), outline=MARK_DIM, width=round(stroke))
    draw.polygon(diamond(center, outer * 0.46, outer * 0.54), fill=MARK)

    mask = rounded_mask(size, round(size * 0.22))
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.paste(canvas, (0, 0), mask)
    return icon.resize((SIZE, SIZE), Image.LANCZOS)


if __name__ == "__main__":
    target = ROOT / "docs" / "icon-source.png"
    build().save(target)
    print(f"wrote {target}")
