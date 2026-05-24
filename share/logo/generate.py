"""8-bit pixel-art logo for wintermute.

A blocky "W" with a snowflake crown and a single magenta consciousness
pixel — Wintermute (Neuromancer) meets winter + mute.

Base grid is 32x32, upscaled with nearest-neighbor to keep the chunky
8-bit feel. Palette is NES-ish: deep navy ground, ice cyan strokes,
white highlights, hot magenta accent.
"""

from PIL import Image

SIZE = 32
SCALE = 16  # final image is 512x512

# Palette (RGBA)
TRANSPARENT = (0, 0, 0, 0)
NAVY        = (8, 14, 36, 255)        # background fill
DARK_CYAN   = (26, 76, 110, 255)      # shadow
CYAN        = (95, 212, 255, 255)     # main stroke
ICE         = (190, 240, 255, 255)    # highlight
WHITE       = (255, 255, 255, 255)
MAGENTA     = (255, 58, 160, 255)     # AI / consciousness pixel
DIM_MAGENTA = (140, 28, 90, 255)

# Sprite, row by row. 32 chars wide × 32 rows.
#   . transparent
#   b navy background block
#   d dark cyan (shadow)
#   c cyan (main W stroke)
#   i ice highlight
#   w white highlight
#   m magenta accent
#   x dim magenta accent
SPRITE = [
    "................................",
    "................................",
    "..............w.................",
    "..............w.................",
    "...........w..w..w..............",
    "............w.w.w...............",
    ".w...........www................",
    ".w..........wwwww.............w.",
    ".w.........w..w..w...........ww.",
    ".cc.......w...w...w..........cc.",
    ".cc...........w.............ccc.",
    ".ccc..........w.............cc..",
    "..cc..........w............ccc..",
    "..ccc.........w...........ccc...",
    "...cc.........w...........cc....",
    "...ccc.......www.........ccc....",
    "....cc......ccccc........cc.....",
    "....ccc....ccc.ccc......ccc.....",
    ".....cc...ccc...ccc.....cc......",
    ".....ccc.ccc.....ccc...ccc......",
    "......ccccc.......ccc.ccc.......",
    "......ccc..........ccccc........",
    ".......cc...........ccc.........",
    ".......cc............c..........",
    "........c........... ...........",
    "................................",
    "..............xmx...............",
    "..............mmm...............",
    "..............xmx...............",
    "................................",
    "................................",
    "................................",
]

CHAR_TO_RGBA = {
    ".": TRANSPARENT,
    "b": NAVY,
    "d": DARK_CYAN,
    "c": CYAN,
    "i": ICE,
    "w": WHITE,
    "m": MAGENTA,
    "x": DIM_MAGENTA,
    " ": TRANSPARENT,
}


def render() -> Image.Image:
    assert len(SPRITE) == SIZE, f"sprite has {len(SPRITE)} rows"
    img = Image.new("RGBA", (SIZE, SIZE), TRANSPARENT)
    px = img.load()
    for y, row in enumerate(SPRITE):
        assert len(row) == SIZE, f"row {y} has {len(row)} cols"
        for x, ch in enumerate(row):
            px[x, y] = CHAR_TO_RGBA[ch]
    # Add a 1px dark navy outline around opaque pixels for contrast on
    # any background. Cheap dilation: any transparent pixel touching an
    # opaque neighbor becomes navy.
    out = img.copy()
    op = out.load()
    for y in range(SIZE):
        for x in range(SIZE):
            if px[x, y][3] != 0:
                continue
            touches = False
            for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < SIZE and 0 <= ny < SIZE and px[nx, ny][3] != 0:
                    touches = True
                    break
            if touches:
                op[x, y] = NAVY
    return out


def main() -> None:
    base = render()
    large = base.resize((SIZE * SCALE, SIZE * SCALE), Image.NEAREST)
    base.save("/home/jsy/wintermute/share/logo/wintermute-logo-32.png")
    large.save("/home/jsy/wintermute/share/logo/wintermute-logo.png")
    # Also a 128x128 mid-size for favicons / tray icons.
    base.resize((128, 128), Image.NEAREST).save(
        "/home/jsy/wintermute/share/logo/wintermute-logo-128.png"
    )
    print("wrote wintermute-logo-32.png, wintermute-logo-128.png, wintermute-logo.png")


if __name__ == "__main__":
    main()
