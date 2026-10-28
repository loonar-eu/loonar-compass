//! The changelog for the assistant (LC-300): no longer the whole file in every system prompt (about 18 000 tokens), but
//! a short table of contents plus the prerequisites and known limitations there, and the tool app_changelog that
//! returns the parts asked for (a version or words to search).

use serde_json::{json, Value};

const CHANGELOG_EN: &str = include_str!("../CHANGELOG.en.md");
/// The most text one call returns.
const MAX_ANSWER: usize = 16_000;

/// A version of the changelog: its title ("Version 0.9.1 (2026-10-27)") and its text.
fn versions() -> Vec<(&'static str, &'static str)> {
    CHANGELOG_EN
        .split("\n## ")
        .skip(1)
        .map(|part| match part.split_once('\n') {
            Some((title, body)) => (title.trim(), body),
            None => (part.trim(), ""),
        })
        .collect()
}

/// The subsections "### …" of a text with their text (the part before the first one is "").
fn subsections(body: &'static str) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    let mut parts = body.split("\n### ");
    if let Some(first) = parts.next() {
        out.push(("", first));
    }
    for part in parts {
        match part.split_once('\n') {
            Some((title, text)) => out.push((title.trim(), text)),
            None => out.push((part.trim(), "")),
        }
    }
    out
}

/// A subsection of the changelog by its title, e.g. "Prerequisites".
fn subsection(title: &str) -> String {
    versions()
        .into_iter()
        .flat_map(|(_, body)| subsections(body))
        .find(|(t, _)| t.eq_ignore_ascii_case(title))
        .map(|(_, text)| text.trim().to_string())
        .unwrap_or_default()
}

/// The part of the system prompt about the application: how to look things up, the table of contents, the
/// prerequisites and the known limitations (stable text: it stays in the cached prefix).
pub fn prompt_section() -> String {
    let toc: Vec<String> = versions()
        .into_iter()
        .map(|(title, body)| {
            let topics: Vec<&str> = subsections(body).into_iter().map(|(t, _)| t).filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("Prerequisites") && !t.eq_ignore_ascii_case("Known limitations")).collect();
            if topics.is_empty() { format!("- {title}") } else { format!("- {title}: {}", topics.join("; ")) }
        })
        .collect();
    format!(
        "Answer questions about yourself, this application, its features, versions and limitations from its changelog: call app_changelog \
         (a version such as \"0.9.1\", or words to search such as \"Confluence certificate\") and answer from what it returns, not from memory. \
         Do not attribute to the application any features that are not in the changelog.\n\
         <changelog_contents>\n{}\n</changelog_contents>\n\
         <prerequisites>\n{}\n</prerequisites>\n\
         <known_limitations>\n{}\n</known_limitations>",
        toc.join("\n"),
        subsection("Prerequisites"),
        subsection("Known limitations"),
    )
}

pub fn definition() -> Value {
    json!({
        "name": "app_changelog",
        "description": "The changelog of this application (Loonar Compass): what each version added or changed, newest first. Give a version (e.g. \"0.9.1\", \"latest\") \
for that version's notes, or words to search for the matching parts of all versions. Without either it returns the latest version.",
        "input_schema": {
            "type": "object",
            "properties": {
                "version": { "type": "string", "description": "A version number such as 0.9.1, or latest." },
                "search": { "type": "string", "description": "Words to look for, e.g. \"SharePoint upload\"." }
            }
        }
    })
}

/// The parts of the changelog asked for.
pub fn lookup(input: &Value) -> String {
    let all = versions();
    let version = input["version"].as_str().map(str::trim).filter(|v| !v.is_empty());
    let search = input["search"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let mut out = String::new();
    let mut push = |text: String, out: &mut String| {
        if out.len() + text.len() > MAX_ANSWER {
            let room = MAX_ANSWER.saturating_sub(out.len());
            out.push_str(&text.chars().take(room).collect::<String>());
            out.push_str("\n(…shortened: ask for a version or narrower words for the rest)");
            false
        } else {
            out.push_str(&text);
            true
        }
    };
    if let Some(words) = search {
        let words: Vec<String> = words.to_lowercase().split_whitespace().map(String::from).collect();
        let matches = |text: &str, all_words: bool| {
            let t = text.to_lowercase();
            if all_words { words.iter().all(|w| t.contains(w)) } else { words.iter().any(|w| t.contains(w)) }
        };
        for all_words in [true, false] {
            for (title, body) in &all {
                for (sub, text) in subsections(body) {
                    if matches(&format!("{sub}\n{text}"), all_words) {
                        let heading = if sub.is_empty() { format!("## {title}") } else { format!("## {title} → {sub}") };
                        if !push(format!("{heading}\n{}\n\n", text.trim()), &mut out) {
                            return out;
                        }
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        return format!("Nothing in the changelog matches \"{}\". Versions: {}", words.join(" "), all.iter().map(|(t, _)| *t).collect::<Vec<_>>().join("; "));
    }
    let wanted = match version {
        None => all.first(),
        Some(v) if v.eq_ignore_ascii_case("latest") => all.first(),
        Some(v) => all.iter().find(|(title, _)| title.split_whitespace().nth(1).is_some_and(|n| n == v.trim_start_matches(['v', 'V']))),
    };
    match wanted {
        Some((title, body)) => {
            push(format!("## {title}\n{}", body.trim()), &mut out);
            out
        }
        None => format!("There is no version {} in the changelog. Versions: {}", version.unwrap_or(""), all.iter().map(|(t, _)| *t).collect::<Vec<_>>().join("; ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_part_is_short_and_keeps_prerequisites() {
        let section = prompt_section();
        assert!(section.len() < CHANGELOG_EN.len() / 4, "{} of {}", section.len(), CHANGELOG_EN.len());
        assert!(section.contains(versions()[0].0) && section.contains("Version 0.1"));
        assert!(!subsection("Prerequisites").is_empty() && section.contains(&subsection("Prerequisites")));
        assert!(section.contains(&subsection("Known limitations")));
    }

    #[test]
    fn versions_and_searches() {
        let latest = lookup(&json!({}));
        assert!(latest.starts_with(&format!("## {}", versions()[0].0)), "{}", &latest[..60]);
        assert_eq!(lookup(&json!({"version": "latest"})), latest);
        let older = lookup(&json!({"version": "0.8.2"}));
        assert!(older.starts_with("## Version 0.8.2") && !older.contains("## Version 0.8.1"));
        assert!(lookup(&json!({"version": "9.9"})).starts_with("There is no version"));
        let found = lookup(&json!({"search": "SQL Server"}));
        assert!(found.contains("SQL Server") && found.len() <= MAX_ANSWER + 100);
        assert!(lookup(&json!({"search": "zzqqxx"})).starts_with("Nothing in the changelog"));
    }
}
