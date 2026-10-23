//! Spelling of the model's texts in the language of Compass: element and diagram notes, scenario steps and the
//! project glossary, checked with the Windows dictionary of that language (EA's own spell checker has only English
//! dictionaries). Names of elements and glossary terms count as known words, so domain terms are not reported.
//! Also: setting EA's own spell checker (Sentry) to the language of Compass when EA has a dictionary for it.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Result};
use std::collections::HashSet;

/// One text of the model: what it belongs to and the text.
struct Text {
    kind: &'static str,
    id: String,
    owner: String,
    text: String,
}

/// Text without HTML-like formatting EA keeps in notes (<b>, <i>, &lt;…).
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"").replace("&nbsp;", " ")
}

/// Values of an attribute in an XML text (scenario steps: name="…").
fn attributes(xml: &str, name: &str) -> Vec<String> {
    let key = format!("{name}=\"");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&key) {
        rest = &rest[i + key.len()..];
        if let Some(j) = rest.find('"') {
            out.push(rest[..j].to_string());
            rest = &rest[j..];
        }
    }
    out
}

fn texts(cfg: &AppConfig, package: Option<i64>) -> Result<Vec<Text>> {
    let filter = match package {
        Some(p) => {
            let ids: Vec<String> = query_rows(cfg, &format!(
                "with recursive tree(package_id) as (select package_id from t_package where package_id = {p} \
                 union all select x.package_id from t_package x join tree t on x.parent_id = t.package_id) select package_id from tree"
            ))?
            .into_iter()
            .map(|r| r[0].clone())
            .collect();
            if ids.is_empty() {
                bail!("{}", crate::i18n::fmt("there is no package with id {0}", &[p.to_string()]));
            }
            format!(" and o.package_id in ({})", ids.join(","))
        }
        None => String::new(),
    };
    let mut out = Vec::new();
    for r in query_rows(cfg, &format!("select o.object_id as c0, coalesce(o.name,'') as c1, coalesce(o.note,'') as c2 from t_object o where coalesce(o.note,'') <> ''{filter}"))? {
        out.push(Text { kind: "element notes", id: r[0].clone(), owner: r[1].clone(), text: plain(&r[2]) });
    }
    let diagram_filter = filter.replace("o.package_id", "d.package_id");
    for r in query_rows(cfg, &format!("select d.diagram_id as c0, coalesce(d.name,'') as c1, coalesce(d.notes,'') as c2 from t_diagram d where coalesce(d.notes,'') <> ''{diagram_filter}"))? {
        out.push(Text { kind: "diagram notes", id: r[0].clone(), owner: r[1].clone(), text: plain(&r[2]) });
    }
    for r in query_rows(cfg, &format!(
        "select o.object_id as c0, coalesce(o.name,'') as c1, coalesce(s.scenario,'') as c2, coalesce(s.notes,'') as c3, coalesce(s.xmlcontent,'') as c4 \
         from t_objectscenarios s join t_object o on o.object_id = s.object_id where 1 = 1{filter}"
    ))? {
        let mut text = format!("{}\n{}", r[2], plain(&r[3]));
        for step in attributes(&r[4], "name") {
            text.push('\n');
            text.push_str(&plain(&step));
        }
        out.push(Text { kind: "scenario", id: r[0].clone(), owner: format!("{} / {}", r[1], r[2]), text });
    }
    if package.is_none() {
        for r in query_rows(cfg, "select coalesce(term,'') as c0, coalesce(meaning,'') as c1 from t_glossary")? {
            out.push(Text { kind: "glossary", id: r[0].clone(), owner: r[0].clone(), text: plain(&r[1]) });
        }
    }
    Ok(out)
}

/// Words that are not reported: names in the model, glossary terms (domain vocabulary).
fn known_words(cfg: &AppConfig) -> HashSet<String> {
    let mut known = HashSet::new();
    for sql in ["select distinct coalesce(name,'') as c0 from t_object", "select distinct coalesce(name,'') as c0 from t_package", "select distinct coalesce(term,'') as c0 from t_glossary"] {
        for r in query_rows(cfg, sql).unwrap_or_default() {
            for w in r[0].split(|c: char| !c.is_alphanumeric()) {
                if !w.is_empty() {
                    known.insert(w.to_lowercase());
                }
            }
        }
    }
    known
}

/// Whether a word is a technical token (identifier, code, abbreviation) and not natural language.
fn technical(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_digit())
        || word.chars().all(|c| !c.is_lowercase())
        || word.chars().skip(1).any(|c| c.is_uppercase())
        || word.contains('_')
        || word.chars().count() < 3
}

pub fn language_name(code: &str) -> &'static str {
    match code {
        "pl" => "polski",
        "de" => "Deutsch",
        "fr" => "français",
        _ => "English",
    }
}

