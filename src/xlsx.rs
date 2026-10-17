//! Excel workbooks (XLSX) assembled by the assistant: element lists, matrices, impact analysis results.

use anyhow::{Context, Result};
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook};
use serde_json::Value;

/// One sheet: name, column headers and rows (text, numbers, booleans).
pub struct Sheet {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// Excel-compatible sheet name: up to 31 characters, without the characters [ ] : * ? / \.
fn sheet_name(name: &str, index: usize) -> String {
    let clean: String = name.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    if clean.trim().is_empty() { crate::i18n::fmt("Sheet{0}", &[(index + 1).to_string()]) } else { clean }
}

pub fn build(sheets: &[Sheet]) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x0D355C))
        .set_border(FormatBorder::Thin);
    let cell = Format::new().set_border(FormatBorder::Thin).set_text_wrap();
    for (index, sheet) in sheets.iter().enumerate() {
        let ws = workbook.add_worksheet();
        ws.set_name(sheet_name(&sheet.name, index)).context(crate::i18n::tr("sheet name"))?;
        for (col, title) in sheet.columns.iter().enumerate() {
            ws.write_with_format(0, col as u16, title.as_str(), &header)?;
        }
        for (r, row) in sheet.rows.iter().enumerate() {
            let r = (r + 1) as u32;
            for (col, value) in row.iter().enumerate() {
                let col = col as u16;
                match value {
                    Value::Number(n) => {
                        ws.write_with_format(r, col, n.as_f64().unwrap_or(0.0), &cell)?;
                    }
                    Value::Bool(b) => {
                        ws.write_with_format(r, col, *b, &cell)?;
                    }
                    Value::Null => {
                        ws.write_blank(r, col, &cell)?;
                    }
                    Value::String(s) => {
                        ws.write_with_format(r, col, s.as_str(), &cell)?;
                    }
                    other => {
                        ws.write_with_format(r, col, other.to_string(), &cell)?;
                    }
                }
            }
        }
        let width = sheet.columns.len().max(sheet.rows.iter().map(Vec::len).max().unwrap_or(0));
        if width > 0 {
            ws.set_freeze_panes(1, 0)?;
            ws.autofilter(0, 0, sheet.rows.len() as u32, (width - 1) as u16)?;
            ws.autofit();
        }
    }
    workbook.save_to_buffer().context(crate::i18n::tr("saving XLSX"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn builds_workbook_with_two_sheets() {
        let bytes = super::build(&[
            super::Sheet { name: "Matrix UC/FR".into(), columns: vec!["UC".into(), "FR-01".into()], rows: vec![vec![json!("UC-01"), json!("X")]] },
            super::Sheet { name: "Numbers".into(), columns: vec!["Name".into(), "Count".into()], rows: vec![vec![json!("A"), json!(3)]] },
        ])
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("xl/worksheets/sheet2.xml").is_ok());
        let mut wb = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("xl/workbook.xml").unwrap(), &mut wb).unwrap();
        assert!(wb.contains("Matrix UCFR"), "sheet name without the / character: {wb}");
    }
}
