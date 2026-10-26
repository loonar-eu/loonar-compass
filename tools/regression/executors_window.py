"""Window tests of parts A (rest), B, C and E: the real Compass window, driven like a person would."""
import sqlite3
import time
from pathlib import Path

from pywinauto import mouse

from executors import executor, window_test
from ui import Compass


@executor("Usuwanie rozmowy")
@window_test
def delete_conversation(c):
    c.start_conversation()
    c.say("Ile pakietów ma model? Odpowiedz jednym zdaniem.")
    c.evidence("old_conversation_in_list")
    # The conversation in the list on the left (the same text is also in the conversation itself).
    left = c.win.rectangle().left + 450
    item = next(t for t in c.win.descendants(control_type="Text")
                if "Ile pakietów" in (t.element_info.name or "") and t.element_info.rectangle.left < left)
    r = item.element_info.rectangle
    mouse.right_click(coords=((r.left + r.right) // 2, (r.top + r.bottom) // 2))
    time.sleep(1)
    c.evidence("context_menu")
    c.click("Usuń rozmowę", contains=True)
    try:
        c.click("Usuń", timeout=3)  # a confirmation, if the window asks
    except RuntimeError:
        pass
    c.evidence("list_after_delete")
    bin_folder = c.work / "history" / "bin"
    files = sorted(p.name for p in bin_folder.iterdir()) if bin_folder.exists() else []
    (c.out / "folder_history_bin.txt").write_text(f"{bin_folder}:\n" + ("\n".join(files) or "(empty or missing)"), encoding="utf-8")


# B. Documents --------------------------------------------------------------------------------------------------
def sample_xlsx(path: Path):
    import openpyxl
    wb = openpyxl.Workbook()
    ws = wb.active
    ws.title = "Orders"
    ws.append(["Zamówienie", "Klient", "Kwota"])
    for row in (["Z-1", "Anna", 120], ["Z-2", "Bartek", 80], ["Z-3", "Celina", 200]):
        ws.append(row)
    wb.create_sheet("Klienci").append(["Anna", "Bartek", "Celina"])
    wb.save(path)


def open_files(c, pages=4):
    """What a person sees after "open": every slide of a PPTX exported by PowerPoint, the pages of a PDF rendered,
    and the full text of a PDF (the evidence of steps that say "open the file")."""
    for f in c.files():
        try:
            if f.suffix.lower() == ".pptx":
                import win32com.client
                app = win32com.client.Dispatch("PowerPoint.Application")
                deck = app.Presentations.Open(str(f), True, False, False)
                for i in range(1, min(deck.Slides.Count, pages) + 1):
                    deck.Slides(i).Export(str(c.out / f"otwarty_pptx_slajd_{i}.png"), "PNG", 960, 540)
                deck.Close()
            elif f.suffix.lower() == ".pdf":
                import pypdfium2
                pdf = pypdfium2.PdfDocument(str(f))
                for i in range(min(len(pdf), pages)):
                    pdf[i].render(scale=1.2).to_pil().save(c.out / f"otwarty_{f.stem[:30]}_strona_{i + 1}.png")
                text = "\n".join(pdf[i].get_textpage().get_text_range() for i in range(len(pdf)))
                (c.out / f"otwarty_{f.stem[:30]}_tekst.txt").write_text(text, encoding="utf-8")
        except Exception as e:
            (c.out / f"otwarcie_{f.name}_blad.txt").write_text(str(e), encoding="utf-8")


def files_report(c, name="files_in_conversation"):
    """What the created files contain: slides of a PPTX, pages of a PDF, size of other files."""
    lines = []
    for f in c.files():
        line = f"{f.name} ({f.stat().st_size} B)"
        try:
            if f.suffix.lower() == ".pptx":
                from pptx import Presentation
                prs = Presentation(f)
                titles = [next((s.text_frame.text for s in slide.shapes if s.has_text_frame and s.text_frame.text.strip()), "") for slide in prs.slides]
                pictures = sum(1 for slide in prs.slides for s in slide.shapes if s.shape_type == 13)
                line += f": {len(prs.slides)} slides, pictures: {pictures}, titles: {titles}"
            elif f.suffix.lower() == ".pdf":
                from pypdf import PdfReader
                pdf = PdfReader(f)
                line += f": {len(pdf.pages)} pages; beginning: {(pdf.pages[0].extract_text() or '')[:300]!r}"
        except Exception as e:
            line += f": could not be read ({e})"
        lines.append(line)
    (c.out / f"{name}.txt").write_text("\n".join(lines) or "(no files)", encoding="utf-8")


@executor("Odczyt PPTX / VSDX / XLSX")
@window_test
def read_office(c):
    c.start_conversation()
    sample = c.work / "orders_test.xlsx"
    sample_xlsx(sample)
    c.attach(sample)
    c.evidence("file_attached")
    c.say_approving("Streść ten plik.")
    c.evidence("summary")


@executor("Odczyt PDF z dysku")
@window_test
def read_pdf(c):
    c.start_conversation()
    c.say_approving("Przeczytaj C:\\in_progress\\Claude\\Loonar_Compass_0.8.2_plan_testow.pdf i powiedz, ile ma części.", label="read_approval")
    c.evidence("answer")


@executor("Tworzenie prezentacji PowerPoint i PDF")
@window_test
def make_slides(c):
    c.start_conversation()
    c.say_approving("Zrób 4-slajdową prezentację PowerPoint o pakiecie Shop domain z diagramem klas, także jako PDF.")
    c.evidence("files_in_conversation")
    files_report(c)
    open_files(c)


@executor("Tworzenie dokumentu PDF przez Worda")
@window_test
def make_pdf(c):
    c.start_conversation()
    c.say_approving("Przygotuj dokument PDF z listą klas pakietu Shop domain.")
    c.evidence("pdf_in_conversation")
    files_report(c)
    open_files(c)
    model_classes(c)


def model_classes(c):
    """The classes of Shop domain (with sub-packages) in the model copy, to compare a generated list with."""
    model = next(c.work.glob("*.qea"))
    con = sqlite3.connect(model)
    rows = con.execute("""with recursive pk(id) as (select package_id from t_package where name = 'Shop domain'
                          union all select p.package_id from t_package p join pk on p.parent_id = pk.id)
                          select name from t_object where object_type = 'Class' and package_id in (select id from pk) order by name""").fetchall()
    con.close()
    (c.out / "model_classes_shop_domain.txt").write_text("Klasy pakietu Shop domain w modelu:\n" + "\n".join(r[0] for r in rows), encoding="utf-8")


# C. Model: instructions, MDG, EA API, sub-agent ------------------------------------------------------------------
def model_objects(c, where, name):
    """Rows of t_object in the conversation's model copy (the .qea file is SQLite), saved as evidence."""
    model = next(c.work.glob("*.qea"))
    con = sqlite3.connect(model)
    rows = con.execute(f"select name, object_type, stereotype, note from t_object where {where}").fetchall()
    con.close()
    (c.out / f"{name}.txt").write_text(f"{model.name}: {where}\n" + ("\n".join(map(str, rows)) or "(none)"), encoding="utf-8")


INSTRUCTIONS = "stereotype = 'CompassInstructions' or name like '%Compass%Instruct%'"


@executor("Instrukcje modelowania w modelu")
@window_test
def instructions(c):
    c.start_conversation()
    c.say_approving("Przygotuj instrukcje modelowania dla tego repozytorium.")
    c.evidence("proposal")
    # A person accepts the proposal; the assistant then asks to write it (approved).
    c.say_approving("Tak, projekt jest dobry. Zapisz instrukcje w modelu.")
    c.evidence("write")
    model_objects(c, INSTRUCTIONS, "model_CompassInstructions")


@executor("Dopisanie reguły i użycie w nowej rozmowie")
@window_test
def instructions_rule(c):
    c.start_conversation()
    c.say_approving("Przygotuj instrukcje modelowania dla tego repozytorium.")  # the precondition: instructions exist
    c.say_approving("Tak, projekt jest dobry. Zapisz instrukcje w modelu.")
    c.say_approving("Dopisz do instrukcji: aktywności zaczynają się od czasownika.")
    c.evidence("rule_added")
    model_objects(c, INSTRUCTIONS, "model_CompassInstructions")
    c.click("Nowa rozmowa")  # new conversation
    c.start_conversation()
    c.say("Jakie mamy zasady nazewnictwa?")
    c.evidence("new_conversation_knows_rules")


@executor("Technologie MDG (ArchiMate)")
@window_test
def mdg(c):
    c.start_conversation()
    c.say("Jakie stereotypy ma ArchiMate dla komponentu aplikacji i jakie wartości tagowane?")
    c.steps()
    c.evidence("answer_and_steps")


@executor("Skrypt EA z weryfikacją API EA")
@window_test
def ea_script(c):
    c.start_conversation()
    state = c.say("Napisz skrypt EA, który wypisze wszystkie klasy bez opisu; sprawdź najpierw API EA.")
    c.steps()
    c.evidence("skrypt_i_kroki")
    if state == "approval":
        c.approve(False)


@executor("Podagent wpływu zmiany")
@window_test
def subagent(c):
    c.start_conversation()
    state = c.say("Użyj podagenta wpływu: co się stanie po zmianie nazwy Order na PurchaseOrder?")
    c.steps()
    c.evidence("answer_and_steps")
    if state == "approval":
        c.approve(False)
    model_objects(c, "name in ('Order', 'PurchaseOrder')", "model_unchanged")


@executor("Status kontroli wersji pakietów")
@window_test
def version_control(c):
    c.start_conversation()
    c.say("Które pakiety są pod kontrolą wersji?")
    c.steps()
    c.evidence("answer_and_steps")


# E. SQL Server in the real window ----------------------------------------------------------------------------------
@executor("Pytania o model w SQL Server")
def sqlserver_questions(out: Path, work: Path):
    c = Compass(work, out, model="sqlserver")
    try:
        c.start_conversation()
        c.say("Ile jest klas i jakie są relacje klasy Order?")
        c.evidence("classes_and_relations")
        c.say("Sprawdź jakość pakietu Shop domain.")
        c.steps()
        c.evidence("quality_check")
    except Exception:
        c.evidence("state_at_error")
        raise
    finally:
        c.close()
    # The same question on the file model, for the comparison the expected result asks for.
    file_work = work / "plik"
    file_work.mkdir()
    c = Compass(file_work, out)
    try:
        c.start_conversation()
        c.say("Ile jest klas i jakie są relacje klasy Order?")
        c.evidence("for_comparison_qea_file")
    finally:
        c.close()


@executor("Model testowy działa jak w 0.8.2")
@window_test
def test_model(c):
    c.evidence("compass_with_model_copy")
    c.start_conversation()
    c.say("Jakie pakiety są w modelu?")
    c.evidence("model_packages")
    model = next(c.work.glob("*.qea"))
    con = sqlite3.connect(model)
    rows = con.execute("select name from t_package order by name").fetchall()
    con.close()
    (c.out / "model_packages.txt").write_text("Pakiety w kopii modelu:\n" + "\n".join(r[0] for r in rows), encoding="utf-8")