/// Spelling check of the model's texts (or of one package with subpackages) in the language of Compass.
pub fn check(cfg: &AppConfig, package: Option<i64>, language: &str) -> Result<String> {
    // SAFETY: COM for the Windows spell checker in this thread (already initialized is fine).
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
    }
    let Some(checker) = crate::spell::SpellChecker::for_language(language) else {
        bail!("{}", crate::i18n::fmt("Windows has no spelling dictionary for {0}: add the language in Windows Settings → Time & language → Language", &[language_name(language).to_string()]));
    };
    let texts = texts(cfg, package)?;
    let known = known_words(cfg);
    let mut lines = Vec::new();
    let mut words_checked = 0usize;
    let mut reported: HashSet<(String, String)> = HashSet::new();
    for t in &texts {
        for range in checker.errors(&t.text) {
            let word = t.text[range].trim_matches(|c: char| !c.is_alphanumeric()).to_string();
            if word.is_empty() || technical(&word) || known.contains(&word.to_lowercase()) || !reported.insert((t.id.clone(), word.to_lowercase())) {
                continue;
            }
            let suggestions = checker.suggest(&word, 3);
            lines.push(format!("{} | {} | {} | {} | {}", t.kind, t.id, t.owner, word, suggestions.join(", ")));
        }
        words_checked += t.text.split_whitespace().count();
    }
    let mut out = vec![crate::i18n::fmt("Spelling ({0}, dictionary {1}): {2} texts, {3} words checked, {4} possible mistakes.", &[
        language_name(language).to_string(), checker.language.clone(), texts.len().to_string(), words_checked.to_string(), lines.len().to_string(),
    ])];
    if !lines.is_empty() {
        out.push("kind | id | object | word | suggestions".into());
        out.extend(lines.into_iter().take(300));
    }
    Ok(out.join("\n"))
}

/// Spelling of one element's notes for the live check in a tab of EA: "word → suggestion" items.
pub fn check_element(cfg: &AppConfig, id: i64, language: &str) -> Vec<String> {
    let Some(checker) = crate::spell::SpellChecker::for_language(language) else { return Vec::new() };
    let Some(row) = query_rows(cfg, &format!("select coalesce(name,'') as c0, coalesce(note,'') as c1 from t_object where object_id = {id}")).ok().and_then(|r| r.into_iter().next()) else { return Vec::new() };
    let text = plain(&row[1]);
    let known = known_words(cfg);
    let mut out = Vec::new();
    for range in checker.errors(&text) {
        let word = text[range].trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        if word.is_empty() || technical(&word) || known.contains(&word.to_lowercase()) {
            continue;
        }
        let suggestion = checker.suggest(&word, 1).into_iter().next().unwrap_or_default();
        out.push(crate::i18n::fmt("\"{0}\": {1} → {2}", &[row[0].clone(), word, suggestion]));
    }
    out
}

/// EA's own spell checker (Sentry) files for a language: (main dictionary files) when EA has them installed.
fn ea_lexicons(language: &str, region: &str) -> Option<String> {
    let path = std::path::PathBuf::from(r"C:\Program Files\Sparx Systems\EA");
    let code = match (language, region) {
        ("en", "GB") => "br",
        ("en", _) => "am",
        ("de", _) => "ge",
        ("fr", _) => "fr",
        ("es", _) => "sp",
        ("it", _) => "it",
        ("nl", _) => "du",
        _ => return None,
    };
    let (main, extra) = (format!("ssce{code}.tlx"), format!("ssce{code}2.clx"));
    path.join(&main).is_file().then(|| {
        let mut files = vec![main];
        if path.join(&extra).is_file() {
            files.push(extra);
        }
        files.extend(["accent.tlx", "tech.tlx", "sparxdic.tlx"].iter().filter(|f| path.join(f).is_file()).map(|f| f.to_string()));
        files.join(",")
    })
}

/// Sets the language of EA's spell checker (Spelling options of EA, read at EA's start) to the language of Compass.
/// Returns what was done, for the user.
pub fn set_ea_spelling(language: &str) -> String {
    let region = if language == "en" && crate::spell::SpellChecker::for_language("en").is_some_and(|c| c.language == "en-GB") { "GB" } else { "" };
    let Some(files) = ea_lexicons(language, region) else {
        return crate::i18n::fmt("EA has no spelling dictionary for {0} (it comes with English only); Compass checks the texts itself in {0} (spell_check, and in the EA tab when an element changes).", &[language_name(language).to_string()]);
    };
    let status = std::process::Command::new("reg")
        .args(["add", r"HKCU\Software\Sparx Systems\EA64\SSCE", "/v", "MainLexFiles", "/t", "REG_SZ", "/d", &files, "/f"])
        .output();
    match status {
        Ok(o) if o.status.success() => crate::i18n::fmt("EA's spell checker set to {0} ({1}); EA uses it after a restart.", &[language_name(language).to_string(), files]),
        _ => crate::i18n::tr("EA's spelling settings could not be changed").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_helpers() {
        assert_eq!(plain("<b>Zamówienie</b> &amp; faktura").trim(), "Zamówienie  & faktura");
        assert_eq!(attributes("<step name=\"Klient wybiera\" uses=\"\"/><step name=\"System zapisuje\"/>", "name"), vec!["Klient wybiera", "System zapisuje"]);
        assert!(technical("OrderItem") && technical("API") && technical("x2") && !technical("zamówienie"));
        assert!(ea_lexicons("pl", "").is_none(), "EA has no Polish dictionary");
    }

    /// The Windows dictionaries: Polish text with mistakes. Run with: `cargo test model_spell_dictionaries -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires Windows spelling dictionaries"]
    fn model_spell_dictionaries() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        for lang in ["pl", "en", "de", "fr"] {
            let c = crate::spell::SpellChecker::for_language(lang);
            println!("{lang}: {:?}", c.as_ref().map(|c| c.language.clone()));
        }
        let pl = crate::spell::SpellChecker::for_language("pl").expect("Polish dictionary");
        let text = "Klijent składa zamuwienie i otrzymuje fakture.";
        let words: Vec<String> = pl.errors(text).into_iter().map(|r| text[r].to_string()).collect();
        println!("{words:?} suggestions for zamuwienie: {:?}", pl.suggest("zamuwienie", 3));
        assert!(words.contains(&"Klijent".to_string()) && words.contains(&"zamuwienie".to_string()));
        assert!(pl.suggest("zamuwienie", 3).iter().any(|s| s == "zamówienie"));
    }
}
