//! UI languages: English, Polish, German, French.
//!
//! Texts in the code are English and serve as keys; translations live in `i18n/strings.src`
//! (one line per text: `en ‖ pl ‖ de ‖ fr`; `\n` in a cell is a line break). A missing translation
//! falls back to English. Texts with values use the placeholders `{0}`, `{1}`… (see `fmt`).
//! A key may end with `@@context` to tell apart identical English texts that translate differently
//! (e.g. a label and the same words in the middle of a sentence); the suffix is never shown.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pl,
    En,
    De,
    Fr,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Pl, Lang::De, Lang::Fr];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
            Lang::De => "de",
            Lang::Fr => "fr",
        }
    }

    /// The language's name in that language (for the language picker).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Pl => "Polski",
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
        }
    }

    /// The language's English name (for instructions to the model).
    pub fn english_name(self) -> &'static str {
        match self {
            Lang::Pl => "Polish",
            Lang::En => "English",
            Lang::De => "German",
            Lang::Fr => "French",
        }
    }

    /// Language code from the settings; unknown or empty means English (the default install language).
    pub fn parse(code: &str) -> Lang {
        match code.trim().to_lowercase().as_str() {
            "pl" => Lang::Pl,
            "de" => Lang::De,
            "fr" => Lang::Fr,
            _ => Lang::En,
        }
    }

    /// Index into a translation row `[pl, de, fr]`; English is the key itself.
    fn column(self) -> Option<usize> {
        match self {
            Lang::En => None,
            Lang::Pl => Some(0),
            Lang::De => Some(1),
            Lang::Fr => Some(2),
        }
    }

    fn id(self) -> u8 {
        match self {
            Lang::En => 0,
            Lang::Pl => 1,
            Lang::De => 2,
            Lang::Fr => 3,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang.id(), Ordering::Relaxed);
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Pl,
        2 => Lang::De,
        3 => Lang::Fr,
        _ => Lang::En,
    }
}

fn unescape(cell: &str) -> String {
    cell.replace("\\n", "\n").replace("\\t", "\t")
}

/// Table: English key -> [pl, de, fr] (the texts live until the program ends).
fn table() -> &'static HashMap<String, [&'static str; 3]> {
    static TABLE: OnceLock<HashMap<String, [&'static str; 3]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for line in include_str!("../i18n/strings.src").lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let cells: Vec<&str> = line.split(" ‖ ").collect();
            if cells.len() < 4 {
                continue;
            }
            let leak = |s: &str| -> &'static str { Box::leak(unescape(s).into_boxed_str()) };
            map.insert(unescape(cells[0]), [leak(cells[1]), leak(cells[2]), leak(cells[3])]);
        }
        map
    })
}

