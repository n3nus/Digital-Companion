#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "nokk"
SOURCE = OUT / "generated" / "source.png"
WALK_SOURCE = OUT / "generated" / "walk_source.png"
WALK_DIAGONAL_SOURCE = OUT / "generated" / "walk_diagonal_source.png"
FRAME = 192
COLUMNS = 8
ROWS = 10
WALK_BASE_INDEX = 48

# Crops from the generated source spritesheet. The generated source is kept in
# assets/nokk/generated/source.png so the app asset can be rebuilt reproducibly.
SPRITE_BOXES: list[tuple[int, int, int, int]] = [
    (40, 16, 184, 196),
    (199, 16, 342, 196),
    (364, 16, 509, 196),
    (531, 14, 677, 196),
    (718, 28, 859, 196),
    (874, 35, 1004, 195),
    (1036, 86, 1195, 190),
    (1210, 86, 1362, 190),
    (39, 217, 155, 379),
    (202, 216, 323, 376),
    (379, 216, 496, 376),
    (542, 218, 661, 377),
    (697, 218, 812, 377),
    (847, 218, 965, 379),
    (1004, 218, 1124, 379),
    (1171, 218, 1295, 380),
    (39, 394, 171, 555),
    (214, 395, 357, 556),
    (404, 397, 537, 555),
    (568, 390, 705, 555),
    (733, 394, 866, 554),
    (894, 395, 1030, 554),
    (1070, 394, 1195, 553),
    (1227, 394, 1360, 554),
    (44, 570, 173, 721),
    (218, 570, 349, 721),
    (389, 571, 520, 721),
    (569, 570, 704, 721),
    (744, 572, 876, 720),
    (918, 574, 1048, 721),
    (1100, 572, 1233, 721),
    (1293, 572, 1425, 721),
    (26, 737, 158, 891),
    (204, 740, 334, 891),
    (373, 734, 511, 890),
    (537, 735, 669, 890),
    (706, 740, 839, 891),
    (876, 740, 1004, 891),
    (1044, 744, 1166, 894),
    (1217, 754, 1341, 894),
    (1372, 81, 1528, 188),
]

HEART_BOXES: list[tuple[int, int, int, int]] = [
    (52, 930, 110, 982),
    (164, 934, 211, 976),
    (268, 941, 308, 977),
    (367, 941, 407, 978),
]


def is_key_pixel(r: int, g: int, b: int) -> bool:
    return r > 165 and b > 145 and g < 120 and r > g + 75 and b > g + 55


def remove_chroma(image: Image.Image) -> Image.Image:
    rgba = image.convert("RGBA")
    pixels = rgba.load()
    width, height = rgba.size
    for y in range(height):
        for x in range(width):
            r, g, b, a = pixels[x, y]
            if is_key_pixel(r, g, b):
                pixels[x, y] = (0, 0, 0, 0)
            elif a:
                pixels[x, y] = (r, g, b, 255)
    return rgba


def crop_with_padding(source: Image.Image, box: tuple[int, int, int, int], padding: int = 6) -> Image.Image:
    left, top, right, bottom = box
    padded = (
        max(0, left - padding),
        max(0, top - padding),
        min(source.width, right + padding + 1),
        min(source.height, bottom + padding + 1),
    )
    return remove_chroma(source.crop(padded))


def trim_alpha(sprite: Image.Image, padding: int = 4) -> Image.Image:
    bbox = sprite.getbbox()
    if bbox is None:
        return sprite
    left, top, right, bottom = bbox
    return sprite.crop(
        (
            max(0, left - padding),
            max(0, top - padding),
            min(sprite.width, right + padding),
            min(sprite.height, bottom + padding),
        )
    )


def crop_grid_cell(source: Image.Image, grid_rows: int, row: int, column: int) -> Image.Image:
    cell_width = source.width // 8
    cell_height = source.height // grid_rows
    cell = source.crop(
        (
            column * cell_width,
            row * cell_height,
            (column + 1) * cell_width,
            (row + 1) * cell_height,
        )
    )
    return trim_alpha(remove_chroma(cell), padding=6)


