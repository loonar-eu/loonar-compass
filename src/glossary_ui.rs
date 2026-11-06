//! `/glossary-add`: a window for adding a term to the EA project glossary. For a Polish term the assistant fills its
//! forms – a noun: seven cases in the singular and the plural (14), a verb: the twelve forms descriptions use (infinitive,
//! third persons, past tense in every gender, participle, verbal noun) – and Compass counts how often each form occurs
//! in the model. The user corrects the forms (and for a verb ticks the ones to keep) before saving. EA highlights a
//! term in notes only in exactly the form written in the glossary, so every distinct form becomes an entry once.

use crate::config::AppConfig;
use crate::ea::{self, BatchOptions};
use serde_json::{json, Value};
use std::sync::mpsc;

/// The chat input that opens the window: `/glossary-add` (or `/glossary_add`), optionally followed by the term.
pub fn command(input: &str) -> Option<String> {
    let t = input.trim();
    let rest = t.strip_prefix("/glossary-add").or_else(|| t.strip_prefix("/glossary_add"))?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim().to_string())
}

/// Polish grammar labels (Polish only: the forms exist only for Polish terms).
const CASES: [&str; 7] = ["mianownik (kto? co?)", "dopełniacz (kogo? czego?)", "celownik (komu? czemu?)", "biernik (kogo? co?)", "narzędnik (z kim? z czym?)", "miejscownik (o kim? o czym?)", "wołacz (o!)"];
const NUMBERS: [&str; 2] = ["liczba pojedyncza", "liczba mnoga"];
const VERB_FORMS: [&str; 12] = [
    "bezokolicznik (zatwierdzać)",
    "teraźniejszy, on/ona (zatwierdza)",
    "teraźniejszy, oni/one (zatwierdzają)",
    "przeszły, on (zatwierdził)",
    "przeszły, ona (zatwierdziła)",
    "przeszły, ono (zatwierdziło)",
    "przeszły, oni (zatwierdzili)",
    "przeszły, one (zatwierdziły)",
    "przyszły, on/ona (zatwierdzi)",
    "przyszły, oni/one (zatwierdzą)",
    "imiesłów (zatwierdzony)",
    "rzeczownik odczasownikowy (zatwierdzenie)",
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Noun,
    Verb,
}

impl Kind {
    /// (rows, columns): cases × numbers, or the verb forms in one column.
    fn shape(self) -> (usize, usize) {
        match self {
            Kind::Noun => (CASES.len(), NUMBERS.len()),
            Kind::Verb => (VERB_FORMS.len(), 1),
        }
    }
}

/// The forms of the term: columns of rows, how often each occurs in the model, whether it is saved.
#[derive(Clone)]
struct Forms {
    cells: Vec<Vec<String>>,
    hits: Vec<Vec<Option<usize>>>,
    keep: Vec<Vec<bool>>,
}

impl Forms {
    fn empty(part: Kind) -> Self {
        let (rows, cols) = part.shape();
        Self { cells: vec![vec![String::new(); rows]; cols], hits: vec![vec![None; rows]; cols], keep: vec![vec![true; rows]; cols] }
    }
}

enum Event {
    Forms(Kind, Result<Forms, String>),
    Saved(Result<String, String>),
}

pub struct Dialog {
    cfg: AppConfig,
    term: String,
    kind: String,
    meaning: String,
    polish: bool,
    part: Kind,
    forms: Forms,
    busy: Option<&'static str>,
    message: Option<(bool, String)>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}

impl Dialog {
    pub fn new(cfg: AppConfig, term: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let polish = crate::i18n::current() == crate::i18n::Lang::Pl;
        Self { cfg, term, kind: "Business".into(), meaning: String::new(), polish, part: Kind::Noun, forms: Forms::empty(Kind::Noun), busy: None, message: None, tx, rx }
    }

    fn inflect(&mut self, ctx: &egui::Context) {
        let (cfg, term, part, tx, wake) = (self.cfg.clone(), self.term.trim().to_string(), self.part, self.tx.clone(), ctx.clone());
        self.busy = Some(crate::i18n::tr("The assistant is inflecting the term…"));
        self.message = None;
        std::thread::spawn(move || {
            let result = polish_forms(&cfg, &term, part).map(|cells| with_hits(&cfg, part, cells)).map_err(|e| format!("{e:#}"));
            let _ = tx.send(Event::Forms(part, result));
            wake.request_repaint();
        });
    }

