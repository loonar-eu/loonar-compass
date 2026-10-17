//! Document templates: Markdown files in the templates folder next to the conversation history. The assistant fills them
//! with content from the model and the conversation (Word document or Confluence page). The first line `> Szablon: description`
//! (in other languages `> Template:`, `> Vorlage:`, `> Modèle:`) is the description for the list; `> …` lines in the body are hints for the assistant.
//! The default templates exist in four languages; the user can change them and add their own.

use crate::i18n::Lang;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Default templates in the given language: (file name without .md, content). Same order in all languages.
pub fn defaults(lang: Lang) -> [(&'static str, &'static str); 4] {
    match lang {
        Lang::Pl => [
            ("Wstępna analiza wykonalności", include_str!("../templates/pl/Feasibility_analysis.md")),
            ("BRD - Wymagania biznesowe", include_str!("../templates/pl/BRD.md")),
            ("Opis zmian w systemie", include_str!("../templates/pl/System_change.md")),
            ("Opis procesu", include_str!("../templates/pl/Process_description.md")),
        ],
        Lang::En => [
            ("Preliminary feasibility analysis", include_str!("../templates/en/Feasibility_analysis.md")),
            ("BRD - Business requirements", include_str!("../templates/en/BRD.md")),
            ("System change description", include_str!("../templates/en/System_change.md")),
            ("Process description", include_str!("../templates/en/Process_description.md")),
        ],
        Lang::De => [
            ("Vorläufige Machbarkeitsanalyse", include_str!("../templates/de/Feasibility_analysis.md")),
            ("BRD - Geschäftsanforderungen", include_str!("../templates/de/BRD.md")),
            ("Beschreibung der Systemänderungen", include_str!("../templates/de/System_change.md")),
            ("Prozessbeschreibung", include_str!("../templates/de/Process_description.md")),
        ],
        Lang::Fr => [
            ("Analyse de faisabilité préliminaire", include_str!("../templates/fr/Feasibility_analysis.md")),
            ("BRD - Exigences métier", include_str!("../templates/fr/BRD.md")),
            ("Description des modifications du système", include_str!("../templates/fr/System_change.md")),
            ("Description du processus", include_str!("../templates/fr/Process_description.md")),
        ],
    }
}

/// Creates the folder with the default templates in the given language if it does not exist (never overwrites existing files).
pub fn ensure_defaults(dir: &Path, lang: Lang) {
    if dir.is_dir() {
        return;
    }
    if std::fs::create_dir_all(dir).is_ok() {
        for (name, text) in defaults(lang) {
            let _ = std::fs::write(dir.join(format!("{name}.md")), text);
        }
    }
}

/// Description from the first line `> Word: description`.
fn description(text: &str) -> String {
    text.trim_start_matches('\u{feff}')
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix('>'))
        .and_then(|l| l.split_once(':'))
        .map(|(_, d)| d.trim().to_string())
        .unwrap_or_default()
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    out.sort();
    out
}

/// Templates: (name, description), alphabetically.
pub fn list(dir: &Path) -> Vec<(String, String)> {
    ensure_defaults(dir, crate::i18n::current());
    files(dir)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).ok()?;
            Some((name, description(&text)))
        })
        .collect()
}

fn file_of(dir: &Path, name: &str) -> Result<PathBuf> {
    let clean = name.trim().trim_end_matches(".md").replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "");
    if clean.is_empty() {
        bail!("{}", crate::i18n::tr("empty template name"));
    }
    Ok(dir.join(format!("{clean}.md")))
}

pub fn read(dir: &Path, name: &str) -> Result<String> {
    ensure_defaults(dir, crate::i18n::current());
    let path = file_of(dir, name)?;
    std::fs::read_to_string(&path).with_context(|| crate::i18n::fmt("there is no template \"{0}\" (available: {1})", &[name.to_string(), list(dir).into_iter().map(|t| t.0).collect::<Vec<_>>().join(", ")]))
}

pub fn save(dir: &Path, name: &str, content: &str) -> Result<String> {
    std::fs::create_dir_all(dir).context(crate::i18n::tr("templates folder"))?;
    let path = file_of(dir, name)?;
    std::fs::write(&path, content).with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))?;
    Ok(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// Language change: untouched default templates in language `from` are replaced with the defaults in language `to` (no AI).
/// Returns the files for the AI to translate: the user's own templates and modified default templates.
pub fn switch_defaults(dir: &Path, from: Lang, to: Lang) -> Vec<PathBuf> {
    if !dir.is_dir() {
        ensure_defaults(dir, to);
        return Vec::new();
    }
    let old = defaults(from);
    let new = defaults(to);
    let normalize = |t: &str| t.replace("\r\n", "\n").trim().to_string();
    let mut to_translate = Vec::new();
    for path in files(dir) {
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if new.iter().any(|(name, body)| *name == stem && normalize(body) == normalize(&text)) {
            continue; // already in the new language
        }
        match old.iter().position(|(name, body)| *name == stem && normalize(body) == normalize(&text)) {
            Some(index) => {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::write(dir.join(format!("{}.md", new[index].0)), new[index].1);
            }
            None => to_translate.push(path),
        }
    }
    to_translate
}

/// List of templates for the model instructions.
pub fn prompt_section(dir: &Path) -> String {
    let items = list(dir);
    if items.is_empty() {
        return "<templates>\nNo templates.\n</templates>".into();
    }
    let lines: Vec<String> = items.iter().map(|(n, d)| format!("- {n}: {d}")).collect();
    format!("<templates>\n{}\n</templates>", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use crate::i18n::Lang;

    #[test]
    fn defaults_in_every_language_have_descriptions() {
        for lang in Lang::ALL {
            for (name, text) in super::defaults(lang) {
                assert!(!super::description(text).is_empty(), "{lang:?} {name}: no description in the first line");
            }
        }
    }

    #[test]
    fn switching_language_replaces_untouched_defaults_and_keeps_custom() {
        let dir = std::env::temp_dir().join(format!("loonar-templates-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::ensure_defaults(&dir, Lang::Pl);
        super::save(&dir, "My template", "> Szablon: test\n# A").unwrap();
        // A modified default template also goes to translation.
        super::save(&dir, "Opis procesu", "> Szablon: modified\n# B").unwrap();
        let pending = super::switch_defaults(&dir, Lang::Pl, Lang::En);
        let names: Vec<String> = super::files(&dir).iter().map(|p| p.file_stem().unwrap().to_string_lossy().to_string()).collect();
        assert!(names.contains(&"Preliminary feasibility analysis".to_string()), "{names:?}");
        assert!(!names.contains(&"Wstępna analiza wykonalności".to_string()));
        assert_eq!(pending.len(), 2, "own and modified template to translate: {pending:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