def paste_character(sheet: Image.Image, cell_index: int, sprite: Image.Image) -> None:
    max_size = FRAME - 8
    if sprite.width > max_size or sprite.height > max_size:
        factor = min(max_size / sprite.width, max_size / sprite.height)
        sprite = sprite.resize(
            (max(1, round(sprite.width * factor)), max(1, round(sprite.height * factor))),
            Image.Resampling.NEAREST,
        )

    cell_x = (cell_index % COLUMNS) * FRAME
    cell_y = (cell_index // COLUMNS) * FRAME
    x = cell_x + (FRAME - sprite.width) // 2
    y = cell_y + FRAME - sprite.height - 4
    sheet.alpha_composite(sprite, (x, y))


def paste_particle(sheet: Image.Image, cell_index: int, sprite: Image.Image) -> None:
    if sprite.width > 72 or sprite.height > 72:
        factor = min(72 / sprite.width, 72 / sprite.height)
        sprite = sprite.resize(
            (max(1, round(sprite.width * factor)), max(1, round(sprite.height * factor))),
            Image.Resampling.NEAREST,
        )

    cell_x = (cell_index % COLUMNS) * FRAME
    cell_y = (cell_index // COLUMNS) * FRAME
    sheet.alpha_composite(sprite, (cell_x + 8, cell_y + 8))


def write_preview(sheet: Image.Image) -> None:
    indices = [
        *range(0, 4),
        6,
        7,
        40,
        *range(16, 24),
        *range(24, 32),
        *range(32, 40),
        *range(48, 56),
        *range(56, 64),
        *range(64, 72),
        *range(72, 80),
        41,
        42,
        43,
        44,
    ]
    columns = 8
    scale = 1
    rows = (len(indices) + columns - 1) // columns
    preview = Image.new("RGBA", (columns * FRAME * scale, rows * FRAME * scale), (14, 31, 20, 255))
    for out_index, frame_index in enumerate(indices):
        sx = (frame_index % COLUMNS) * FRAME
        sy = (frame_index // COLUMNS) * FRAME
        frame = sheet.crop((sx, sy, sx + FRAME, sy + FRAME))
        if scale != 1:
            frame = frame.resize((FRAME * scale, FRAME * scale), Image.Resampling.NEAREST)
        dx = (out_index % columns) * FRAME * scale
        dy = (out_index // columns) * FRAME * scale
        preview.alpha_composite(frame, (dx, dy))
    preview.save(OUT / "preview.png")


def write_manifest() -> None:
    (OUT / "manifest.ron").write_text(
        """(
    frame_size: 192,
    sheet_columns: 8,
    animations: {
        "idle": (frames: [(index: 0, duration_ms: 520), (index: 1, duration_ms: 640), (index: 0, duration_ms: 560), (index: 1, duration_ms: 720)], looped: true),
        "blink": (frames: [(index: 0, duration_ms: 110), (index: 2, duration_ms: 130), (index: 3, duration_ms: 130), (index: 0, duration_ms: 140)], looped: false),
        "walk": (frames: [(index: 48, duration_ms: 105), (index: 49, duration_ms: 105), (index: 50, duration_ms: 105), (index: 51, duration_ms: 105), (index: 52, duration_ms: 105), (index: 53, duration_ms: 105), (index: 54, duration_ms: 105), (index: 55, duration_ms: 105)], looped: true),
        "walk_down": (frames: [(index: 48, duration_ms: 105), (index: 49, duration_ms: 105), (index: 50, duration_ms: 105), (index: 51, duration_ms: 105), (index: 52, duration_ms: 105), (index: 53, duration_ms: 105), (index: 54, duration_ms: 105), (index: 55, duration_ms: 105)], looped: true),
        "walk_up": (frames: [(index: 56, duration_ms: 105), (index: 57, duration_ms: 105), (index: 58, duration_ms: 105), (index: 59, duration_ms: 105), (index: 60, duration_ms: 105), (index: 61, duration_ms: 105), (index: 62, duration_ms: 105), (index: 63, duration_ms: 105)], looped: true),
        "walk_left": (frames: [(index: 64, duration_ms: 105), (index: 65, duration_ms: 105), (index: 66, duration_ms: 105), (index: 67, duration_ms: 105), (index: 68, duration_ms: 105), (index: 69, duration_ms: 105), (index: 70, duration_ms: 105), (index: 71, duration_ms: 105)], looped: true),
        "walk_right": (frames: [(index: 72, duration_ms: 105), (index: 73, duration_ms: 105), (index: 74, duration_ms: 105), (index: 75, duration_ms: 105), (index: 76, duration_ms: 105), (index: 77, duration_ms: 105), (index: 78, duration_ms: 105), (index: 79, duration_ms: 105)], looped: true),
        "sit": (frames: [(index: 4, duration_ms: 900), (index: 5, duration_ms: 1100)], looped: true),
        "sleep": (frames: [(index: 6, duration_ms: 1200), (index: 7, duration_ms: 1500), (index: 40, duration_ms: 1800), (index: 7, duration_ms: 1500)], looped: true),
        "happy": (frames: [(index: 24, duration_ms: 130), (index: 25, duration_ms: 130), (index: 26, duration_ms: 130), (index: 27, duration_ms: 130), (index: 28, duration_ms: 130), (index: 29, duration_ms: 130)], looped: true),
        "poke": (frames: [(index: 32, duration_ms: 120), (index: 33, duration_ms: 130), (index: 34, duration_ms: 150), (index: 35, duration_ms: 170), (index: 36, duration_ms: 210), (index: 37, duration_ms: 260), (index: 38, duration_ms: 340), (index: 39, duration_ms: 760)], looped: false),
        "dance": (frames: [(index: 16, duration_ms: 120), (index: 17, duration_ms: 120), (index: 18, duration_ms: 120), (index: 19, duration_ms: 120), (index: 20, duration_ms: 120), (index: 21, duration_ms: 120), (index: 22, duration_ms: 120), (index: 23, duration_ms: 120)], looped: true),
    },
    hit_zones: [
        (name: "head", rect: (x: 42, y: 18, w: 108, h: 82)),
        (name: "back", rect: (x: 48, y: 88, w: 98, h: 38)),
        (name: "body", rect: (x: 38, y: 72, w: 116, h: 112)),
    ],
    heart_spawn_points: [(x: 74, y: 22), (x: 96, y: 12), (x: 122, y: 24)],
    heart_frames: [41, 42, 43, 44],
)
""",
        encoding="utf-8",
    )


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"missing generated source image: {SOURCE}")
    if not WALK_SOURCE.exists():
        raise SystemExit(f"missing generated walk source image: {WALK_SOURCE}")
    if not WALK_DIAGONAL_SOURCE.exists():
        raise SystemExit(f"missing generated diagonal walk source image: {WALK_DIAGONAL_SOURCE}")

    OUT.mkdir(parents=True, exist_ok=True)
    source = Image.open(SOURCE).convert("RGB")
    walk_source = Image.open(WALK_SOURCE).convert("RGB")
    walk_diagonal_source = Image.open(WALK_DIAGONAL_SOURCE).convert("RGB")
    sheet = Image.new("RGBA", (FRAME * COLUMNS, FRAME * ROWS), (0, 0, 0, 0))

    for index, box in enumerate(SPRITE_BOXES):
        paste_character(sheet, index, crop_with_padding(source, box))

    for offset, box in enumerate(HEART_BOXES):
        paste_particle(sheet, 41 + offset, crop_with_padding(source, box, padding=4))

    for column in range(8):
        paste_character(
            sheet,
            WALK_BASE_INDEX + column,
            crop_grid_cell(walk_source, 4, 0, column),
        )
        paste_character(
            sheet,
            WALK_BASE_INDEX + 8 + column,
            crop_grid_cell(walk_source, 4, 1, column),
        )
        paste_character(
            sheet,
            WALK_BASE_INDEX + 16 + column,
            crop_grid_cell(walk_diagonal_source, 2, 0, column),
        )
        paste_character(
            sheet,
            WALK_BASE_INDEX + 24 + column,
            crop_grid_cell(walk_diagonal_source, 2, 1, column),
        )

    sheet.save(OUT / "spritesheet.png")
    write_preview(sheet)
    write_manifest()


if __name__ == "__main__":
    main()
