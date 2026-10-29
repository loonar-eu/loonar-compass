//! Rendering Markdown in the chat and in the changelog: headings, lists, tables, code blocks,
//! and within text bold (`**like this**`), inline code (`` `like this` ``) and links (`[text](https://…)` and bare
//! https:// addresses, clickable: they open in the browser, LC-310).

use crate::theme;
use eframe::egui;
use egui::text::LayoutJob;

/// Name of the font family with the bold typeface (registered at application start-up).
pub const BOLD_FAMILY: &str = "bold";

const BODY_SIZE: f32 = 15.0;

/// Draws `text` as Markdown. `salt` distinguishes tables of different messages (grid identifiers).
pub fn show(ui: &mut egui::Ui, text: &str, salt: impl std::hash::Hash) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut table_no = 0;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Code block: from ``` to ```.
            if trimmed.starts_with("```") {
                let mut code = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    code.push(lines[i]);
                    i += 1;
                }
                i += 1;
                egui::Frame::none()
                    .fill(theme::SIDEBAR)
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(code.join("\n")).monospace().color(theme::TEXT)).wrap());
                    });
                continue;
            }

            // Table: consecutive lines starting with `|`.
            if trimmed.starts_with('|') {
                let mut rows: Vec<Vec<&str>> = Vec::new();
                while i < lines.len() && lines[i].trim().starts_with('|') {
                    let row = lines[i].trim().trim_matches('|');
                    let is_separator = !row.is_empty() && row.chars().all(|c| matches!(c, '-' | ':' | '|' | ' '));
                    if !is_separator {
                        rows.push(row.split('|').map(str::trim).collect());
                    }
                    i += 1;
                }
                table(ui, &rows, (&salt, table_no));
                table_no += 1;
                continue;
            }

            i += 1;
            if is_image_only(trimmed) {
                // A Markdown image link such as `![Order process](diagram:73)` or a placeholder such as
                // `[Diagram image shown in chat]`: the app shows diagram images itself, so the line would only appear as raw text.
                continue;
            }
            if trimmed.is_empty() {
                ui.add_space(6.0);
            } else if let Some(heading) = trimmed.strip_prefix("### ") {
                ui.add_space(4.0);
                rich(ui, heading, 16.0, true);
            } else if let Some(heading) = trimmed.strip_prefix("## ") {
                ui.add_space(8.0);
                rich(ui, heading, 19.0, true);
            } else if let Some(heading) = trimmed.strip_prefix("# ") {
                ui.add_space(8.0);
                rich(ui, heading, 22.0, true);
            } else if let Some((marker, item)) = list_item(trimmed) {
                // The indentation of nested items follows the indentation of the line in the text.
                let indent = (line.len() - line.trim_start().len()) as f32 * 6.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(6.0 + indent);
                    ui.label(egui::RichText::new(marker).color(theme::TEXT));
                    rich(ui, item, BODY_SIZE, false);
                });
            } else {
                rich(ui, trimmed, BODY_SIZE, false);
            }
        }
    });
}

/// A line holding only a Markdown image (`![alt](target)`) or only a bracketed placeholder (`[Diagram …]`).
fn is_image_only(line: &str) -> bool {
    let image = line.starts_with("![") && line.ends_with(')') && line.contains("](");
    let placeholder = line.starts_with('[') && line.ends_with(']') && !line[1..line.len() - 1].contains(['[', ']']);
    image || placeholder
}

/// Recognises a list item: `- text`, `* text` or `1. text`. Returns the marker to show and the content.
fn list_item(line: &str) -> Option<(String, &str)> {
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return Some(("•".to_string(), rest));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits <= 3 {
        if let Some(rest) = line[digits..].strip_prefix(". ") {
            return Some((format!("{}.", &line[..digits]), rest));
        }
    }
    None
}

fn table(ui: &mut egui::Ui, rows: &[Vec<&str>], salt: impl std::hash::Hash) {
    egui::Frame::none()
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt(("md_table_scroll", &salt)).show(ui, |ui| {
                egui::Grid::new(("md_table", &salt)).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                    for (row_no, row) in rows.iter().enumerate() {
                        for cell in row {
                            // The first row is the table header.
                            // Cells keep their text on one line (the table scrolls sideways): wrapped, a grid gives
                            // every column its narrowest width (found in the 0.9.2 lab tests: very narrow tables).
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            rich(ui, cell, BODY_SIZE, row_no == 0);
                        }
                        ui.end_row();
                    }
                });
            });
        });
}

