"""Development aid: opens a portable Compass, performs a few actions and saves screenshots and the UI Automation
tree, to find the names of controls for new window tests. python probe.py <folder> <scenario>"""
import sys
import time
from pathlib import Path

from pywinauto import keyboard, mouse

from ui import Compass


def tree(c, name):
    lines = []
    for e in c.win.descendants():
        i = e.element_info
        r = i.rectangle
        lines.append(f"{i.control_type} | {i.name!r} | {r.left},{r.top},{r.right},{r.bottom}")
    (c.out / f"{name}_drzewo.txt").write_text("\n".join(lines), encoding="utf-8")


def context_menu(c):
    c.start_conversation()
    c.say("Ile pakietów ma model? Odpowiedz jednym zdaniem.")
    left = c.win.rectangle().left + 450
    item = next(t for t in c.win.descendants(control_type="Text") if "Ile pakietów" in (t.element_info.name or "") and t.element_info.rectangle.left < left)
    r = item.element_info.rectangle
    x, y = (r.left + r.right) // 2, (r.top + r.bottom) // 2
    mouse.move(coords=(x, y))
    time.sleep(0.5)
    item.right_click_input()
    time.sleep(1.5)
    c.evidence("menu")
    tree(c, "menu")


def paste_file(c):
    import subprocess
    c.start_conversation()
    f = c.work / "plik.txt"
    f.write_text("Testowy plik", encoding="utf-8")
    subprocess.run(["powershell", "-NoProfile", "-Command", f"Set-Clipboard -Path '{f}'"], check=True)
    field = c._message_field()
    field.click_input()
    time.sleep(0.5)
    keyboard.send_keys("{VK_CONTROL down}{v down}")
    time.sleep(0.6)
    keyboard.send_keys("{v up}{VK_CONTROL up}")
    time.sleep(2)
    c.evidence("po_ctrl_v")
    tree(c, "po_ctrl_v")


def settings(c):
    time.sleep(3)
    navs = sorted([e for e in c.win.descendants(control_type="Custom") if e.element_info.rectangle.width() < 70],
                  key=lambda e: e.element_info.rectangle.top)
    r = navs[1].element_info.rectangle
    x, y = (r.left + r.right) // 2, (r.top + r.bottom) // 2
    try:
        navs[1].invoke()
        print("invoke ok")
    except Exception as e:
        print("invoke failed", e)
        mouse.move(coords=(x, y + 2)); time.sleep(0.3); mouse.press(coords=(x, y)); time.sleep(0.15); mouse.release(coords=(x, y))
    time.sleep(2)
    c.evidence("ustawienia")
    tree(c, "ustawienia")


if __name__ == "__main__":
    work = Path(sys.argv[1])
    out = work / "out"
    out.mkdir(parents=True, exist_ok=True)
    c = Compass(work, out)
    try:
        globals()[sys.argv[2]](c)
    finally:
        c.close()