    fn save(&mut self, ctx: &egui::Context) {
        let forms = if self.polish { distinct(&self.term, &self.forms) } else { Vec::new() };
        let op = json!({ "op": "add_glossary_term", "term": self.term.trim(), "type": self.kind.trim(), "meaning": self.meaning.trim(), "forms": forms });
        let (cfg, tx, wake) = (self.cfg.clone(), self.tx.clone(), ctx.clone());
        self.busy = Some(crate::i18n::tr("Saving in the project glossary…"));
        self.message = None;
        std::thread::spawn(move || {
            let ops = [op];
            let written = ea::run_batch(&cfg, &ops, &BatchOptions::default());
            crate::tools::audit(&cfg, &ops, &written);
            let result = written
                .map(|out| out["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Event::Saved(result));
            wake.request_repaint();
        });
    }

    /// Draws the window; false when it was closed.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        while let Ok(event) = self.rx.try_recv() {
            self.busy = None;
            match event {
                Event::Forms(part, Ok(forms)) if part == self.part => self.forms = forms,
                Event::Forms(_, Ok(_)) => {}
                Event::Forms(_, Err(e)) | Event::Saved(Err(e)) => self.message = Some((false, e)),
                Event::Saved(Ok(text)) => self.message = Some((true, if text.is_empty() { crate::i18n::tr("Saved.").to_string() } else { text })),
            }
        }
        let mut open = true;
        let (mut inflect, mut save) = (false, false);
        egui::Window::new(crate::i18n::tr("Add a term to the project glossary"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(660.0)
            .show(ctx, |ui| {
                egui::Grid::new("glossary_add").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Term"));
                    ui.add(egui::TextEdit::singleline(&mut self.term).desired_width(440.0));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Type"));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.kind).desired_width(160.0));
                        for k in ["Business", "Technical"] {
                            if ui.small_button(k).clicked() {
                                self.kind = k.to_string();
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(crate::i18n::tr("Meaning"));
                    ui.add(egui::TextEdit::multiline(&mut self.meaning).desired_rows(3).desired_width(440.0));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Language"));
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.polish, true, "Polski");
                        ui.radio_value(&mut self.polish, false, "English");
                    });
                    ui.end_row();
                });
                if self.polish {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let before = self.part;
                        ui.radio_value(&mut self.part, Kind::Noun, "rzeczownik (7 przypadków × 2 liczby)");
                        ui.radio_value(&mut self.part, Kind::Verb, "czasownik (formy używane w opisach)");
                        if self.part != before {
                            self.forms = Forms::empty(self.part);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.busy.is_none() && !self.term.trim().is_empty(), egui::Button::new(crate::i18n::tr("Inflect by cases"))).clicked() {
                            inflect = true;
                        }
                        ui.label(egui::RichText::new(crate::i18n::tr("EA highlights only these exact forms; each distinct form is saved once.")).small());
                    });
                    self.forms_grid(ui);
                }
                ui.add_space(8.0);
                if let Some(busy) = self.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(busy);
                    });
                }
                if let Some((ok, text)) = &self.message {
                    ui.colored_label(if *ok { crate::theme::SUCCESS } else { crate::theme::ERROR }, text);
                }
                ui.horizontal(|ui| {
                    let ready = self.busy.is_none() && !self.term.trim().is_empty() && !self.meaning.trim().is_empty();
                    if ui.add_enabled(ready, egui::Button::new(crate::i18n::tr("Save in the EA glossary"))).clicked() {
                        save = true;
                    }
                });
            });
        if inflect {
            self.inflect(ctx);
        }
        if save {
            self.save(ctx);
        }
        open
    }

    /// The forms: a noun as cases × numbers, a verb as a list with a tick for each form; next to every form how often
    /// it occurs in the model (after "Inflect").
    fn forms_grid(&mut self, ui: &mut egui::Ui) {
        let (rows, cols) = self.part.shape();
        let hit = |n: Option<usize>| match n {
            Some(0) => egui::RichText::new("–").small().color(crate::theme::MUTED),
            Some(n) => egui::RichText::new(format!("w modelu: {n}")).small().color(crate::theme::SUCCESS),
            None => egui::RichText::new("").small(),
        };
        egui::Grid::new("glossary_forms").num_columns(1 + cols * 2).spacing([8.0, 4.0]).striped(true).show(ui, |ui| {
            if self.part == Kind::Noun {
                ui.label("");
                for c in NUMBERS {
                    ui.label(egui::RichText::new(c).strong());
                    ui.label("");
                }
                ui.end_row();
            }
            for r in 0..rows {
                match self.part {
                    Kind::Noun => {
                        ui.label(CASES[r]);
                    }
                    Kind::Verb => {
                        ui.checkbox(&mut self.forms.keep[0][r], VERB_FORMS[r]);
                    }
                }
                for c in 0..cols {
                    ui.add(egui::TextEdit::singleline(&mut self.forms.cells[c][r]).desired_width(if cols == 2 { 190.0 } else { 260.0 }));
                    ui.label(hit(self.forms.hits[c][r]));
                }
                ui.end_row();
            }
        });
    }
}

