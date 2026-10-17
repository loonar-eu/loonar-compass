//! Rendering Markdown in the chat and in the changelog: headings, lists, tables, code blocks,
//! and within text bold (`**like this**`) and inline code (`` `like this` ``).

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
            if trimmed.is_empty() {
                ui.add_space(6.0);
            } else if let Some(heading) = trimmed.strip_prefix("### ") {
                ui.add_space(4.0);
                ui.add(egui::Label::new(inline(heading, 16.0, true)).wrap());
            } else if let Some(heading) = trimmed.strip_prefix("## ") {
                ui.add_space(8.0);
                ui.add(egui::Label::new(inline(heading, 19.0, true)).wrap());
            } else if let Some(heading) = trimmed.strip_prefix("# ") {
                ui.add_space(8.0);
                ui.add(egui::Label::new(inline(heading, 22.0, true)).wrap());
            } else if let Some((marker, item)) = list_item(trimmed) {
                // The indentation of nested items follows the indentation of the line in the text.
                let indent = (line.len() - line.trim_start().len()) as f32 * 6.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(6.0 + indent);
                    ui.label(egui::RichText::new(marker).color(theme::TEXT));
                    ui.add(egui::Label::new(inline(item, BODY_SIZE, false)).wrap());
                });
            } else {
                ui.add(egui::Label::new(inline(trimmed, BODY_SIZE, false)).wrap());
            }
        }
    });
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
                            ui.label(inline(cell, BODY_SIZE, row_no == 0));
                        }
                        ui.end_row();
                    }
                });
            });
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
    fn list_markers() {
        assert_eq!(list_item("- item"), Some(("•".to_string(), "item")));
        assert_eq!(list_item("2. second"), Some(("2.".to_string(), "second")));
        assert_eq!(list_item("2024 year"), None);
    }
}
