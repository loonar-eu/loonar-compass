//! A Polish dictionary for EA's own spell checker (the Notes window and the notes of the properties): EA comes with
//! English dictionaries only and checks in the Latin-1 character set, so every Polish word is underlined. Compass
//! builds a word list of every inflected form from the SJP.pl dictionary (sjp.pl/sl/odmiany, licences GPL 2, LGPL 2.1,
//! Apache 2.0 or CC BY 4.0 – used here under CC BY 4.0), in ISO-8859-2, and adds it to EA's user dictionaries with the
//! Central European character set (CharSet 2). EA reads both at its start; the EA add-in sets them again before EA's
//! spell checker starts, because EA writes its own CharSet back when it closes.
//!
//! The list leaves out entries the spell checker cannot load (with digits, hyphens or spaces – they made it hang) and
//! keeps one spelling per word ignoring case (EA checks without case). Known gap: EA splits words at "ż" in its
//! own editor, so words with "ż" can stay underlined.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

const SJP_PAGE: &str = "https://sjp.pl/sl/odmiany/";
/// The file name of the dictionary in EA's user dictionary folder (also checked by the EA add-in).
pub const FILE: &str = "loonar-pl.tlx";
const SSCE_KEY: &str = r"HKCU\Software\Sparx Systems\EA64\SSCE";

/// The word list Compass keeps (built once): %APPDATA%\Loonar Compass\spelling\loonar-pl.tlx.
pub fn built_file() -> PathBuf {
    crate::config::user_data_dir().join("spelling").join(FILE)
}

/// One letter in ISO-8859-2 (the Central European set EA's spell checker uses with CharSet 2); None outside it.
fn latin2(c: char) -> Option<u8> {
    if c.is_ascii() {
        return Some(c as u8);
    }
    let table = [
        ('Ą', 0xA1), ('Ł', 0xA3), ('Ś', 0xA6), ('Ź', 0xAC), ('Ż', 0xAF), ('ą', 0xB1), ('ł', 0xB3), ('ś', 0xB6), ('ź', 0xBC), ('ż', 0xBF),
        ('Ć', 0xC6), ('Ę', 0xCA), ('Ń', 0xD1), ('Ó', 0xD3), ('ć', 0xE6), ('ę', 0xEA), ('ń', 0xF1), ('ó', 0xF3),
        ('Á', 0xC1), ('Ä', 0xC4), ('Ç', 0xC7), ('Č', 0xC8), ('É', 0xC9), ('Ë', 0xCB), ('Í', 0xCD), ('Ö', 0xD6), ('Ř', 0xD8), ('Ú', 0xDA), ('Ü', 0xDC), ('Ý', 0xDD),
        ('á', 0xE1), ('ä', 0xE4), ('ç', 0xE7), ('č', 0xE8), ('é', 0xE9), ('ë', 0xEB), ('í', 0xED), ('ö', 0xF6), ('ř', 0xF8), ('ú', 0xFA), ('ü', 0xFC), ('ý', 0xFD),
        ('Š', 0xA9), ('š', 0xB9), ('Ž', 0xAE), ('ž', 0xBE), ('ß', 0xDF),
    ];
    table.iter().find(|(l, _)| *l == c).map(|(_, b)| *b)
}

