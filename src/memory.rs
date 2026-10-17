//! Assistant memory between conversations: notes saved locally as Markdown files.
//!
//! Each note is a separate `<name>.md` file in the `memory` folder (next to the conversation history), with a
//! `name` and `description` header. The application itself rebuilds the `MEMORY.md` index from them. The content of all notes
//! goes into the model instructions at the start of every step, so a new conversation knows the earlier notes.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// How many characters of note content go into the model instructions; above that only the index, and the content via `memory_read`.
const PROMPT_LIMIT: usize = 40_000;
const INDEX_FILE: &str = "MEMORY.md";

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub description: String,
    pub content: String,
    pub modified: String,
}

/// File name from the note name: lowercase letters, digits and hyphens (Polish letters converted to Latin ones).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        let c = match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l', 'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(60).collect()
}

fn path_of(dir: &Path, name: &str) -> Result<PathBuf> {
    let slug = slug(name);
    if slug.is_empty() || slug == "memory" {
        bail!("{}", crate::i18n::fmt("invalid note name: '{0}'", &[name.to_string()]));
    }
    Ok(dir.join(format!("{slug}.md")))
}

/// All notes, newest first.
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<(std::time::SystemTime, Entry)> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md") && p.file_name().is_some_and(|n| n != INDEX_FILE))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let modified = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
            let stem = p.file_stem()?.to_string_lossy().to_string();
            Some((modified, parse(&stem, &text, modified)))
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries.into_iter().map(|(_, e)| e).collect()
}

fn parse(stem: &str, text: &str, modified: std::time::SystemTime) -> Entry {
    let text = text.trim_start_matches('\u{feff}');
    let (mut name, mut description, mut content) = (stem.to_string(), String::new(), text.trim().to_string());
    if let Some(rest) = text.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            for line in rest[..end].lines() {
                if let Some(v) = line.strip_prefix("name:") {
                    name = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("description:") {
                    description = v.trim().to_string();
                }
            }
            content = rest[end + 4..].trim().to_string();
        }
    }
    let modified = chrono::DateTime::<chrono::Local>::from(modified).format("%Y-%m-%d %H:%M").to_string();
    Entry { name, description, content, modified }
}

/// Saves (or overwrites) a note and refreshes the index. Returns the file name.
pub fn save(dir: &Path, name: &str, description: &str, content: &str) -> Result<String> {
    let path = path_of(dir, name)?;
    std::fs::create_dir_all(dir).with_context(|| crate::i18n::fmt("memory folder {0}", &[dir.display().to_string()]))?;
    let description = description.replace('\n', " ");
    let text = format!("---\nname: {}\ndescription: {}\n---\n\n{}\n", name.trim(), description.trim(), content.trim());
    std::fs::write(&path, text).with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))?;
    write_index(dir)?;
    Ok(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// Deletes a note and refreshes the index.
pub fn delete(dir: &Path, name: &str) -> Result<()> {
    let path = path_of(dir, name)?;
    if !path.is_file() {
        bail!("{}", crate::i18n::fmt("there is no note '{0}'", &[name.to_string()]));
    }
    std::fs::remove_file(&path).with_context(|| crate::i18n::fmt("removing {0}", &[path.display().to_string()]))?;
    write_index(dir)
}

/// Content of one note (for the model, when there are too many notes to fit into the instructions).
pub fn read(dir: &Path, name: &str) -> Result<String> {
    let path = path_of(dir, name)?;
    std::fs::read_to_string(&path).with_context(|| crate::i18n::fmt("there is no note '{0}'", &[name.to_string()]))
}

/// The `MEMORY.md` index: one line per note (for a user who looks into the folder).
fn write_index(dir: &Path) -> Result<()> {
    // The index is for the user, so it is in the interface language.
    let mut text = format!("# {}\n\n{}\n\n", crate::i18n::tr("Assistant memory"), crate::i18n::tr("Notes saved by Loonar Compass. One file per note."));
    for e in list(dir) {
        text.push_str(&format!("- [{}]({}.md) - {}\n", e.name, slug(&e.name), e.description));
    }
    std::fs::write(dir.join(INDEX_FILE), text).context(crate::i18n::tr("saving the MEMORY.md index"))
}

/// Part of the model instructions with the notes: the full content, or only the index when there is too much of it.
pub fn prompt_section(dir: &Path) -> String {
    let entries = list(dir);
    if entries.is_empty() {
        return "<memory>\nNo saved notes.\n</memory>".to_string();
    }
    let full: usize = entries.iter().map(|e| e.content.len() + e.description.len() + e.name.len()).sum();
    let mut text = String::from("<memory>\n");
    for e in &entries {
        if full <= PROMPT_LIMIT {
            text.push_str(&format!("## {} ({})\n{}\nSaved: {}\n\n{}\n\n", e.name, slug(&e.name), e.description, e.modified, e.content));
        } else {
            text.push_str(&format!("- {} ({}): {} [saved {}]\n", e.name, slug(&e.name), e.description, e.modified));
        }
    }
    if full > PROMPT_LIMIT {
        text.push_str("\nThere are many notes, so only the index is shown above; read their content with the memory_read tool.\n");
    }
    text.push_str("</memory>");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_delete_roundtrip() {
        let dir = std::env::temp_dir().join(format!("loonar-memory-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&dir, "Nazewnictwo podprocesów", "how to name subprocesses", "Embedded BPMN subprocesses, names start with a capital letter.").unwrap();
        save(&dir, "Nazewnictwo podprocesów", "how to name subprocesses", "Changed note.").unwrap();
        let entries = list(&dir);
        assert_eq!(entries.len(), 1, "the same name overwrites the file");
        assert_eq!(entries[0].content, "Changed note.");
        assert!(dir.join("nazewnictwo-podprocesow.md").is_file());
        assert!(std::fs::read_to_string(dir.join(INDEX_FILE)).unwrap().contains("nazewnictwo-podprocesow.md"));
        assert!(prompt_section(&dir).contains("Changed note."));
        delete(&dir, "nazewnictwo-podprocesow").unwrap();
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
