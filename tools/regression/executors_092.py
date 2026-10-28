"""Executors of the 0.9.1 regression (cycle LC-R3) and of the 0.9.2 cases: the real window where a person would use
it, live tests of the program (cargo) where the check is in the program's own test, web pages where the check is on
loonar.eu. Each one only gathers evidence; the judge decides."""
import json
import re
import os
import shutil
import subprocess
import time
import urllib.request
from pathlib import Path

from pywinauto import Application, mouse

from executors import executor, run
from services import CONFIG, secret
from ui import Compass

REPO = Path(CONFIG["compass_exe"]).parents[2]
TEMPLATES = Path(CONFIG.get("test_templates", str(Path(__file__).with_name("templates_092"))))  # the user's sample templates
SCRATCH = Path(os.environ.get("LOONAR_REGRESSION_SCRATCH", str(Path(__file__).with_name("out") / "scratch")))


# Helpers ---------------------------------------------------------------------------------------------------------
def fresh(out: Path, work: Path, settings="", telemetry=False, keep_notice=False):
    """A portable Compass. Statistics are off unless the test is about them (test copies must not count as
    installations); the first-run notice about statistics is closed unless the test is about it."""
    if telemetry:
        os.environ.pop("LOONAR_TELEMETRY", None)
    else:
        os.environ["LOONAR_TELEMETRY"] = "0"
    c = Compass(work, out, settings=settings)
    if not keep_notice:
        for name in ("Zostaw włączone", "Wyłącz"):
            if c.shows(name):
                try:
                    c.click(name, timeout=3)
                except RuntimeError:
                    pass
                break
    return c


def restart(c: Compass):
    """Closes the window and starts the same portable copy again (same settings and data)."""
    c.close()
    time.sleep(2)
    c.app = Application(backend="uia").start(str(c.work / "LoonarCompass.exe"), work_dir=str(c.work))
    c.win = c.app.window(title="Loonar Compass", found_index=0)
    c.win.wait("visible ready", timeout=60)
    c.win.set_focus()
    c.win.maximize()
    time.sleep(2)


def in_window(fn, **kw):
    """A window test with its own settings: always closes the window, a screenshot on errors."""
    def runner(out: Path, work: Path):
        c = None
        try:
            c = fresh(out, work, **kw)
            fn(c)
        except Exception:
            if c:
                c.evidence("state_at_error")
            raise
        finally:
            if c:
                c.close()
            os.environ.pop("LOONAR_TELEMETRY", None)
    return runner