/// The word list from SJP's odm.txt (lines of comma-separated forms): letters only, one spelling per word ignoring
/// case (the lower-case one when there is one), in ISO-8859-2, sorted, CRLF.
pub fn word_list(odm: &str) -> Vec<u8> {
    let mut words: BTreeMap<String, String> = BTreeMap::new();
    for form in odm.lines().flat_map(|l| l.split(',')).map(str::trim) {
        if form.is_empty() || !form.chars().all(char::is_alphabetic) || !form.chars().all(|c| latin2(c).is_some()) {
            continue;
        }
        let key = form.to_lowercase();
        let lower = form == key;
        match words.get(&key) {
            Some(_) if !lower => {}
            _ => {
                words.insert(key, form.to_string());
            }
        }
    }
    let mut encoded: Vec<Vec<u8>> = words.into_values().map(|w| w.chars().filter_map(latin2).collect()).collect();
    encoded.sort();
    let mut out = Vec::with_capacity(encoded.len() * 14);
    for w in encoded {
        out.extend_from_slice(&w);
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Downloads the newest SJP.pl list of inflected forms and builds the word list; returns the number of words.
pub fn build() -> Result<usize> {
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(300)).build()?;
    let page = client.get(SJP_PAGE).send()?.error_for_status()?.text()?;
    let name = page
        .split('"')
        .find(|s| s.starts_with("sjp-odm-") && s.ends_with(".zip"))
        .context("the SJP.pl page has no list of inflected forms (sjp-odm-….zip)")?
        .to_string();
    let bytes = client.get(format!("{SJP_PAGE}{name}")).send()?.error_for_status()?.bytes()?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut odm = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("odm.txt").context("odm.txt in the SJP.pl archive")?, &mut odm)?;
    let list = word_list(&odm);
    let count = list.iter().filter(|b| **b == b'\n').count();
    if count < 100_000 {
        bail!("the SJP.pl list has only {count} words");
    }
    let file = built_file();
    std::fs::create_dir_all(file.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(&file, list)?;
    Ok(count)
}

fn reg_query(value: &str) -> Option<String> {
    let out = std::process::Command::new(crate::config::system_program("reg.exe")).args(["query", SSCE_KEY, "/v", value]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().find(|l| l.trim_start().starts_with(value))?;
    let mut parts = line.split_whitespace().skip(2);
    let first = parts.next()?;
    Some(std::iter::once(first).chain(parts).collect::<Vec<_>>().join(" "))
}

fn reg_set(value: &str, kind: &str, data: &str) -> Result<()> {
    let status = std::process::Command::new(crate::config::system_program("reg.exe")).args(["add", SSCE_KEY, "/v", value, "/t", kind, "/d", data, "/f"]).output()?;
    if !status.status.success() {
        bail!("{}", String::from_utf8_lossy(&status.stderr));
    }
    Ok(())
}

/// Whether the Polish dictionary is built (and how many words), for the settings.
pub fn status() -> Option<usize> {
    std::fs::metadata(built_file()).ok().map(|m| (m.len() / 12) as usize)
}

/// EA's spell checker in the language of Compass: for Polish (with the list built) the list among EA's user
/// dictionaries and CharSet 2; for another language the list taken out and CharSet 1 (EA's own) again.
pub fn apply(language: &str) -> Result<String> {
    let path = reg_query("UserLexPath").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Sparx Systems\EA"));
    let files = reg_query("UserLexFiles").unwrap_or_else(|| "userdic.tlx".into());
    let mut list: Vec<String> = files.split(',').map(str::trim).filter(|f| !f.is_empty() && *f != FILE).map(str::to_string).collect();
    if language == "pl" && built_file().is_file() {
        std::fs::create_dir_all(&path)?;
        std::fs::copy(built_file(), path.join(FILE)).context("copying the dictionary to EA's dictionaries")?;
        list.push(FILE.to_string());
        reg_set("UserLexFiles", "REG_SZ", &list.join(","))?;
        reg_set("CharSet", "REG_DWORD", "2")?;
        return Ok(crate::i18n::tr("EA checks spelling in Polish (SJP.pl dictionary); EA uses it after a restart.").to_string());
    }
    if files.split(',').any(|f| f.trim() == FILE) {
        reg_set("UserLexFiles", "REG_SZ", &list.join(","))?;
        reg_set("CharSet", "REG_DWORD", "1")?;
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_list_keeps_letters_one_spelling_latin2() {
        let odm = "książka, książki, książce\n3D\nPolska, polska, polskiej\na-b, a cappella\nżaba, Żaba\nWien, wiener\nŻółć\n";
        let list = super::word_list(odm);
        let words: Vec<&[u8]> = list.split(|b| *b == b'\n').map(|w| w.strip_suffix(b"\r").unwrap_or(w)).filter(|w| !w.is_empty()).collect();
        // ą = 0xB1 and ż = 0xBF in ISO-8859-2; "3D", "a-b" and "a cappella" are left out; Polska/polska and żaba/Żaba once.
        assert!(words.contains(&&b"ksi\xb1\xbfka"[..]));
        assert!(words.contains(&&b"polska"[..]) && !words.contains(&&b"Polska"[..]));
        assert!(words.contains(&&b"\xbfaba"[..]) && !words.contains(&&b"\xafaba"[..]));
        assert!(words.contains(&&b"\xaf\xf3\xb3\xe6"[..]), "a word only written with a capital keeps it");
        assert!(!words.iter().any(|w| w.contains(&b'-') || w.contains(&b' ') || w.iter().any(u8::is_ascii_digit)));
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted);
    }

    /// The real SJP.pl list (LOONAR_SJP_ZIP = a downloaded sjp-odm-….zip) gives the list tested in EA.
    #[test]
    #[ignore = "needs the SJP.pl archive"]
    fn word_list_of_real_sjp() {
        let file = std::fs::File::open(std::env::var("LOONAR_SJP_ZIP").unwrap()).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let mut odm = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("odm.txt").unwrap(), &mut odm).unwrap();
        let list = super::word_list(&odm);
        let count = list.iter().filter(|b| **b == b'\n').count();
        println!("{count} words, {} bytes", list.len());
        if let Ok(out) = std::env::var("LOONAR_SJP_OUT") {
            std::fs::write(out, &list).unwrap();
        }
        assert!(count > 4_000_000);
    }
}