/// The forms to save: every distinct kept form once (a form shared by two cases is one entry), without the term
/// itself, in the order of the table.
fn distinct(term: &str, forms: &Forms) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (c, column) in forms.cells.iter().enumerate() {
        for (r, form) in column.iter().enumerate() {
            let form = form.trim();
            if forms.keep[c][r] && !form.is_empty() && !form.eq_ignore_ascii_case(term.trim()) && !out.iter().any(|f| f.eq_ignore_ascii_case(form)) {
                out.push(form.to_string());
            }
        }
    }
    out
}

/// The forms of a Polish term from the assistant: a noun 2 columns (singular, plural) × 7 cases, a verb the 12 forms.
fn polish_forms(cfg: &AppConfig, term: &str, part: Kind) -> anyhow::Result<Vec<Vec<String>>> {
    let client = crate::claude::ClaudeClient::from_config(cfg, 1200)?.with_purpose("glossary");
    let system = match part {
        Kind::Noun => "You decline Polish nouns and noun phrases. For the given term give its forms in the seven cases in this order: \
mianownik, dopełniacz, celownik, biernik, narzędnik, miejscownik, wołacz – in the singular and in the plural. For a phrase decline the \
whole phrase (adjectives agree, a genitive complement stays as it is); keep the capitalisation of the term. Answer with JSON only, nothing \
else: {\"singular\": [7 forms], \"plural\": [7 forms]} – exactly 7 forms in each list, the bare form without case names or comments; \
when a noun has no plural or singular, repeat the forms it has.",
        Kind::Verb => "You give the forms of Polish verbs used in model descriptions. For the given verb answer with JSON only, nothing \
else: {\"forms\": [12 strings]} in exactly this order: infinitive; present tense 3rd person singular; present 3rd person plural; past tense \
3rd person singular masculine, feminine, neuter; past 3rd person plural masculine-personal, non-masculine-personal; future 3rd person \
singular; future 3rd person plural; the passive adjectival participle (masculine singular); the verbal noun. For an imperfective verb the \
future is the compound form (będzie zatwierdzać, będą zatwierdzać); a perfective verb has no present tense: give empty strings there. \
Bare forms only, no comments.",
    };
    let response = client.send_value(system, &[json!({ "role": "user", "content": term })], None)?;
    let text: String = response["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("");
    cells_from(&text, part).ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("The assistant returned no forms")))
}

/// The forms from the assistant's JSON answer, each cleaned (a note in brackets or before a colon removed); None when
/// the answer has no list of the right length.
fn cells_from(text: &str, part: Kind) -> Option<Vec<Vec<String>>> {
    let json: Value = serde_json::from_str(text.get(text.find('{')?..=text.rfind('}')?)?).ok()?;
    let keys: &[&str] = match part {
        Kind::Noun => &["singular", "plural"],
        Kind::Verb => &["forms"],
    };
    let (rows, _) = part.shape();
    let clean = |v: &Value| {
        let s = v.as_str().unwrap_or("");
        let s = s.split('(').next().unwrap_or("");
        let s = s.rsplit(':').next().unwrap_or("");
        s.trim().trim_matches(['"', ',', '.']).trim().to_string()
    };
    let mut cells = Vec::new();
    for key in keys {
        let list: Vec<String> = json[*key].as_array()?.iter().map(clean).collect();
        if list.len() != rows || list.iter().all(String::is_empty) {
            return None;
        }
        cells.push(list);
    }
    Some(cells)
}

/// How often a form occurs as a whole word in a text (case does not matter).
fn count_word(text: &str, form: &str) -> usize {
    let (text, form) = (text.to_lowercase(), form.to_lowercase());
    let mut n = 0;
    let mut from = 0;
    while let Some(i) = text[from..].find(&form) {
        let start = from + i;
        let end = start + form.len();
        let before = text[..start].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = text[end..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            n += 1;
        }
        from = end;
    }
    n
}

