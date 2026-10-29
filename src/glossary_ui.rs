//! `/glossary-add`: a window for adding a term to the EA project glossary. For a Polish term the assistant proposes
//! its inflected forms (singular and plural, all seven cases; a phrase inflected as a whole), which the user corrects
//! before saving: EA highlights a term in notes only in exactly the form written in the glossary.

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

enum Event {
    Forms(Result<Vec<String>, String>),
    Saved(Result<String, String>),
}

pub struct Dialog {
    cfg: AppConfig,
    term: String,
    kind: String,
    meaning: String,
    polish: bool,
    forms: String,
    busy: Option<&'static str>,
    message: Option<(bool, String)>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}

impl Dialog {
    pub fn new(cfg: AppConfig, term: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let polish = crate::i18n::current() == crate::i18n::Lang::Pl;
        Self { cfg, term, kind: "Business".into(), meaning: String::new(), polish, forms: String::new(), busy: None, message: None, tx, rx }
    }

    fn inflect(&mut self, ctx: &egui::Context) {
        let (cfg, term, tx, wake) = (self.cfg.clone(), self.term.trim().to_string(), self.tx.clone(), ctx.clone());
        self.busy = Some(crate::i18n::tr("The assistant is inflecting the term…"));
        self.message = None;
        std::thread::spawn(move || {
            let _ = tx.send(Event::Forms(polish_forms(&cfg, &term).map_err(|e| format!("{e:#}"))));
            wake.request_repaint();
        });
    }

    fn save(&mut self, ctx: &egui::Context) {
        let forms: Vec<String> = self.forms.lines().map(str::trim).filter(|f| !f.is_empty()).map(str::to_string).collect();
        let op = json!({ "op": "add_glossary_term", "term": self.term.trim(), "type": self.kind.trim(), "meaning": self.meaning.trim(), "forms": forms });
        let (cfg, tx, wake) = (self.cfg.clone(), self.tx.clone(), ctx.clone());
        self.busy = Some(crate::i18n::tr("Saving in the project glossary…"));
        self.message = None;
        std::thread::spawn(move || {
            let result = ea::run_batch(&cfg, &[op], &BatchOptions::default())
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
                Event::Forms(Ok(forms)) => self.forms = forms.join("\n"),
                Event::Forms(Err(e)) | Event::Saved(Err(e)) => self.message = Some((false, e)),
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
            .default_width(520.0)
            .show(ctx, |ui| {
                egui::Grid::new("glossary_add").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Term"));
                    ui.add(egui::TextEdit::singleline(&mut self.term).desired_width(360.0));
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
                    ui.add(egui::TextEdit::multiline(&mut self.meaning).desired_rows(3).desired_width(360.0));
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
                        ui.label(egui::RichText::new(crate::i18n::tr("Forms (one per line): EA highlights only these exact forms")).small());
                        if ui.add_enabled(self.busy.is_none() && !self.term.trim().is_empty(), egui::Button::new(crate::i18n::tr("Inflect by cases"))).clicked() {
                            inflect = true;
                        }
                    });
                    ui.add(egui::TextEdit::multiline(&mut self.forms).desired_rows(8).desired_width(f32::INFINITY));
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
            if !self.polish {
                self.forms.clear();
            }
            self.save(ctx);
        }
        open
    }
}

/// Polish inflected forms of a term (singular and plural in all seven cases, without repetitions; the term itself is
/// left out), from the assistant.
fn polish_forms(cfg: &AppConfig, term: &str) -> anyhow::Result<Vec<String>> {
    let client = crate::claude::ClaudeClient::from_config(cfg, 1000)?.with_purpose("glossary");
    let system = "You inflect Polish nouns and noun phrases. For the given term return every form of it: singular and plural in all seven \
cases (mianownik, dopełniacz, celownik, biernik, narzędnik, miejscownik, wołacz); for a phrase inflect the whole phrase (adjectives agree, \
a genitive complement stays as it is). Keep the capitalisation of the term. Return ONLY the distinct forms, one per line, nothing else.";
    let response = client.send_value(system, &[json!({ "role": "user", "content": term })], None)?;
    let text: String = response["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("");
    let mut forms: Vec<String> = Vec::new();
    for line in text.lines().map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim()) {
        if !line.is_empty() && !line.eq_ignore_ascii_case(term) && !forms.iter().any(|f| f == line) {
            forms.push(line.to_string());
        }
    }
    anyhow::ensure!(!forms.is_empty(), "{}", crate::i18n::tr("The assistant returned no forms"));
    Ok(forms)
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_with_and_without_term() {
        assert_eq!(super::command("/glossary-add"), Some(String::new()));
        assert_eq!(super::command("  /glossary_add  zamówienie hurtowe "), Some("zamówienie hurtowe".into()));
        assert_eq!(super::command("/glossary-addx"), None);
        assert_eq!(super::command("dodaj /glossary-add"), None);
    }
}