/// English text of a key: the key without its `@@context` suffix.
fn english(key: &'static str) -> &'static str {
    key.split_once("@@").map_or(key, |(text, _)| text)
}

/// Text in the current language.
pub fn tr(key: &'static str) -> &'static str {
    tr_in(current(), key)
}

pub fn tr_in(lang: Lang, key: &'static str) -> &'static str {
    let Some(column) = lang.column() else { return english(key) };
    match table().get(key) {
        Some(row) if !row[column].is_empty() => row[column],
        _ => english(key),
    }
}

/// Text with values: the key contains placeholders {0}, {1}…
pub fn fmt(key: &'static str, values: &[String]) -> String {
    let mut out = tr(key).to_string();
    for (i, value) in values.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), value);
    }
    out.replace("{{", "{").replace("}}", "}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_used_in_code_has_all_translations() {
        // Keys in the code: crate::i18n::tr("…"), crate::i18n::fmt("…", …) and i18n::tr_in(lang, "…"),
        // in every file with texts for the user (window, messages, errors shown in the app).
        let sources = [
            include_str!("main.rs"),
            include_str!("relang.rs"),
            include_str!("../installer/src/main.rs"),
            include_str!("check.rs"),
            include_str!("mdg.rs"),
            include_str!("instructions.rs"),
            include_str!("oidc.rs"),
            include_str!("documents.rs"),
            include_str!("graphql.rs"),
            include_str!("auth.rs"),
            include_str!("office.rs"),
            include_str!("pptx.rs"),
            include_str!("ea_api.rs"),
            include_str!("claude.rs"),
            include_str!("model_db.rs"),
            include_str!("ops.rs"),
            include_str!("agent_ro.rs"),
            include_str!("cmdb.rs"),
            include_str!("com.rs"),
            include_str!("config.rs"),
            include_str!("confluence.rs"),
            include_str!("diagram_editor.rs"),
            include_str!("docx.rs"),
            include_str!("ea.rs"),
            include_str!("ea_com.rs"),
            include_str!("ea_ui.rs"),
            include_str!("governance.rs"),
            include_str!("history.rs"),
            include_str!("hub.rs"),
            include_str!("jira.rs"),
            include_str!("openai.rs"),
            include_str!("memory.rs"),
            include_str!("miro.rs"),
            include_str!("render.rs"),
            include_str!("report.rs"),
            include_str!("templates.rs"),
            include_str!("tools.rs"),
            include_str!("estimate.rs"),
            include_str!("undo.rs"),
            include_str!("code_drift.rs"),
            include_str!("traceability.rs"),
            include_str!("model_spell.rs"),
            include_str!("agents.rs"),
            include_str!("scripts.rs"),
            include_str!("mcp_client.rs"),
            include_str!("verify.rs"),
            include_str!("window.rs"),
            include_str!("xlsx.rs"),
        ];
        let mut missing = Vec::new();
        let mut checked = 0;
        for source in sources {
            for pattern in ["i18n::tr(", "i18n::fmt(", "i18n::tr_in("] {
                let mut rest = source;
                while let Some(pos) = rest.find(pattern) {
                    rest = &rest[pos + pattern.len()..];
                    let body = if pattern != "i18n::tr_in(" {
                        // The text may start on the next line (long calls are wrapped by rustfmt).
                        match rest.trim_start().strip_prefix('"') {
                            Some(body) => body,
                            None => continue,
                        }
                    } else {
                        // tr_in(lang, "…"): skip the language argument.
                        match rest.find('"') {
                            Some(quote) if !rest[..quote].contains(')') => &rest[quote + 1..],
                            _ => continue,
                        }
                    };
                    let bytes = body.as_bytes();
                    let mut end = 0;
                    while end < bytes.len() && !(bytes[end] == b'"' && (end == 0 || bytes[end - 1] != b'\\')) {
                        end += 1;
                    }
                    let key = body[..end].replace("\\\"", "\"").replace("\\n", "\n");
                    checked += 1;
                    match table().get(&key) {
                        Some(row) if row.iter().all(|t| !t.is_empty()) => {}
                        _ => missing.push(key),
                    }
                }
            }
        }
        assert!(checked > 700, "too few keys found in the code: {checked}");
        missing.sort();
        missing.dedup();
        // One key per line, with line breaks inside a key shown as \n (as in strings.src).
        let listed: Vec<String> = missing.iter().map(|k| k.replace('\n', "\\n")).collect();
        assert!(missing.is_empty(), "missing translations ({}):\n{}", missing.len(), listed.join("\n"));
    }

    #[test]
    fn fmt_fills_values_and_falls_back_to_english() {
        set(Lang::Pl);
        assert_eq!(fmt("Saved: {0}", &["x".into()]), "Zapisano: x");
        set(Lang::En);
        assert_eq!(tr("text without a translation xyz"), "text without a translation xyz");
        assert_eq!(tr("Claude API key@@in-sentence"), "Claude API key");
        set(Lang::Pl);
        assert_eq!(tr("Claude API key@@in-sentence"), "klucz API Claude");
        set(Lang::En);
    }
}
