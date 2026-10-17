"""New splash/banner background (own artwork): navy star map with a globe grid and a faint compass rose.
Writes splash_bg.svg; render it to PNG 1078x604 with headless Edge."""
import math, random, sys

out = sys.argv[1]
W, H = 1078, 604
random.seed(7)
parts = []
parts.append("""<defs>
  <radialGradient id="sky" cx="30%" cy="38%" r="85%">
    <stop offset="0" stop-color="#1A5F86"/>
    <stop offset="0.55" stop-color="#0E4A6B"/>
    <stop offset="1" stop-color="#072B41"/>
  </radialGradient>
  <radialGradient id="glow" cx="50%" cy="50%" r="50%">
    <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.10"/>
    <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
  </radialGradient>
</defs>""")
parts.append(f'<rect width="{W}" height="{H}" fill="url(#sky)"/>')

# Globe grid on the left: outline, parallels and meridians of an orthographic globe.
cx, cy, r = 250, 330, 250
g = ['<g fill="none" stroke="#BFD8EC" stroke-opacity="0.16" stroke-width="1.4">']
g.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" stroke-opacity="0.26"/>')
tilt = math.radians(18)
for lat in range(-60, 90, 30):
    y = cy - r * math.sin(math.radians(lat)) * math.cos(tilt)
    rx = r * math.cos(math.radians(lat))
    ry = rx * math.sin(tilt)
    g.append(f'<ellipse cx="{cx}" cy="{y:.1f}" rx="{rx:.1f}" ry="{ry:.1f}"/>')
for lon in range(0, 180, 30):
    rx = abs(r * math.sin(math.radians(lon)))
    g.append(f'<ellipse cx="{cx}" cy="{cy}" rx="{rx:.1f}" ry="{r}" transform="rotate(-8 {cx} {cy})"/>')
g.append("</g>")
parts.extend(g)
parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#glow)"/>')

# Faint compass rose over the globe.
rose = ['<g fill="#FFFFFF" fill-opacity="0.07">']
for k in range(8):
    a = math.radians(k * 45)
    length = 300 if k % 2 == 0 else 170
    w = 26 if k % 2 == 0 else 18
    tip = (cx + math.sin(a) * length, cy - math.cos(a) * length)
    l = (cx + math.sin(a - math.pi / 2) * w, cy - math.cos(a - math.pi / 2) * w)
    rr = (cx + math.sin(a + math.pi / 2) * w, cy - math.cos(a + math.pi / 2) * w)
    rose.append(f'<path d="M{l[0]:.1f} {l[1]:.1f} L{tip[0]:.1f} {tip[1]:.1f} L{rr[0]:.1f} {rr[1]:.1f} Z"/>')
rose.append("</g>")
parts.extend(rose)

# Stars: small dots, a few brighter ones, a constellation on the right half.
stars = []
for _ in range(170):
    x, y = random.uniform(0, W), random.uniform(0, H)
    size = random.choice([0.8, 0.9, 1.0, 1.2, 1.5, 2.0])
    alpha = random.uniform(0.18, 0.6)
    stars.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{size}" fill="#FFFFFF" fill-opacity="{alpha:.2f}"/>')
parts.extend(stars)
constellation = [(600, 120), (690, 175), (770, 140), (850, 210), (930, 170), (980, 250), (900, 300), (820, 330)]
line = " ".join(f"{x},{y}" for x, y in constellation)
parts.append(f'<polyline points="{line}" fill="none" stroke="#FFFFFF" stroke-opacity="0.13" stroke-width="1.2"/>')
for x, y in constellation:
    parts.append(f'<circle cx="{x}" cy="{y}" r="2.4" fill="#FFFFFF" fill-opacity="0.45"/>')

svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">\n' + "\n".join(parts) + "\n</svg>\n"
open(out, "w", encoding="utf-8").write(svg)
print("ok")
