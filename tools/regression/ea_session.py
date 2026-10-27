"""The user's running EA, for the tests of Compass in EA: the repository through COM (to prepare test data and to
check the result) and the EA window through UI Automation (to do what the test says a person does, e.g. delete
an element from the Project Browser, which is what makes EA ask the add-in)."""
import time
from pathlib import Path

import pythoncom
import win32com.client
from pywinauto import Application, Desktop, keyboard


class EA:
    def __init__(self, out: Path):
        self.out, self.shot_no = out, 0
        ctx, rot = pythoncom.CreateBindCtx(0), pythoncom.GetRunningObjectTable()
        found = [(m.GetDisplayName(ctx, None), m) for m in rot if m.GetDisplayName(ctx, None).startswith("!Sparx.EA.App:")]
        if not found:
            raise RuntimeError("Enterprise Architect nie jest uruchomiony")
        name, moniker = found[0]
        self.pid = int(name.split(":")[1])
        self.app = win32com.client.Dispatch(rot.GetObject(moniker).QueryInterface(pythoncom.IID_IDispatch))
        self.repo = self.app.Repository
        self.ui = Application(backend="uia").connect(process=self.pid)
        self.win = self.ui.top_window()

    def model_file(self):
        return self.repo.ConnectionString

    def find_package(self, name):
        def walk(pkgs):
            for i in range(pkgs.Count):
                p = pkgs.GetAt(i)
                if p.Name == name:
                    return p
                hit = walk(p.Packages)
                if hit:
                    return hit
            return None
        for i in range(self.repo.Models.Count):
            hit = walk(self.repo.Models.GetAt(i).Packages)
            if hit:
                return hit
        raise RuntimeError(f"brak pakietu {name}")

    def add_requirement(self, package, name, status):
        e = package.Elements.AddNew(name, "Requirement")
        e.Status = status
        e.Update()
        package.Elements.Refresh()
        self.repo.RefreshModelView(package.PackageID)
        return e.ElementID

    def exists(self, element_id):
        try:
            return self.repo.GetElementByID(element_id) is not None
        except Exception:
            return False

    def evidence(self, label, window=None):
        self.shot_no += 1
        w = window or self.win
        w.capture_as_image().save(self.out / f"{self.shot_no:02d}_ea_{label}.png")

    def delete_from_model_in_ui(self, element_id):
        """Selects the element in the Project Browser and deletes it as a person does (Ctrl+Delete, confirm Yes).
        Returns the message boxes that appeared (title, texts) until EA is idle again."""
        element = self.repo.GetElementByID(element_id)
        self.repo.ShowInProjectView(element)
        self.win.set_focus()
        time.sleep(1)
        keyboard.send_keys("^{DELETE}")
        boxes = []
        end = time.time() + 90
        while time.time() < end:
            time.sleep(1)
            dialogs = [w for w in Desktop(backend="uia").windows(process=self.pid) if w.handle != self.win.handle and w.is_visible()]
            dialogs += [w for w in self.win.children(control_type="Window")]
            if not dialogs:
                if boxes:
                    break
                continue
            d = dialogs[0]
            texts = [c.window_text() for c in d.descendants() if c.element_info.control_type == "Text" and c.window_text()]
            title = d.window_text()
            self.evidence(f"dialog_{len(boxes) + 1}", d)
            boxes.append((title, " | ".join(texts)))
            buttons = {c.window_text(): c for c in d.descendants(control_type="Button")}
            for name in ("Yes", "Tak", "OK"):
                if name in buttons:
                    buttons[name].click_input()
                    break
            else:
                break
        return boxes

    def system_output(self):
        """The texts of EA's System Output window (the "Loonar Compass" tab), if it is open."""
        texts = []
        for c in self.win.descendants():
            t = c.window_text() or ""
            if "🛡" in t or "Loonar Compass" in t:
                texts.append(t)
        return "\n".join(texts[-40:])
