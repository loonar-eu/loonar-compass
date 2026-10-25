"""How each automatable test case is executed. An executor gets the folder for its evidence and writes into it
what a person would look at: command output, created files, answers, screenshots. It does not decide the result;
the judge does. Test cases without an executor stay for a person."""
import shutil
import subprocess
from pathlib import Path

from services import CONFIG

EXECUTORS = {}


def executor(name):
    """Registers an executor by the test case name (keys differ between projects, names do not)."""
    def register(fn):
        EXECUTORS[name] = fn
        return fn
    return register


def run(cmd, out: Path, name, cwd=None, timeout=600):
    """Runs a command, saves the command and its output as evidence, returns the exit code."""
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)
    (out / f"{name}.txt").write_text(f"> {' '.join(map(str, cmd))}\nexit code: {p.returncode}\n\n{p.stdout}\n{p.stderr}", encoding="utf-8")
    return p.returncode


def sqlserver_properties(folder: Path):
    """A portable settings file for the SQL Server test repository (the password stays in this file, which is not
    the installed application's settings, so nothing is written to Credential Manager)."""
    m = CONFIG["mssql"]
    props = folder / "app.properties"
    props.write_text(f"model.source=sqlserver\ndb.host={m['host']}\ndb.port={m['port']}\ndb.name={m['database']}\ndb.user={m['user']}\ndb.password={m['password']}\n", encoding="utf-8")
    return props


def powershell(command, out: Path, name, cwd: Path):
    """Runs the command exactly as a test case writes it, in PowerShell, in the given folder."""
    return run(["powershell", "-NoProfile", "-Command", command], out, name, cwd=cwd)


def compass_in(work: Path):
    """The application under test as LoonarCompass.exe in the working folder, as the guide names it."""
    shutil.copy2(CONFIG["compass_exe"], work / "LoonarCompass.exe")


@executor("Kontrola modelu z wiersza poleceń (SQL Server)")
def check_sqlserver(out: Path, work: Path):
    sqlserver_properties(work)
    compass_in(work)
    powershell(".\\LoonarCompass.exe --check --model app.properties --report report.md | Out-Host", out, "1_command", work)
    shutil.copy2(work / "report.md", out / "report.md")
    # The same check on the file model, for the comparison in step 2 ("as for a file").
    shutil.copy2(CONFIG["test_model"], work / "Sklep.qea")
    powershell(".\\LoonarCompass.exe --check --model Sklep.qea --report report_qea_file.md | Out-Host", out, "2_comparison_qea_file", work)
    shutil.copy2(work / "report_qea_file.md", out / "report_qea_file.md")


# A. Conversation: new buttons and behaviour (the real window) ---------------------------------------------------
from ui import Compass  # noqa: E402  (UI Automation is only needed by the window tests)


def window_test(fn):
    """Runs a window test with a fresh portable Compass, always closing it, with a final screenshot on errors."""
    def run_in_window(out: Path, work: Path):
        c = Compass(work, out)
        try:
            fn(c)
        except Exception:
            c.evidence("state_at_error")
            raise
        finally:
            c.close()
    return run_in_window


@executor("Wybór elementu z modelu (⌖)")
@window_test
def picker(c):
    from pywinauto import keyboard
    c.start_conversation()
    c.evidence("bar_under_message_field")
    # The search window has its own text field (its title is painted, not exposed to UI Automation).
    c.click("⌖", expect=lambda: len(c.win.descendants(control_type="Edit")) >= 3)
    c.evidence("search_window")
    keyboard.send_keys("ord", pause=0.05)
    import time
    time.sleep(2)
    c.evidence("suggestions_for_ord")
    c.click("Order  ·  Class", contains=True)
    c.evidence("message_after_pick")


@executor("Rozumowanie asystenta (💭)")
@window_test
def reasoning(c):
    c.start_conversation()
    c.click("💭")
    c.evidence("reasoning_on")
    c.say("Przeanalizuj, które klasy w pakiecie Shop domain nie mają opisu i co z tym zrobić.")
    c.evidence("answer")
    c.steps()
    c.evidence("answer_steps")
    c.click("💭")
    c.evidence("reasoning_off")


@executor("Pytania doprecyzowujące (formularz)")
@window_test
def questions(c):
    c.start_conversation()
    if c.say("Dodaj nową klasę płatności do modelu.") == "approval":
        # The assistant went straight to a change instead of asking: that is the evidence; the change is refused.
        c.evidence("write_request_instead_of_questions")
        c.click("Odrzuć")
        c.wait_idle()
        c.evidence("change_refused")
        return
    c.evidence("question_form")
    c.answer_form()
    c.evidence("form_filled")
    c.click("Wyślij odpowiedzi", timeout=10)
    state = c.wait_idle()
    c.evidence("after_sending_answers")
    if state == "approval":
        c.click("Odrzuć")
        c.wait_idle()
        c.evidence("change_refused")


@executor("Lista zadań asystenta (📋 Plan)")
@window_test
def plan(c):
    c.start_conversation()
    c.say("Przejrzyj pakiet Shop domain w 4 krokach: klasy, relacje, kontrola modelu, 3 najważniejsze problemy.", watch="plan")
    c.steps()
    c.evidence("plan_at_end")


@executor("Wykonywanie kodu w piaskownicy (∑)")
@window_test
def sandbox(c):
    c.start_conversation()
    c.click("∑")
    c.evidence("code_on")
    c.say("Policz w Pythonie średnią liczbę atrybutów na klasę w pakiecie Shop domain.")
    c.evidence("answer")
    c.steps()
    c.evidence("steps_with_code")
    c.click("∑")
    c.evidence("code_off")
