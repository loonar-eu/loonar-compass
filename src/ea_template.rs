//! EA document templates (Publish → Report Builder) made by Compass (LC-335): the assistant turns a document the user
//! gave into a template – a part for the package, one for each element and one for each diagram, in Markdown with EA's
//! fields such as {Element.Notes} – and Compass writes it into the model as an EA template (t_document, DocType
//! SSDOCSTYLE: a zip with an SSDocument XML whose body is RTF with EA's section bookmarks and MERGEFIELD fields).

use anyhow::{bail, Result};
use std::io::Write;

/// Options of the template (as in EA's own "Element Journal") and the sections this module writes.
const HEAD: &str = r#"<SSDocument version="1.0" name="{NAME}"><SSDocument.Options modcreate="-1" statuslike="-1" phaselike="-1" beforeafter="-1" pkgorder="1" elemorder="1" featorder="1" dgmorder="1" pkgasc="0" elemasc="0" featasc="0" dgmasc="0" imagefmt="1" hindent="9" noteless="0" skiproot="0" statuslist="" phase="All" exdetails="" wordsubst="0" nobookmarks="0" hideanonymous="0" htmlformating="0" exconndetails="" exdiagdetails="" conndirfilter="0"/><SSDocument.Sections><SSDocument.Section name="Pkg" order="1" markerstart="package &gt;" markerend="&lt; package"/><SSDocument.Section name="Pkg_Diagram" order="1" markerstart="diagram &gt;" markerend="&lt; diagram"/><SSDocument.Section name="Pkg_Element" order="2" markerstart="element &gt;" markerend="&lt; element"/><SSDocument.Section name="Pkg_Pkg" order="3" markerstart="child packages &gt;" markerend="&lt; child packages"/></SSDocument.Sections>"#;

const MARK: &str = r"\plain\f1\fs16\cf2\b\protect ";
const TEXT: &str = r"\plain\f0\fs22 ";

fn begin(section: &str, word: &str) -> String {
    format!(r"\pard{{\*\bkmkstart {s}_Begin}}{{\*\bkmkend {s}_Begin}}{MARK}{word} {{\*\bkmkstart {s}_Begin_Inner}}{{\*\bkmkend {s}_Begin_Inner}}>{TEXT}\par", s = section) + "\r\n"
}

fn end(section: &str, word: &str) -> String {
    let (head, last) = word.split_at(word.len() - 1);
    format!(r"\pard{{\*\bkmkstart {s}_End_Inner}}{{\*\bkmkend {s}_End_Inner}}{MARK}< {head}{{\*\bkmkstart {s}_End}}{{\*\bkmkend {s}_End}}{last}{TEXT}\par", s = section) + "\r\n"
}

/// Plain text for RTF: control characters escaped, everything outside ASCII as \uN?.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 128 => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
    out
}

/// A line of the template: **bold** and EA fields ({Element.Name}) inside text.
fn inline(text: &str) -> String {
    let mut out = String::new();
    for (i, part) in text.split("**").enumerate() {
        let bold = i % 2 == 1;
        if bold {
            out.push_str(r"{\b ");
        }
        let mut rest = part;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start..].find('}') else { break };
            let field = &rest[start + 1..start + len];
            // A field, also one with an argument: {Element.valueOf(Owner)}, the value of a tagged value (LC-467).
            let (name_part, argument) = match field.split_once('(') {
                Some((n, rest)) if rest.ends_with(')') => (n, &rest[..rest.len() - 1]),
                _ => (field, ""),
            };
            let is_field = name_part.split_once('.').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
                && name_part.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                && !argument.contains(['{', '}', '\\']);
            out.push_str(&escape(&rest[..start]));
            if is_field {
                out.push_str(&format!(r"{{\field{{\*\fldinst MERGEFIELD {field}}}{{\fldrslt \{{{field}\}}}}}}"));
            } else {
                out.push_str(&escape(&rest[start..start + len + 1]));
            }
            rest = &rest[start + len + 1..];
        }
        out.push_str(&escape(rest));
        if bold {
            out.push('}');
        }
    }
    out
}

