"""Drives the real Compass window through Windows UI Automation (egui exposes its widgets via AccessKit): starts a
portable copy with the test model, clicks and types by the names a person sees, and saves screenshots and the
visible texts of the window as evidence. The screen must be unlocked while it runs."""
import shutil
import time
from pathlib import Path

from pywinauto import Application, keyboard, mouse

from services import CONFIG

SEND = "Wyślij"  # the send button (its accessible name in the Polish UI the tests run in)


class Compass:
    def __init__(self, work: Path, out: Path, model=None, settings=""):
        """A portable Compass in `work` with a copy of the test model (or `model`) and extra `settings` lines."""
        self.work, self.out, self.shot_no = work, out, 0
        shutil.copy2(CONFIG["compass_exe"], work / "LoonarCompass.exe")
        if model == "sqlserver":  # the SQL Server test repository instead of a file
            m = CONFIG["mssql"]
            (work / "app.properties").write_text(f"model.source=sqlserver\ndb.host={m['host']}\ndb.port={m['port']}\ndb.name={m['database']}\n"
                                                 f"db.user={m['user']}\ndb.password={m['password']}\nui.language=pl\n{settings}", encoding="utf-8")
        else:
            source = Path(model or CONFIG["test_model"])
            shutil.copy2(source, work / source.name)
            (work / "app.properties").write_text(f"model.source=file\nmodel.file={source.name}\nui.language=pl\n{settings}", encoding="utf-8")
        self.app = Application(backend="uia").start(str(work / "LoonarCompass.exe"), work_dir=str(work))
        self.win = self.app.window(title="Loonar Compass", found_index=0)  # an inner window (e.g. a picker) has the same title
        self.win.wait("visible ready", timeout=60)
        self.win.set_focus()
        self.win.maximize()  # the same layout on every screen, nothing under the taskbar
        time.sleep(1)

    def close(self):
        try:
            self.app.kill()
        except Exception:
            pass

    # Finding and acting by visible names ----------------------------------------------------------------------
    def find(self, name, control_type="Button", timeout=20, contains=False):
        end = time.time() + timeout
        while time.time() < end:
            for c in self.win.descendants(control_type=control_type):
                text = c.element_info.name or ""
                if (name in text) if contains else (text == name):
                    return c
            time.sleep(0.5)
        raise RuntimeError(f"not found in the window: {control_type} \"{name}\"")

    def click(self, name, control_type="Button", contains=False, timeout=20, expect=None):
        """Clicks like a person: the pointer first rests on the control (egui reacts to a hovered control), then
        clicks. With `expect`, the click is repeated until that text appears in the window."""
        for attempt in range(3):
            control = self.find(name, control_type, timeout, contains)
            r = control.element_info.rectangle
            mouse.move(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
            time.sleep(0.4)
            mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
            time.sleep(1.0)
            if not expect or (expect() if callable(expect) else self.shows(expect)):
                return
        raise RuntimeError(f"after clicking \"{name}\", \"{expect}\" did not appear")

    def shows(self, text):
        """Whether any part of the window (also a title of an inner window) shows this text."""
        return any(text in (c.element_info.name or "") for c in self.win.descendants())

    def texts(self):
        """Every text the window shows now, top to bottom (what a person could read)."""
        items = []
        for c in self.win.descendants():
            name = (c.element_info.name or "").strip()
            if name and c.element_info.control_type in ("Text", "Button", "Edit", "Document", "ListItem", "CheckBox", "RadioButton", "Hyperlink"):
                r = c.element_info.rectangle
                items.append((r.top, r.left, f"[{c.element_info.control_type}] {name}"))
        return "\n".join(t for _, _, t in sorted(items))

    def evidence(self, label):
        """A screenshot and the visible texts of the window, numbered in the order they were taken."""
        self.shot_no += 1
        stem = f"{self.shot_no:02d}_{label}"
        self.win.set_focus()
        time.sleep(0.5)
        self.win.capture_as_image().save(self.out / f"{stem}.png")
        (self.out / f"{stem}_teksty.txt").write_text(self.texts(), encoding="utf-8")

    # Conversation ---------------------------------------------------------------------------------------------
    def start_conversation(self):
        self.click("Zatwierdź model i rozpocznij", timeout=30, expect=lambda: not self.shows("Zatwierdź model i rozpocznij"))

    def say(self, message, timeout=600, watch=None):
        """Types the message into the message field, sends it and waits until the assistant has finished
        (returns "done" or "approval")."""
        field = self._message_field()
        field.click_input()
        keyboard.send_keys("^a{BACKSPACE}", pause=0.02)
        field.type_keys(message.replace("{", "{{}").replace("}", "{}}").replace("+", "{+}").replace("^", "{^}").replace("%", "{%}").replace("~", "{~}").replace("(", "{(}").replace(")", "{)}"),
                        with_spaces=True, pause=0.005)
        self.click(SEND, expect=lambda: not (self._message_field().get_value() or "").strip())
        return self.wait_idle(timeout, watch)

    def _message_field(self):
        edits = [c for c in self.win.descendants(control_type="Edit")]
        return max(edits, key=lambda c: c.element_info.rectangle.width())  # the wide field under the conversation

    APPROVAL = ("Zatwierdź", "Odrzuć")  # the approval buttons of the Polish UI the tests run in

    def wait_idle(self, timeout=600, watch=None):
        """Waits until the assistant has finished: the send button is back and no status says it is working ("…"),
        steadily for a few seconds; returns "approval" early when a write waits for approval. `watch` takes
        evidence every 15 s while it works (for things that change during the work, like the plan)."""
        end, started, last_watch = time.time() + timeout, time.time(), time.time()
        time.sleep(2)
        calm = 0
        while time.time() < end:
            try:
                buttons = [(c.element_info.name or "").strip() for c in self.win.descendants(control_type="Button")]
                texts = self.win.descendants(control_type="Text")
            except Exception:  # the window is busy redrawing: UI Automation may miss it for a moment
                time.sleep(1)
                continue
            if any(b in self.APPROVAL for b in buttons):
                return "approval"
            working = any((c.element_info.name or "").strip().endswith("…") and "Napisz" not in (c.element_info.name or "")
                          for c in texts)
            calm = calm + 1 if SEND in buttons and not working else 0
            if calm >= 3:
                return "done"
            if watch and time.time() - last_watch > 15:
                self.evidence(f"{watch}_during_{int(time.time() - started)}s")
                last_watch = time.time()
            time.sleep(1)
        raise RuntimeError("the assistant did not finish in time")

    def answer_form(self):
        """Fills a question form like a person in a hurry: the first option of every question."""
        for _ in range(6):  # the form may start above the visible part of the conversation
            radios = sorted(self.win.descendants(control_type="RadioButton"), key=lambda c: c.element_info.rectangle.top)
            if radios and radios[0].element_info.rectangle.top > self.win.rectangle().top + 120:
                break
            r = self.win.rectangle()
            mouse.scroll(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2), wheel_dist=5)
            time.sleep(0.5)
        groups, last = [], None
        for radio in sorted(self.win.descendants(control_type="RadioButton"), key=lambda c: c.element_info.rectangle.top):
            top = radio.element_info.rectangle.top
            if last is None or top - last > 45:
                groups.append(radio)
            last = top
        for first in groups:
            r = first.element_info.rectangle
            mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
            time.sleep(0.4)
        return len(groups)

    def attach(self, path):
        """Attaches a file the way a person can without dragging: copied in Explorer, pasted with the paste button."""
        import subprocess
        subprocess.run(["powershell", "-NoProfile", "-Command", f"Set-Clipboard -Path '{path}'"], check=True)
        self._message_field().click_input()
        time.sleep(0.5)
        # Held for a moment: the window checks the keys while it draws a frame, a quick press can fall between frames.
        keyboard.send_keys("{VK_CONTROL down}{v down}")
        time.sleep(0.6)
        keyboard.send_keys("{v up}{VK_CONTROL up}")
        time.sleep(1.5)

    def approve(self, yes=True):
        """Answers a pending approval and waits for the work to go on."""
        self.click("Zatwierdź" if yes else "Odrzuć")
        return self.wait_idle()

    def say_approving(self, message, yes=True, label="approval"):
        """Sends a message and answers every approval it asks for (with evidence of each approval)."""
        state, n = self.say(message), 0
        while state == "approval" and n < 6:
            n += 1
            self.evidence(f"{label}_{n}")
            state = self.approve(yes)
        return state

    def files(self):
        """Files in the conversation attachments folder of the portable copy."""
        folder = self.work / "attachments"
        return sorted(p for p in folder.rglob("*") if p.is_file()) if folder.exists() else []

    def steps(self):
        """Expands the steps of the last answer (if collapsed)."""
        try:
            self.click("rozwiń", control_type="Text", timeout=3)
        except RuntimeError:
            try:
                self.click("Kroki ostatniej odpowiedzi", control_type="Text", contains=True, timeout=3)
            except RuntimeError:
                pass