/// The forms with how often each occurs in the model (names and notes of elements and diagrams, notes of attributes and
/// operations). For a verb, the forms found are ticked (with the infinitive); when none is found, all stay ticked.
fn with_hits(cfg: &AppConfig, part: Kind, cells: Vec<Vec<String>>) -> Forms {
    let mut forms = Forms::empty(part);
    for (c, column) in cells.iter().enumerate() {
        for (r, form) in column.iter().enumerate() {
            forms.cells[c][r] = form.clone();
            if form.trim().is_empty() {
                continue;
            }
            let like = crate::model_db::sql_text(cfg.model_source, form.trim());
            let sql = format!(
                "select coalesce(name,'') || ' ' || coalesce(note,'') from t_object where name like '%{like}%' or note like '%{like}%' \
                 union all select coalesce(name,'') || ' ' || coalesce(notes,'') from t_diagram where name like '%{like}%' or notes like '%{like}%' \
                 union all select coalesce(notes,'') from t_attribute where notes like '%{like}%' \
                 union all select coalesce(notes,'') from t_operation where notes like '%{like}%'"
            );
            forms.hits[c][r] = crate::tools::query_rows(cfg, &sql).ok().map(|rows| rows.iter().map(|row| count_word(&row[0], form.trim())).sum());
        }
    }
    if part == Kind::Verb && forms.hits[0].iter().any(|h| h.unwrap_or(0) > 0) {
        for r in 0..VERB_FORMS.len() {
            forms.keep[0][r] = r == 0 || forms.hits[0][r].unwrap_or(0) > 0;
        }
    }
    forms
}

#[cfg(test)]
mod tests {
    /// LC-340: "}" before the first "{" is no answer, not a panic.
    #[test]
    fn cells_from_brace_order() {
        assert!(super::cells_from("} żaden {", super::Kind::Noun).is_none());
    }

    use super::{Forms, Kind};

    #[test]
    fn command_with_and_without_term() {
        assert_eq!(super::command("/glossary-add"), Some(String::new()));
        assert_eq!(super::command("  /glossary_add  zamówienie hurtowe "), Some("zamówienie hurtowe".into()));
        assert_eq!(super::command("/glossary-addx"), None);
        assert_eq!(super::command("dodaj /glossary-add"), None);
    }

    #[test]
    fn noun_of_14_forms_saved_once_each() {
        let answer = "Here you go:\n{\"singular\": [\"książka\", \"książki\", \"książce\", \"książkę\", \"książką\", \"książce (miejscownik)\", \"książko\"], \
\"plural\": [\"książki\", \"książek\", \"książkom\", \"książki\", \"książkami\", \"książkach\", \"książki\"]}";
        let cells = super::cells_from(answer, Kind::Noun).expect("forms");
        assert_eq!(cells.iter().map(Vec::len).sum::<usize>(), 14);
        assert_eq!(cells[0][5], "książce", "a note in brackets is removed");
        let mut forms = Forms::empty(Kind::Noun);
        forms.cells = cells;
        assert_eq!(super::distinct("książka", &forms), vec!["książki", "książce", "książkę", "książką", "książko", "książek", "książkom", "książkami", "książkach"]);
        // Lines instead of JSON, or a list of the wrong length, are no forms.
        assert!(super::cells_from("książki\nksiążce", Kind::Noun).is_none());
        assert!(super::cells_from("{\"singular\": [\"a\"], \"plural\": [\"b\"]}", Kind::Noun).is_none());
    }

    #[test]
    fn verb_of_12_forms_and_only_ticked_ones_saved() {
        let answer = "{\"forms\": [\"zatwierdzać\", \"zatwierdza\", \"zatwierdzają\", \"zatwierdzał\", \"zatwierdzała\", \"zatwierdzało\", \"zatwierdzali\", \"zatwierdzały\", \"będzie zatwierdzać\", \"będą zatwierdzać\", \"zatwierdzany\", \"zatwierdzanie\"]}";
        let cells = super::cells_from(answer, Kind::Verb).expect("forms");
        assert_eq!(cells[0].len(), 12);
        let mut forms = Forms::empty(Kind::Verb);
        forms.cells = cells;
        assert_eq!(super::distinct("zatwierdzać", &forms).len(), 11, "all ticked, without the term itself");
        forms.keep[0] = vec![false; 12];
        forms.keep[0][1] = true;
        assert_eq!(super::distinct("zatwierdzać", &forms), vec!["zatwierdza"]);
    }

    #[test]
    fn whole_words_are_counted() {
        assert_eq!(super::count_word("Książki klienta. KSIĄŻKI i książkiem", "książki"), 2);
        assert_eq!(super::count_word("zamówienia hurtowego, zamówienia hurtowegoX", "zamówienia hurtowego"), 1);
    }
}