/// Markdown lines (# headings, - lists, paragraphs) as RTF paragraphs.
fn part(markdown: &str) -> String {
    let mut out = String::new();
    for line in markdown.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let (level, text) = match t {
            _ if t.starts_with("### ") => (3, &t[4..]),
            _ if t.starts_with("## ") => (2, &t[3..]),
            _ if t.starts_with("# ") => (1, &t[2..]),
            _ => (0, t),
        };
        let p = match level {
            1 => format!(r"\pard\sb240\sa120\outlinelevel0{TEXT}\fs34\b\cf3 {}\par", inline(text)),
            2 => format!(r"\pard\sb200\sa100\outlinelevel1{TEXT}\fs28\b\cf3 {}\par", inline(text)),
            3 => format!(r"\pard\sb160\sa80\outlinelevel2{TEXT}\fs24\b {}\par", inline(text)),
            _ => match text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
                Some(item) => format!(r"\pard\li360\fi-240\sa60{TEXT}\bullet  {}\par", inline(item)),
                None => format!(r"\pard\sa120{TEXT}{}\par", inline(text)),
            },
        };
        out.push_str(&p);
        out.push_str("\r\n");
    }
    out
}

/// The RTF body: the package part, then a diagram part for each diagram and an element part for each element.
pub fn rtf(package: &str, element: &str, diagram: &str) -> String {
    let mut out = String::from(
        r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss Calibri;}{\f1\fswiss Arial;}}{\colortbl;\red0\green0\blue0;\red128\green128\blue128;\red31\green56\blue100;}",
    );
    out.push_str("\r\n");
    out.push_str(&begin("Pkg", "package"));
    out.push_str(&part(package));
    if !diagram.trim().is_empty() {
        out.push_str(&begin("Pkg_Diagram", "diagram"));
        out.push_str(&part(diagram));
        out.push_str(&end("Pkg_Diagram", "diagram"));
    }
    if !element.trim().is_empty() {
        out.push_str(&begin("Pkg_Element", "element"));
        out.push_str(&part(element));
        out.push_str(&end("Pkg_Element", "element"));
    }
    out.push_str(&end("Pkg", "package"));
    out.push('}');
    out
}

/// The stored template: a zip with str.dat (UTF-16 SSDocument XML, the RTF in base64), as EA keeps it in t_document.
pub fn document(name: &str, rtf: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let attr = name.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let body = base64::engine::general_purpose::STANDARD.encode(rtf.as_bytes());
    let xml = format!(
        "{}<SSDocument.Document xmlns:dt=\"urn:schemas-microsoft-com:datatypes\" dt:dt=\"bin.base64\">{body}</SSDocument.Document></SSDocument>",
        HEAD.replace("{NAME}", &attr)
    );
    let utf16: Vec<u8> = xml.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("str.dat", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))?;
    zip.write_all(&utf16)?;
    Ok(zip.finish()?.into_inner())
}

