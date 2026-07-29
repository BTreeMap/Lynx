"""Render the Lynx mark to the raster icons in `frontend/public/`.

Run by hand when the mark changes; not part of `npm run build`, since it needs Python
and Pillow, which CI's Node-only frontend job does not have:

    python3 scripts/render-icons.py public

The source of truth for the mark is `public/favicon.svg`, which carries the same three
paths as `src/components/layout/Logo.tsx`. Those paths are reproduced here rather than
rasterised, because no SVG rasteriser is guaranteed present. Each arc in the source is
an SVG elliptical-arc command; both were converted to centre form once and are sampled
as polylines, which lets one wide round-joined stroke draw a whole subpath (Pillow's
arc primitive has no round caps).

Geometry lives in the source 24x24 space and is mapped into the tile exactly as the
SVG's transform does: translate(14.4 14.4) scale(1.4666667) on a 64-unit tile.
"""

import math
from PIL import Image, ImageDraw

BRAND = (65, 103, 136, 255)  # baltic-blue-500 #416788
WHITE = (255, 255, 255, 255)
SS = 4  # supersampling factor; downscaled with LANCZOS for antialiasing


def arc_points(cx, cy, r, a0, a1, steps=64):
    """Sample an arc from a0 to a1 (degrees, y-down, increasing = SVG sweep=1)."""
    return [
        (cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / steps)),
         cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / steps)))
        for i in range(steps + 1)
    ]


# Path 1: M9.5 13.5 L5.8 17.2 a3.3(→3.3234, radius scaled to the chord) … l3.2 -3.2 a3.3 …
PATH_1 = (
    [(9.5, 13.5), (5.8, 17.2)]
    + arc_points(3.45, 14.85, 3.3234, 45.0, 225.0)          # semicircle: 2r == chord
    + [(1.1, 12.5), (4.3, 9.3)]
    + arc_points(6.65, 11.6168, 3.3, 224.57, 315.43)        # centre from chord + sagitta
    + [(9.0, 9.3)]
)
# Path 2 is Path 1 rotated 180° about the glyph centre (12, 12).
PATH_2 = [(24.0 - x, 24.0 - y) for (x, y) in PATH_1]
PATH_3 = [(9.0, 15.0), (15.0, 9.0)]
PATHS = (PATH_1, PATH_2, PATH_3)

STROKE_24 = 2.0     # stroke-width in the 24-unit space
OFFSET_64 = 14.4    # translate() in the 64-unit tile
SCALE_64 = 1.4666667
RADIUS_64 = 14.0    # rounded-corner radius in the 64-unit tile


def render(size: int, optical: float = 1.0) -> Image.Image:
    """Render one square icon.

    `optical` enlarges the glyph and its stroke for very small renditions. At 16px the
    faithful geometry puts a sub-pixel stroke on an 8px glyph, which antialiases to grey
    mush; the mark has to be drawn slightly heavier to read at all. This is optical
    sizing, and it is why the 16px frame is not pixel-faithful to the SVG.
    """
    canvas = size * SS
    unit = canvas / 64.0                      # tile units → pixels
    scale = unit * SCALE_64 * optical         # glyph units → pixels
    offset = (canvas - 24 * scale) / 2        # centred, whatever the glyph size

    image = Image.new('RGBA', (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        (0, 0, canvas - 1, canvas - 1), radius=RADIUS_64 * unit, fill=BRAND
    )

    width = max(1, round(STROKE_24 * scale * (1.3 if optical > 1.0 else 1.0)))
    for path in PATHS:
        points = [(offset + x * scale, offset + y * scale) for (x, y) in path]
        # PIL strokes with butt ends and no round joins, so the stroke is composed by
        # hand: one segment per sampled pair, plus a disc at every vertex. That is the
        # definition of a round-capped, round-joined stroke, and it leaves no notches.
        radius = width / 2
        for start, end in zip(points, points[1:]):
            draw.line((start, end), fill=WHITE, width=width)
        for px, py in points:
            draw.ellipse((px - radius, py - radius, px + radius, py + radius), fill=WHITE)

    return image.resize((size, size), Image.LANCZOS)


if __name__ == '__main__':
    import sys

    out = sys.argv[1]
    for name, size in (('apple-touch-icon.png', 180), ('icon-192.png', 192), ('icon-512.png', 512)):
        render(size).save(f'{out}/{name}', optimize=True)
        print(f'{out}/{name}')

    # Multi-size ICO for Safari <= 18.7 and anything else that will not take an SVG.
    # Each frame is rendered from the vector geometry rather than downsampled from one
    # bitmap: at 16px the stroke is a single pixel, and resampling a larger raster turns
    # it to grey mush.
    # 16px alone gets the optical boost; 32 and 48 read correctly as drawn.
    ico_sizes = (16, 32, 48)
    frames = [render(16, optical=1.75 / SCALE_64), render(32), render(48)]
    frames[-1].save(
        f'{out}/favicon.ico',
        format='ICO',
        sizes=[(s, s) for s in ico_sizes],
        append_images=frames[:-1],
    )
    print(f'{out}/favicon.ico')