/// A part of a line: text with inline formatting, or a link (label, address).
#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Text(&'a str),
    Link(&'a str, &'a str),
}

/// Splits a line into text and links: `[label](https://…)` and bare http(s) addresses outside `code`. Only http and
/// https addresses become links (nothing that could start a program or open a file).
fn pieces_with_links(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut start = 0; // start of the pending text
    let mut i = 0;
    let mut in_code = false;
    let bytes = text.as_bytes();
    let is_web = |s: &str| s.starts_with("https://") || s.starts_with("http://");
    while i < text.len() {
        let c = bytes[i];
        if c == b'`' {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if !in_code && c == b'[' && (i == 0 || bytes[i - 1] != b'!') {
            if let Some(close) = text[i + 1..].find("](").map(|p| i + 1 + p) {
                if let Some(end) = text[close + 2..].find(')').map(|p| close + 2 + p) {
                    let (label, url) = (&text[i + 1..close], text[close + 2..end].trim());
                    if is_web(url) && !label.is_empty() && !label.contains('[') {
                        if start < i {
                            out.push(Piece::Text(&text[start..i]));
                        }
                        out.push(Piece::Link(label, url));
                        i = end + 1;
                        start = i;
                        continue;
                    }
                }
            }
        }
        if !in_code && is_web(&text[i..]) && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) {
            let mut end = text[i..].find(|ch: char| ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '`')).map_or(text.len(), |p| i + p);
            // Punctuation after an address belongs to the sentence (also a closing bracket without an opening one in it).
            while end > i {
                let last = text[..end].chars().next_back().unwrap_or(' ');
                let unbalanced = last == ')' && text[i..end].matches('(').count() < text[i..end].matches(')').count();
                if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*') || unbalanced {
                    end -= last.len_utf8();
                } else {
                    break;
                }
            }
            if end > i + "https://".len() {
                if start < i {
                    out.push(Piece::Text(&text[start..i]));
                }
                out.push(Piece::Link(&text[i..end], &text[i..end]));
                i = end;
                start = i;
                continue;
            }
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    if start < text.len() {
        out.push(Piece::Text(&text[start..]));
    }
    out
}

/// The addresses a line shows as clickable links.
#[cfg(test)]
pub fn link_targets(text: &str) -> Vec<String> {
    pieces_with_links(text).into_iter().filter_map(|p| match p { Piece::Link(_, url) => Some(url.to_string()), Piece::Text(_) => None }).collect()
}

/// One line with inline formatting; links in it are clickable and open in the browser.
fn rich(ui: &mut egui::Ui, text: &str, size: f32, all_bold: bool) {
    let pieces = pieces_with_links(text);
    let extend = ui.style().wrap_mode == Some(egui::TextWrapMode::Extend);
    let label = |job| if extend { egui::Label::new(job).extend() } else { egui::Label::new(job).wrap() };
    if !pieces.iter().any(|p| matches!(p, Piece::Link(..))) {
        ui.add(label(inline(text, size, all_bold)));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for piece in pieces {
            match piece {
                Piece::Text(t) => {
                    ui.add(label(inline(t, size, all_bold)));
                }
                Piece::Link(label, url) => {
                    let label = label.replace("**", "").replace('`', "");
                    ui.hyperlink_to(egui::RichText::new(label).size(size).color(theme::NAV_ACTIVE), url).on_hover_text(url);
                }
            }
        }
    });
}