def click_text(c: Compass, text, exact=True, timeout=15):
    """Clicks a visible text of any kind (button, link, card title), scrolling the middle of the window to it."""
    r = c.win.rectangle()
    end = time.time() + timeout
    while time.time() < end:
        found = [d for d in c.win.descendants() if ((d.element_info.name or "") == text if exact else text in (d.element_info.name or ""))]
        visible = [d for d in found if r.top + 40 < d.element_info.rectangle.top < r.bottom - 40]
        if visible:
            box = visible[0].element_info.rectangle
            mouse.move(coords=((box.left + box.right) // 2, (box.top + box.bottom) // 2))
            time.sleep(0.4)
            mouse.click(coords=((box.left + box.right) // 2, (box.top + box.bottom) // 2))
            time.sleep(1.2)
            return
        mouse.scroll(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2), wheel_dist=-3)
        time.sleep(0.4)
    raise RuntimeError(f"text \"{text}\" not found in the window")


def nav(c: Compass, name):
    c.click(name)
    time.sleep(1.5)


def wiggle(c: Compass):
    """Moves the pointer a little: the window draws its next frame (e.g. a changed button) on input."""
    r = c.win.rectangle()
    for dx in (0, 40, 0):
        mouse.move(coords=(r.left + 900 + dx, r.top + 600))
        time.sleep(0.3)
    time.sleep(0.5)


def row_button(c: Compass, text, button):
    """Clicks `button` in the row of the card titled `text` (scrolled into view)."""
    r = c.win.rectangle()
    title = None
    for _ in range(40):
        found = [d for d in c.win.descendants(control_type="Text") if (d.element_info.name or "") == text]
        if found and r.top + 40 < found[0].element_info.rectangle.top < r.bottom - 60:
            title = found[0]
            break
        mouse.scroll(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2), wheel_dist=-3)
        time.sleep(0.4)
    if title is None:
        raise RuntimeError(f"card \"{text}\" not reached by scrolling")
    top = title.element_info.rectangle.top
    target = min((b for b in c.win.descendants(control_type="Button") if (b.element_info.name or "") == button),
                 key=lambda b: abs(b.element_info.rectangle.top - top))
    r = target.element_info.rectangle
    mouse.move(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
    time.sleep(0.4)
    mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
    time.sleep(1)


def cargo_live(out: Path, test: str, name: str, env=None, timeout=1800):
    """A live test of the program (cargo test --ignored) with its output as evidence."""
    full = dict(os.environ, **(env or {}))
    p = subprocess.run(["cargo", "test", "--bin", "loonar-compass", test, "--", "--ignored", "--nocapture", "--test-threads=1"],
                       cwd=REPO, env=full, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)
    lines = [l for l in (p.stdout + p.stderr).splitlines() if not l.startswith(("   Compiling", "warning", "  ", "    ", "  |", "   |", "    |")) or "panicked" in l]
    (out / f"{name}.txt").write_text(f"> cargo test {test} -- --ignored\nexit code: {p.returncode}\n\n" + "\n".join(lines[-400:]), encoding="utf-8")
    return p.returncode


def cargo_unit(out: Path, filters, name):
    """Unit tests of the program (no network), only the lines about the tests as evidence."""
    lines = []
    for f in filters:
        p = subprocess.run(["cargo", "test", "--bin", "loonar-compass", f], cwd=REPO, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=1800)
        lines.append(f"> cargo test {f}   (exit code {p.returncode})")
        lines += [l for l in (p.stdout + p.stderr).splitlines() if l.startswith(("test ", "running ", "failures", "---- ")) or "panicked" in l]
    (out / f"{name}.txt").write_text(chr(10).join(lines), encoding="utf-8")


def jira_env():
    return {"LOONAR_TEST_JIRA_URL": CONFIG["site"], "LOONAR_TEST_JIRA_USER": CONFIG["account"], "LOONAR_TEST_JIRA_TOKEN": secret(CONFIG["jira_credential"])}


def test_props(out: Path):
    """Settings for the live tests: a copy of the test model."""
    folder = SCRATCH / "live"
    folder.mkdir(parents=True, exist_ok=True)
    shutil.copy2(CONFIG["test_model"], folder / "Sklep.qea")
    props = folder / "app.properties"
    props.write_text(f"model.source=file\nmodel.file={(folder / 'Sklep.qea').as_posix()}\nui.language=pl\n", encoding="utf-8")
    return props


def fetch(url, out: Path, name):
    req = urllib.request.Request(url, headers={"User-Agent": "LoonarCompass-regression/1"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read().decode("utf-8", errors="replace")
            status = r.status
    except Exception as e:
        body, status = f"{type(e).__name__}: {e}", 0
    (out / f"{name}.txt").write_text(f"GET {url}\nstatus: {status}\n\n{body}", encoding="utf-8")
    return body


def html_text(html):
    import re
    html = re.sub(r"(?s)<(script|style).*?</\1>", " ", html)
    text = re.sub(r"<[^>]+>", " ", html)
    return re.sub(r"[ \t]+", " ", re.sub(r"\n\s*\n+", "\n", text))


def sample_services(root: Path):
    """Three small Java services with REST controllers (for the source code tests)."""
    for svc, ops in (("orders", ["GET /orders/{orderId}", "POST /orders"]), ("payments", ["POST /payments", "GET /payments/{paymentId}/status"]),
                     ("customers", ["GET /customers/{customerId}", "PUT /customers/{customerId}/address"])):
        folder = root / svc / "src" / "main" / "java" / "shop" / svc
        folder.mkdir(parents=True, exist_ok=True)
        methods = []
        for op in ops:
            verb, path = op.split(" ")
            ann = {"GET": "GetMapping", "POST": "PostMapping", "PUT": "PutMapping"}[verb]
            fn = (verb.lower() + "".join(p.strip("{}").title() for p in path.split("/") if p))[:40]
            methods.append(f'    @{ann}("{path}")\n    public String {fn}() {{ return "ok"; }}\n')
        cls = svc.title() + "Controller"
        (folder / f"{cls}.java").write_text(f"package shop.{svc};\n\nimport org.springframework.web.bind.annotation.*;\n\n@RestController\npublic class {cls} {{\n" + "\n".join(methods) + "}\n", encoding="utf-8")
    return root


# 0.9.1 regression (LC-R3) --------------------------------------------------------------------------------------
@executor("Pierwsze uruchomienie: informacja o statystykach")
def first_run_notice(out: Path, work: Path):
    def body(c):
        c.evidence("first_start")
        click_text(c, "Pokaż szczegóły")
        c.evidence("details_view")
        restart(c)
        c.evidence("second_start_no_notice")
        (out / "app_properties.txt").write_text((c.work / "app.properties").read_text(encoding="utf-8"), encoding="utf-8")
    in_window(body, telemetry=True, keep_notice=True)(out, work)


@executor("Ping przy starcie z identyfikatorem instalacji")
def start_ping(out: Path, work: Path):
    def body(c):
        time.sleep(5)
        nav(c, "Statystyki użycia")
        c.evidence("stats_view_after_start")
        ids = sorted(str(p) for p in c.work.rglob("install_id"))
        first = [Path(p).read_text(encoding="utf-8").strip() for p in ids]
        restart(c)
        time.sleep(5)
        nav(c, "Statystyki użycia")
        c.evidence("stats_view_after_restart")
        second = [Path(p).read_text(encoding="utf-8").strip() for p in ids]
        (out / "install_id.txt").write_text(f"files: {ids}\nafter 1st start: {first}\nafter 2nd start: {second}\nsame: {first == second}", encoding="utf-8")
    in_window(body, telemetry=True)(out, work)
    fetch("http://127.0.0.1:8105/", out, "author_dashboard_8105")


@executor("Ocena odpowiedzi 👍/👎 z powodem")
def rating(out: Path, work: Path):
    def body(c):
        c.start_conversation()
        c.say("Ile pakietów ma model? Jedno zdanie.")
        c.say("Wymień nazwy pakietów najwyższego poziomu. Krótko.")
        ups = [b for b in c.win.descendants(control_type="Button") if "👍" in (b.element_info.name or "")]
        downs = [b for b in c.win.descendants(control_type="Button") if "👎" in (b.element_info.name or "")]
        (out / "rating_buttons.txt").write_text(f"👍 buttons: {len(ups)}, 👎 buttons: {len(downs)}", encoding="utf-8")
        for b, label in ((ups[0], "after_thumb_up"),):
            r = b.element_info.rectangle
            mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
            time.sleep(1)
            c.evidence(label)
        downs = [b for b in c.win.descendants(control_type="Button") if "👎" in (b.element_info.name or "")]
        r = downs[-1].element_info.rectangle
        mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
        time.sleep(1)
        c.evidence("thumb_down_menu")
        c.click("Za wolno", contains=True, timeout=5)
        time.sleep(1)
        nav(c, "Statystyki użycia")
        c.evidence("stats_today")
    in_window(body, telemetry=True)(out, work)


@executor("„💡 Zgłoś zapotrzebowanie”, gdy asystent czegoś nie umie")
def gap_request(out: Path, work: Path):
    def body(c):
        c.start_conversation()
        c.say("Zaimportuj model z pliku StarUML .mdj")
        c.evidence("answer_with_gap_button")
        c.click("💡 Zgłoś zapotrzebowanie", contains=True, timeout=10)
        time.sleep(1.5)
        c.evidence("request_form_not_sent")
        (out / "note.txt").write_text("The form was opened but NOT sent: sending creates a public GitHub issue (step 3 is for a person).", encoding="utf-8")
        nav(c, "Statystyki użycia")
        c.evidence("stats_today_gaps")
    in_window(body, telemetry=True)(out, work)


@executor("Wyłączenie statystyk: widok, zmienna, rejestr (ADMX), hub")
def telemetry_off(out: Path, work: Path):
    def body(c):
        nav(c, "Statystyki użycia")
        c.evidence("stats_on")
        box = next(b for b in c.win.descendants(control_type="CheckBox") if "Wysyłaj" in (b.element_info.name or ""))
        r = box.element_info.rectangle
        mouse.click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
        time.sleep(1.5)
        c.evidence("stats_switched_off")
        c.close()
        os.environ["LOONAR_TELEMETRY"] = "0"
        restart(c)
        nav(c, "Statystyki użycia")
        c.evidence("env_LOONAR_TELEMETRY_0")
        (out / "note.txt").write_text("Steps 3 (ADMX policy) and 4 (hub policy) run in Compass Lab.", encoding="utf-8")
    in_window(body, telemetry=True)(out, work)


@executor("Nowy identyfikator instalacji")
def new_install_id(out: Path, work: Path):
    def body(c):
        time.sleep(3)
        ids = sorted(c.work.rglob("install_id"))
        before = [p.read_text(encoding="utf-8").strip() for p in ids]
        nav(c, "Statystyki użycia")
        c.click("Nowy identyfikator", timeout=10)
        time.sleep(1.5)
        c.evidence("after_new_identifier")
        after = [p.read_text(encoding="utf-8").strip() for p in ids]
        restart(c)
        time.sleep(4)
        nav(c, "Statystyki użycia")
        c.evidence("after_restart")
        (out / "install_id.txt").write_text(f"before: {before}\nafter click: {after}\nchanged: {before != after}", encoding="utf-8")
    in_window(body, telemetry=True)(out, work)


@executor("Usługa statystyk i pulpit autora")
def stats_service(out: Path, work: Path):
    fetch("https://compass-feedback.loonar-eu.workers.dev/health", out, "1_health")
    fetch("https://compass-feedback.loonar-eu.workers.dev/v1/summary", out, "2_summary")
    page = fetch("http://127.0.0.1:8105/", out, "3_author_dashboard_raw")
    text = html_text(page)
    sections = ", ".join(f"{h}: {'present' if h in text else 'MISSING'}" for h in ("Website and downloads", "The application"))
    (out / "3_author_dashboard.txt").write_text(f"Author's dashboard (python tools/stats/dashboard.py --serve 8105), as text. Sections: {sections}\n" + text[:5000], encoding="utf-8")
    (out / "3_author_dashboard_raw.txt").unlink()
    (out / "note.txt").write_text("Step 4 (D1 retention) needs the Cloudflare console: for a person.", encoding="utf-8")


@executor("Polityka prywatności na loonar.eu (po wydaniu)")
def privacy(out: Path, work: Path):
    for lang, url in (("pl", "https://loonar.eu/pl/privacy/"), ("en", "https://loonar.eu/privacy/"), ("de", "https://loonar.eu/de/privacy/"), ("fr", "https://loonar.eu/fr/privacy/")):
        body = fetch(url, out, f"privacy_{lang}_raw")
        text = html_text(body)
        dates = sorted(set(re.findall(r"\d{1,2}[.\s]\d{1,2}[.\s]20\d\d|20\d\d-\d\d-\d\d|\d{1,2}\.? \w+ 20\d\d", text)))
        start = max(0, min([i for i in (text.find(w) for w in ("Statystyki użycia", "Usage statistics", "Nutzungsstatistik", "Statistiques d")) if i >= 0] or [0]))
        (out / f"privacy_{lang}.txt").write_text(f"{url}\nDates on the page: {dates}\n\nThe usage statistics section:\n{text[start:start + 4500]}", encoding="utf-8")
        (out / f"privacy_{lang}_raw.txt").unlink()


@executor("Strona i instrukcja 0.9.1 (po wydaniu)")
def website(out: Path, work: Path):
    for lang, url in (("pl", "https://loonar.eu/pl/"), ("en", "https://loonar.eu/"), ("de", "https://loonar.eu/de/"), ("fr", "https://loonar.eu/fr/")):
        body = fetch(url, out, f"site_{lang}_raw")
        text = html_text(body)
        keys = [l.strip() for l in text.splitlines() if any(w in l for w in ("0.9.1", "SHA", "SharePoint", "OneDrive", "Google"))]
        summary = (f"Long dashes (em dash, U+2014) in the page source: {body.count(chr(0x2014))}. "
                   f"(Short dashes U+2013, the typographic dash of German and French, are not long dashes: {body.count(chr(0x2013))}.)\n"
                   + "Lines with 0.9.1 / SHA / SharePoint / OneDrive / Google:\n" + "\n".join(keys[:40]))
        (out / f"site_{lang}.txt").write_text(summary + "\n\n" + text, encoding="utf-8")
        (out / f"site_{lang}_raw.txt").unlink()
    docs = REPO / "docs" / "Instrukcja_uzytkownika.html"
    text = html_text(docs.read_text(encoding="utf-8"))
    keep = [l for l in text.splitlines() if any(w in l for w in ("innych modeli", "Eksport", "Import", "streszcz", "Koszt", "Zatwierdź wszystkie", "Spróbuj ponownie", "0.9.1", "0.9"))]
    (out / "manual_pl_excerpts.txt").write_text("\n".join(keep[:200]), encoding="utf-8")
    archive = fetch("https://loonar.eu/docs/0.9/pl/", out, "docs_archive_0.9_pl_raw")
    (out / "docs_archive_0.9_pl.txt").write_text("https://loonar.eu/docs/0.9/pl/ (archive of the 0.9 manual):\n" + html_text(archive)[:3000], encoding="utf-8")
    (out / "docs_archive_0.9_pl_raw.txt").unlink()


@executor("Adres Jira/Confluence z przeglądarki (/spaces, /browse)")
def service_urls(out: Path, work: Path):
    cargo_unit(out, ["keys_and_addresses", "base"], "unit_tests_base_url")
    def body(c):
        nav(c, "Ustawienia")
        c.evidence("settings")
        click_text(c, "Confluence")
        c.evidence("confluence_card")
    in_window(body)(out, work)


@executor("Kod źródłowy: karta Ustawień i odczyt")
def source_code(out: Path, work: Path):
    repo = sample_services(work / "repos")
    def body(c):
        nav(c, "Ustawienia")
        click_text(c, "Kod źródłowy")
        c.evidence("source_code_card")
        nav(c, "Czat")
        c.start_conversation()
        c.say("wiesz gdzie mam kod?")
        c.evidence("where_is_the_code")
        c.say("Wypisz publiczne operacje REST kontrolerów we wszystkich usługach w moim kodzie.")
        c.steps()
        c.evidence("rest_operations")
    in_window(body, settings=f"code.folders={repo.as_posix()}\n")(out, work)


@executor("Asystent pamięta kroki poprzednich odpowiedzi")
def remembers_steps(out: Path, work: Path):
    repo = sample_services(work / "repos")
    def body(c):
        c.start_conversation()
        c.say_approving("Przeczytaj kod kontrolera zamówień (OrdersController) w moim kodzie i dodaj do modelu w pakiecie najwyższego poziomu klasę OrdersController z operacjami z tego kontrolera.")
        c.steps()
        c.evidence("first_answer_with_steps")
        c.say("czy czytałeś kod źródłowy w poprzedniej odpowiedzi? co dokładnie zrobiłeś?")
        c.evidence("second_answer")
        (out / "note.txt").write_text("The source code stands in for the Confluence page of the test case (the check is the same: the assistant remembers what its earlier steps did).", encoding="utf-8")
    in_window(body, settings=f"code.folders={repo.as_posix()}\napproval.mode=auto\n")(out, work)


@executor("Samodzielna praca do końca, bez „czy kontynuować?”")
def autonomy(out: Path, work: Path):
    repo = sample_services(work / "repos")
    def body(c):
        c.start_conversation()
        c.say_approving("Dla każdej z 3 usług w moim kodzie (orders, payments, customers) dodaj do modelu w nowym pakiecie „Usługi REST” interfejs z operacjami REST tej usługi; ścieżkę REST zapisz w aliasie operacji.", label="approval")
        c.steps()
        c.evidence("one_answer_all_services")
    in_window(body, settings=f"code.folders={repo.as_posix()}\napproval.mode=auto\n")(out, work)


@executor("Skrypty EA i „Zatwierdź wszystkie … do zamknięcia Compassa”")
def scripts_approve_all(out: Path, work: Path):
    def body(c):
        c.start_conversation()
        state = c.say("W pakiecie najwyższego poziomu utwórz klasę AliasTest z operacjami getOrder i createOrder, a operacjom ustaw aliasy GET /orders/{orderId} i POST /orders.")
        c.steps()
        c.evidence("aliases_without_script")
        state = c.say("Teraz koniecznie użyj skryptu EA (JScript), żeby wypisać nazwy wszystkich pakietów modelu w oknie wyjścia skryptu.")
        if state == "approval":
            c.evidence("script_approval_card")
            c.click("Zatwierdź wszystkie skrypty", contains=True, timeout=10)
            c.wait_idle()
        c.evidence("after_first_script")
        state = c.say("Uruchom jeszcze raz taki sam skrypt EA, ale dla elementów pakietu najwyższego poziomu.")
        c.evidence("second_script_no_question" if state == "done" else "second_script_asked_again")
        (out / "note.txt").write_text("Step 3 (5 Jira issues) is not run: it would create issues in the real Jira; covered by the program's unit tests with a Jira imitation.", encoding="utf-8")
    in_window(body, settings="approval.mode=auto\n")(out, work)
    cargo_unit(out, ["approval_questions_are_recognised_by_kind", "scripts_differing_only_in_data_have_one_build", "other_code_is_another_build"], "unit_tests_approve_all")


@executor("Koszt odpowiedzi, cache i długie rozmowy")
def cost_and_cache(out: Path, work: Path):
    def body(c):
        c.start_conversation()
        c.say("Ile klas jest w modelu i w jakich pakietach? Krótko.")
        c.steps()
        c.evidence("cost_step")
        c.say("Od której wersji Compass obsługuje certyfikat klienta (mTLS) dla Jiry?")
        c.steps()
        c.evidence("changelog_answer")
        logs = sorted((c.work / "usage").glob("*.jsonl")) if (c.work / "usage").exists() else sorted(c.work.rglob("usage/*.jsonl"))
        (out / "usage_log.txt").write_text("\n\n".join(f"{p}:\n{p.read_text(encoding='utf-8')}" for p in logs) or "no usage log found", encoding="utf-8")
    in_window(body)(out, work)
    # The user's own settings (memory, templates and instructions are part of the prompt the threshold is about).
    session = max(Path(os.environ["APPDATA"], "Loonar Compass", "history").glob("*.json"), key=lambda p: p.stat().st_size)
    cargo_live(out, "long_conversations_live", "long_conversation_live_test", {"LOONAR_TEST_PROPS": str(Path(os.environ["APPDATA"], "Loonar Compass", "app.properties")), "LOONAR_TEST_SESSION": str(session)})


# 0.9.2 (LC-303 … LC-311) ---------------------------------------------------------------------------------------
def with_templates(work: Path, legacy=True):
    folder = work / "templates"
    folder.mkdir(parents=True, exist_ok=True)
    for f in TEMPLATES.glob("*.md"):
        shutil.copy2(f, folder / f.name)
    if legacy:  # an untouched template shipped by 0.9.1
        shutil.copy2(REPO / "templates" / "pl" / "BRD.md", folder / "BRD - Wymagania biznesowe.md")
    return folder


@executor("Szablony: lista według narzędzia i grupy, stare szablony w koszu")
def templates_list(out: Path, work: Path):
    folder = with_templates(work)
    def body(c):
        nav(c, "Szablony dokumentów")
        c.evidence("templates_view")
        files = "\n".join(sorted(str(p.relative_to(folder)) for p in folder.rglob("*.md")))
        (out / "templates_folder.txt").write_text(f"{folder}:\n{files}", encoding="utf-8")
    in_window(body)(out, work)


@executor("Szablony: dodanie, właściwości i usunięcie")
def templates_edit(out: Path, work: Path):
    folder = with_templates(work, legacy=False)
    def body(c):
        nav(c, "Szablony dokumentów")
        c.click("➕ Nowy szablon", timeout=10)
        time.sleep(1)
        c.evidence("new_template_form")
        form_top = c.find("Nowy szablon", control_type="Text").element_info.rectangle.top
        edits = sorted([e for e in c.win.descendants(control_type="Edit") if e.element_info.rectangle.top > form_top], key=lambda e: e.element_info.rectangle.top)
        edits[0].click_input(); edits[0].type_keys("Notatka ze spotkania", with_spaces=True)
        edits[1].click_input(); edits[1].type_keys("Krótka notatka ze spotkania projektowego", with_spaces=True)
        body_field = max(edits, key=lambda e: e.element_info.rectangle.height())
        body_field.click_input(); body_field.type_keys("{#} 1. Uczestnicy{ENTER}{#} 2. Ustalenia{ENTER}{#} 3. Zadania", with_spaces=True)
        c.evidence("new_template_filled")
        c.click("Zapisz", timeout=5)
        time.sleep(1.5)
        c.evidence("after_save")
        saved_now = folder / "Notatka ze spotkania.md"
        (out / "saved_template_file.txt").write_text(f"{saved_now}:\n{saved_now.read_text(encoding='utf-8') if saved_now.exists() else '(missing)'}", encoding="utf-8")
        row_button(c, "Notatka ze spotkania", "Właściwości")
        time.sleep(1)
        c.evidence("properties_form")
        c.click("Anuluj", timeout=5)
        row_button(c, "Notatka ze spotkania", "Usuń")
        wiggle(c)
        c.evidence("remove_asks_second_click")
        c.click("Kliknij ponownie, aby usunąć", timeout=5)
        wiggle(c)
        time.sleep(1)
        c.evidence("after_remove")
        listing = "\n".join(sorted(str(p.relative_to(folder)) for p in folder.rglob("*.md")))
        saved = folder / "Notatka ze spotkania.md"
        (out / "templates_folder.txt").write_text(f"{listing}\n\nNotatka ze spotkania.md:\n{saved.read_text(encoding='utf-8') if saved.exists() else '(moved)'}", encoding="utf-8")
    in_window(body)(out, work)


@executor("Szablony: Użyj wstawia prośbę według narzędzia")
def templates_use(out: Path, work: Path):
    with_templates(work, legacy=False)
    def body(c):
        nav(c, "Szablony dokumentów")
        c.click("Użyj", timeout=10)
        time.sleep(1.5)
        c.evidence("chat_with_request")
        (out / "message_field.txt").write_text(c._message_field().get_value() or "", encoding="utf-8")
    in_window(body)(out, work)


@executor("Dokument z szablonu: propozycja mocniejszego modelu i dyscyplina dokumentu")
def document_from_template(out: Path, work: Path):
    with_templates(work, legacy=False)
    def body(c):
        c.start_conversation()
        state = c.say("Przygotuj stronę Confluence według szablonu „Poziom ogólny” opisującą model Sklep (pakiety, najważniejsze elementy). Nie publikuj.", timeout=900)
        if state == "approval":
            c.evidence("stronger_model_offer")
            c.click("Zatwierdź", timeout=5)
            state = c.wait_idle(900)
        while state == "approval":
            c.evidence("other_approval")
            c.click("Zatwierdź", timeout=5)
            state = c.wait_idle(900)
        c.steps()
        wiggle(c)
        c.evidence("document_answer")
        try:
            from executors_window import scroll_to
            scroll_to(c, "Ta odpowiedź")
            c.evidence("document_steps_model")
        except Exception:
            pass
        pages = sorted((c.work / "attachments").glob("*.confluence.md")) if (c.work / "attachments").exists() else []
        (out / "page_source.md").write_text(pages[-1].read_text(encoding="utf-8") if pages else "no page", encoding="utf-8")
        shutil.copy2(TEMPLATES / "Poziom ogólny.md", out / "template_Poziom_ogolny.md")
        c.say("Jaki model napisał poprzednią odpowiedź i jakim modelem odpowiadasz teraz? Jedno zdanie.")
        c.steps()
        c.evidence("next_answer_back_to_settings_model")
    in_window(body)(out, work)


@executor("Klikalne linki w odpowiedzi")
def clickable_links(out: Path, work: Path):
    def body(c):
        c.start_conversation()
        c.say("Odpowiedz dokładnie trzema wierszami: 1) zdanie z linkiem markdown [strona Loonar](https://loonar.eu), 2) zdanie z gołym adresem https://loonar.eu/pl/, 3) adres w kodzie: `https://example.com/kod`.")
        c.evidence("answer_with_links")
        # A link shows the address on hover (a plain text does not): the pointer rests on each of them.
        for label, name in (("strona Loonar", "hover_markdown_link"), ("https://loonar.eu/pl/", "hover_bare_address"), ("https://example.com/kod", "hover_address_in_code")):
            found = [t for t in c.win.descendants() if label in (t.element_info.name or "") and t.element_info.rectangle.left > c.win.rectangle().left + 400]
            if found:
                r = found[-1].element_info.rectangle
                mouse.move(coords=(r.left + min(30, (r.right - r.left) // 2), (r.top + r.bottom) // 2))
                time.sleep(1.5)
                c.evidence(name)
        (out / "note.txt").write_text("egui shows links to UI Automation as text, not as Hyperlink controls: the hover screenshots show the address tooltip and the hand pointer that only links have.", encoding="utf-8")
    in_window(body)(out, work)
    cargo_unit(out, ["links_in_text"], "unit_test_links")


@executor("Załączniki Jira: pobieranie obrazów do dokumentu")
def jira_images(out: Path, work: Path):
    cargo_live(out, "attachments_download_live", "download_live_test", dict(jira_env(), LOONAR_TEST_JIRA_ISSUE="LC-245"))
    links = SCRATCH / "e2e_links_report.md"
    if links.exists():
        text = links.read_text(encoding="utf-8")
        part = text.split("## 2.")[1].split("## 3.")[0] if "## 2." in text else ""
        (out / "jira_issue_gives_its_link.md").write_text("Reading the issue gives its link (answer of the program in a real conversation):\n## 2." + part, encoding="utf-8")
    report = SCRATCH / "e2e_document_report.md"
    if report.exists():
        text = report.read_text(encoding="utf-8")
        turn1 = text.split("## 2.")[0]
        checks = text.split("## Checks")[1].split("## The page")[0] if "## Checks" in text else ""
        page = text.split("## The page (markdown)")[1] if "## The page (markdown)" in text else ""
        images = [l.strip() for l in page.splitlines() if "](image:" in l]
        (out / "end_to_end_document_with_jira_images.md").write_text(
            "End-to-end run of the program (documents_092_live, 2026-10-28): a Confluence page from the template \"Poziom biznesowy\" for the Jira issue LC-245, with its images.\n\n"
            + turn1 + "\n## Checks" + checks + f"\n## Image lines on the page ({len(images)})\n" + "\n".join(images), encoding="utf-8")


@executor("Załączniki w Jirze i ostre obrazy diagramów")
def jira_attach_sharp(out: Path, work: Path):
    cargo_unit(out, ["cloud_and_server_against_mock", "keys_and_addresses"], "unit_tests_jira_create_epic_update_attachment")
    source = (REPO / "src" / "jira.rs").read_text(encoding="utf-8")
    start = source.index("fn cloud_and_server_against_mock")
    (out / "what_the_jira_test_checks.txt").write_text(
        "The source of the unit test cloud_and_server_against_mock (Jira Cloud and Server imitation): it creates issues, a story under an epic "
        "(Cloud: parent, Server: the Epic Link field), changes an issue (description, labels) and adds an attachment, and checks the requests.\n\n"
        + source[start:start + 5000], encoding="utf-8")
    cargo_live(out, "ea_export_size", "ea_export_size_live", {"LOONAR_TEST_PROPS": str(test_props(out))})


@executor("Niewidoczne strzałki: naprawa połączeń na diagramie")
def hidden_connectors(out: Path, work: Path):
    cargo_live(out, "ea_duplicate_links", "ea_duplicate_links_live", {"LOONAR_TEST_PROPS": str(test_props(out))})


@executor("Długie rozmowy: próg z policzonych tokenów")
def counted_tokens(out: Path, work: Path):
    session = max(Path(os.environ["APPDATA"], "Loonar Compass", "history").glob("*.json"), key=lambda p: p.stat().st_size)
    cargo_live(out, "prompt_tokens_counted_live", "counted_tokens_live", {"LOONAR_TEST_PROPS": str(test_props(out)), "LOONAR_TEST_SESSION": str(session)})