/// The binary value as an SQL literal of the repository's database (EA's Repository.RepositoryType()).
pub fn binary_literal(repository_type: &str, bytes: &[u8]) -> Result<String> {
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    Ok(match repository_type.to_uppercase().as_str() {
        "POSTGRES" => format!("decode('{hex}', 'hex')"),
        "SQLSVR" | "ASA" => format!("0x{hex}"),
        "MYSQL" | "SL3" | "JET" | "" => format!("X'{hex}'"),
        other => bail!("saving an EA template is not supported for a {other} repository yet: create it in EA (Publish → Report Builder → Templates)"),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn rtf_has_sections_fields_and_escapes() {
        let rtf = super::rtf("# {Pkg.Name}\n{Pkg.Notes}", "## **{Element.Name}** ({Element.Type})\n- Opis: {Element.Notes}", "");
        assert!(rtf.contains(r"{\*\bkmkstart Pkg_Begin}") && rtf.contains(r"{\*\bkmkstart Pkg_Element_End}"));
        assert!(rtf.contains(r"MERGEFIELD Element.Notes") && rtf.contains(r"\{Pkg.Name\}"));
        assert!(rtf.contains(r"\u-") || rtf.contains(r"Opis"));
        assert!(!rtf.contains("Pkg_Diagram"), "no diagram part when none is given");
        // LC-467: a tagged value by its name is a field too.
        assert!(super::inline("Owner: {Element.valueOf(Owner)}").contains("MERGEFIELD Element.valueOf(Owner)"));
        let zip = super::document("Specyfikacja usług", &rtf).unwrap();
        assert!(zip.starts_with(b"PK"));
        assert_eq!(super::binary_literal("POSTGRES", &[1, 255]).unwrap(), "decode('01FF', 'hex')");
        assert!(super::binary_literal("ORACLE", &[1]).is_err());
    }

    /// A template saved in the test model and a report made with it (package, diagrams, elements).
    /// Run with: `cargo test ea_template_live -- --ignored --nocapture` (LOONAR_TEST_PROPS = a copy of the model).
    #[test]
    #[ignore = "requires EA, a license and the model from LOONAR_TEST_PROPS"]
    fn ea_template_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let file = std::env::temp_dir().join("loonar_template_live.docx");
        let ops = [
            serde_json::json!({"op":"save_document_template","name":"Compass – opis interesariuszy","package_part":"# Pakiet: {Pkg.Name}\n{Pkg.Notes}","diagram_part":"## Diagram {Diagram.Name}\n{Diagram.DiagramImg}","element_part":"## **{Element.Name}** ({Element.Type})\n- Status: {Element.Status}\n{Element.Notes}"}),
            serde_json::json!({"op":"generate_report","package":385,"file":file.display().to_string(),"template":"Compass – opis interesariuszy"}),
        ];
        let out = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("template and report");
        crate::ea::shutdown();
        println!("{}", out["warnings"]);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&file).unwrap()).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
        let text: String = xml.split("<w:t").skip(1).filter_map(|p| p.split_once('>').map(|(_, r)| r.split("</w:t>").next().unwrap_or(""))).collect::<Vec<_>>().join("|");
        println!("{}", &text.chars().take(1200).collect::<String>());
        assert!(text.contains("Pakiet: Stakeholders"), "package part");
        assert!(text.contains("Chief Financial Officer"), "element part for each element");
        assert!(!text.contains("(Note)"), "notes are not described as elements");
        assert!(xml.contains("<w:drawing") || xml.contains("<pic:pic"), "diagram picture");
    }
}

#[cfg(test)]
mod tag_tests {
    /// LC-467: {Element.valueOf(Owner)} gives the tag's value in EA's report, on a copy of the round 4 model:
    /// `LOONAR_TEST_PROPS=E:/livetest/r440/m5.properties LOONAR_TEST_PACKAGE=1116 cargo test template_tag_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA, a license and the model from LOONAR_TEST_PROPS"]
    fn template_tag_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let package: i64 = std::env::var("LOONAR_TEST_PACKAGE").unwrap().parse().unwrap();
        let file = std::env::temp_dir().join("loonar_template_tag.docx");
        let ops = [
            serde_json::json!({"op":"save_document_template","name":"Compass – tagi","package_part":"# {Pkg.Name}","element_part":"## {Element.Name}\n- Owner: {Element.valueOf(Owner)}"}),
            serde_json::json!({"op":"generate_report","package":package,"file":file.display().to_string(),"template":"Compass – tagi"}),
        ];
        crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions::default()).expect("template and report");
        crate::ea::shutdown();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&file).unwrap()).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
        let text: String = xml.split("<w:t").skip(1).filter_map(|p| p.split_once('>').map(|(_, r)| r.split("</w:t>").next().unwrap_or(""))).collect::<Vec<_>>().join("|");
        println!("{}", &text.chars().take(800).collect::<String>());
        assert!(text.replace('|', "").contains("Zespół A") && !text.contains("valueOf"), "{text}");
    }
}