/// Lays out one line of text with inline formatting: `**bold**` and `` `code` ``.
fn inline(text: &str, size: f32, all_bold: bool) -> LayoutJob {
    let normal = egui::TextFormat {
        font_id: egui::FontId::new(size, egui::FontFamily::Proportional),
        color: theme::TEXT,
        ..Default::default()
    };
    let bold = egui::TextFormat {
        font_id: egui::FontId::new(size, egui::FontFamily::Name(BOLD_FAMILY.into())),
        ..normal.clone()
    };
    let code = egui::TextFormat {
        font_id: egui::FontId::new(size - 1.0, egui::FontFamily::Monospace),
        background: theme::SIDEBAR,
        ..normal.clone()
    };

    let mut job = LayoutJob::default();
    let mut is_bold = all_bold;
    let mut rest = text;
    while !rest.is_empty() {
        let next_bold = rest.find("**");
        let next_code = rest.find('`');
        match (next_bold, next_code) {
            (Some(b), c) if c.map_or(true, |c| b < c) => {
                job.append(&rest[..b], 0.0, if is_bold { bold.clone() } else { normal.clone() });
                is_bold = !is_bold || all_bold;
                rest = &rest[b + 2..];
            }
            (_, Some(c)) => {
                job.append(&rest[..c], 0.0, if is_bold { bold.clone() } else { normal.clone() });
                let after = &rest[c + 1..];
                match after.find('`') {
                    Some(end) => {
                        job.append(&after[..end], 0.0, code.clone());
                        rest = &after[end + 1..];
                    }
                    // A single ` without a pair: show it literally.
                    None => {
                        job.append(&rest[c..], 0.0, if is_bold { bold.clone() } else { normal.clone() });
                        rest = "";
                    }
                }
            }
            _ => {
                job.append(rest, 0.0, if is_bold { bold.clone() } else { normal.clone() });
                rest = "";
            }
        }
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(text: &str) -> Vec<(String, bool, bool)> {
        let job = inline(text, BODY_SIZE, false);
        job.sections
            .iter()
            .filter(|s| !s.byte_range.is_empty())
            .map(|s| {
                (
                    job.text[s.byte_range.clone()].to_string(),
                    s.format.font_id.family == egui::FontFamily::Name(BOLD_FAMILY.into()),
                    s.format.font_id.family == egui::FontFamily::Monospace,
                )
            })
            .collect()
    }

    #[test]
    fn inline_bold_and_code() {
        assert_eq!(
            pieces("**Package** `Model` plain"),
            vec![
                ("Package".to_string(), true, false),
                (" ".to_string(), false, false),
                ("Model".to_string(), false, true),
                (" plain".to_string(), false, false),
            ]
        );
    }

    #[test]
    fn links_in_text() {
        use Piece::*;
        assert_eq!(pieces_with_links("See [LC-1](https://x.atlassian.net/browse/LC-1)."), vec![Text("See "), Link("LC-1", "https://x.atlassian.net/browse/LC-1"), Text(".")]);
        assert_eq!(pieces_with_links("Page: https://wiki.example.com/p?id=5, done"), vec![Text("Page: "), Link("https://wiki.example.com/p?id=5", "https://wiki.example.com/p?id=5"), Text(", done")]);
        assert_eq!(pieces_with_links("(https://a.example.com/x)"), vec![Text("("), Link("https://a.example.com/x", "https://a.example.com/x"), Text(")")]);
        assert_eq!(pieces_with_links("`https://in.code/x` and [f](file:///C:/x)"), vec![Text("`https://in.code/x` and [f](file:///C:/x)")]);
        assert_eq!(pieces_with_links("![d](https://img.example.com/a.png)"), vec![Text("![d]("), Link("https://img.example.com/a.png", "https://img.example.com/a.png"), Text(")")]);
        assert_eq!(pieces_with_links("plain **text**"), vec![Text("plain **text**")]);
    }

    #[test]
    fn list_markers() {
        assert_eq!(list_item("- item"), Some(("•".to_string(), "item")));
        assert_eq!(list_item("2. second"), Some(("2.".to_string(), "second")));
        assert_eq!(list_item("2024 year"), None);
    }

    #[test]
    fn image_only_lines() {
        assert!(is_image_only("![Order process](diagram:73)"));
        assert!(!is_image_only("See ![x](y) here"));
        assert!(!is_image_only("![not closed"));
        assert!(is_image_only("[Diagram image shown in chat]"));
        assert!(!is_image_only("[a] and [b]"));
        assert!(!is_image_only("[x](https://example.com)"));
    }
}
