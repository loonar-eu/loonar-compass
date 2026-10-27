"""The flat Loonar logo: the same "star nodes" on a navy background, without glow or gradients.
Two colours (navy + white) and a one-colour variant for print."""
import math, os, sys

out = sys.argv[1]
NAVY = "#0E4A6B"
WHITE = "#FFFFFF"

# (x, y, radius of the disc, angles of the rays in degrees, length of the rays)
NODES = [
    (110, 168, 30, [62, 152, 238, 330], 2.1),
    (164, 88, 24, [40, 128, 215, 305], 2.2),
    (62, 112, 17, [55, 145, 230, 320], 2.2),
    (200, 192, 19, [30, 115, 205, 295], 2.1),
]


def ray(x, y, r, ang, k, color):
    a = math.radians(ang)
    tip = (x + math.cos(a) * r * k * 1.25, y - math.sin(a) * r * k * 1.25)
    w = r * 0.36
    p1 = (x + math.cos(a + math.pi / 2) * w, y - math.sin(a + math.pi / 2) * w)
    p2 = (x + math.cos(a - math.pi / 2) * w, y - math.sin(a - math.pi / 2) * w)
    return f'<path d="M{p1[0]:.1f} {p1[1]:.1f} L{tip[0]:.1f} {tip[1]:.1f} L{p2[0]:.1f} {p2[1]:.1f} Z" fill="{color}"/>'


def mark(fg, bg):
    parts = []
    for x, y, r, angs, k in NODES:
        for ang in angs:
            parts.append(ray(x, y, r, ang, k, fg))
    for x, y, r, angs, k in NODES:
        parts.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fg}"/>')
        parts.append(f'<circle cx="{x}" cy="{y}" r="{r*0.62:.1f}" fill="none" stroke="{bg}" stroke-width="{max(2.2, r*0.13):.1f}"/>')
        parts.append(f'<circle cx="{x}" cy="{y}" r="{r*0.26:.1f}" fill="{bg}"/>')
    return "\n  ".join(parts)


def svg(body, background=None, size=256):
    bgrect = f'<rect width="256" height="256" rx="48" fill="{background}"/>\n  ' if background else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="{size}" height="{size}">\n'
            f'  {bgrect}{body}\n</svg>\n')


os.makedirs(out, exist_ok=True)
files = {
    "loonar-logo.svg": svg(mark(WHITE, NAVY), NAVY),             # main: white on navy
    "loonar-logo-navy.svg": svg(mark(NAVY, WHITE)),              # navy on a light background, no background (print, mug)
    "loonar-logo-black.svg": svg(mark("#000000", WHITE)),        # one colour
    "loonar-logo-white.svg": svg(mark(WHITE, "#000000").replace('stroke="#000000"', 'stroke="none"').replace('fill="#000000"', 'fill="none"')),
}
for name, text in files.items():
    open(os.path.join(out, name), "w", encoding="utf-8").write(text)
print("ok")
