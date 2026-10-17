# -*- coding: utf-8 -*-
"""Images of loonar.eu (requires Pillow).

    python -I -B website_src\\assets.py          the share image assets/img/og-image.png (1200 x 630)
    python -I -B website_src\\assets.py icons    also the logos and favicons from assets/brand (after a logo change)

The licence copies are made by build.py.
"""
import os, shutil, sys
from PIL import Image, ImageDraw, ImageFont

P = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
W = P + r"\website"
B = P + r"\assets\brand"
NAME = ("Loonar", "Compass")
TAGLINE = "AI assistant for Sparx Enterprise Architect"
FOOT = "Freeware for Windows  ·  loonar.eu"


def rounded_logo():
    src = Image.open(B + r"\loonar-logo-1024.png").convert("RGBA")
    m = Image.new("L", (4096, 4096), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, 4095, 4095], radius=768, fill=255)
    src.putalpha(m.resize((1024, 1024), Image.LANCZOS))
    return src


def icons(src):
    shutil.copy(B + r"\loonar-logo.svg", W + r"\assets\img\logo.svg")
    shutil.copy(B + r"\loonar-logo-navy.svg", W + r"\assets\img\logo-navy.svg")
    shutil.copy(B + r"\loonar-logo-white.svg", W + r"\assets\img\logo-white.svg")
    shutil.copy(B + r"\loonar-logo.svg", W + r"\favicon.svg")
    src.resize((32, 32), Image.LANCZOS).save(W + r"\favicon-32.png", optimize=True)
    # apple touch icon: opaque, navy behind the rounded corners
    at = Image.new("RGBA", (1024, 1024), (14, 74, 107, 255)); at.alpha_composite(src)
    at.convert("RGB").resize((180, 180), Image.LANCZOS).save(W + r"\apple-touch-icon.png", optimize=True)
    src.save(W + r"\favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)])


def og_image(src):
    og = Image.new("RGB", (1200, 630), (14, 74, 107))
    d = ImageDraw.Draw(og)
    for i in range(630):  # subtle gradient
        d.line([(0, i), (1200, i)], fill=(int(14 + 8 * i / 630), int(74 - 20 * i / 630), int(107 - 18 * i / 630)))
    lg = src.resize((300, 300), Image.LANCZOS)
    og.paste(lg, (90, 165), lg)
    fb = ImageFont.truetype(r"C:\Windows\Fonts\segoeuib.ttf", 76)
    fr = ImageFont.truetype(r"C:\Windows\Fonts\segoeui.ttf", 36)
    fs = ImageFont.truetype(r"C:\Windows\Fonts\segoeuisl.ttf", 28)
    d.text((450, 200), NAME[0], font=fb, fill="white")
    d.text((450, 285), NAME[1], font=fb, fill="white")
    d.text((452, 395), TAGLINE, font=fr, fill=(214, 232, 242))
    d.text((452, 445), FOOT, font=fs, fill=(170, 203, 222))
    og.save(W + r"\assets\img\og-image.png", optimize=True)


if __name__ == "__main__":
    logo = rounded_logo()
    if "icons" in sys.argv[1:]:
        icons(logo)
    og_image(logo)
    print("done")
