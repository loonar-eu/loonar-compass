//! Spell checking with the Windows system dictionary (the same one the system applications use).
//! The language comes from the system settings; when Windows has no dictionary for it, checking is disabled.

use std::ops::Range;
use windows::core::HSTRING;
use windows::Win32::Foundation::S_OK;
use windows::Win32::Globalization::{
    GetUserDefaultLocaleName, GetUserPreferredUILanguages, ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory,
    MUI_LANGUAGE_NAME,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

pub struct SpellChecker {
    checker: ISpellChecker,
    /// Dictionary language tag, e.g. `pl-PL`.
    pub language: String,
}

/// Languages from the system settings in the order they are tried: regional format, then display languages.
fn system_languages() -> Vec<String> {
    let mut languages = Vec::new();
    // SAFETY: the buffers are local and have the sizes passed to the functions.
    unsafe {
        let mut name = [0u16; 85];
        let len = GetUserDefaultLocaleName(&mut name);
        if len > 1 {
            languages.push(String::from_utf16_lossy(&name[..len as usize - 1]));
        }
        let mut count = 0u32;
        let mut size = 0u32;
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, windows::core::PWSTR::null(), &mut size).is_ok() && size > 0 {
            let mut buffer = vec![0u16; size as usize];
            if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, windows::core::PWSTR(buffer.as_mut_ptr()), &mut size).is_ok() {
                // List of zero-separated strings, terminated by a double zero.
                languages.extend(buffer.split(|c| *c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy));
            }
        }
    }
    languages.dedup();
    languages
}

impl SpellChecker {
    /// Dictionary for the first system language that Windows supports. Call from the window thread (COM is already initialized there).
    pub fn system() -> Option<Self> {
        // SAFETY: standard COM calls; the results are checked.
        unsafe {
            let factory: ISpellCheckerFactory = CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            for language in system_languages() {
                let tag = HSTRING::from(language.as_str());
                if factory.IsSupported(&tag).map(|b| b.as_bool()).unwrap_or(false) {
                    if let Ok(checker) = factory.CreateSpellChecker(&tag) {
                        return Some(Self { checker, language });
                    }
                }
            }
        }
        None
    }

    /// Adds a word to the user's Windows dictionary of this language (it is then accepted in every application).
    pub fn add(&self, word: &str) -> bool {
        // SAFETY: standard COM call.
        unsafe { self.checker.Add(&HSTRING::from(word)).is_ok() }
    }

    /// Ranges of misspelled words as byte positions in `text`.
    pub fn errors(&self, text: &str) -> Vec<Range<usize>> {
        let mut found = Vec::new();
        if text.trim().is_empty() {
            return found;
        }
        // Windows reports positions in UTF-16 units: the table maps them to byte positions.
        let mut byte_at = Vec::with_capacity(text.len() + 1);
        for (offset, ch) in text.char_indices() {
            for _ in 0..ch.len_utf16() {
                byte_at.push(offset);
            }
        }
        byte_at.push(text.len());

        // SAFETY: standard COM calls; the results are checked.
        unsafe {
            let Ok(errors) = self.checker.Check(&HSTRING::from(text)) else { return found };
            loop {
                let mut error = None;
                if errors.Next(&mut error) != S_OK {
                    break;
                }
                let Some(error) = error else { break };
                let (Ok(start), Ok(len)) = (error.StartIndex(), error.Length()) else { continue };
                let (start, end) = (start as usize, (start + len) as usize);
                if end < byte_at.len() && start < end {
                    found.push(byte_at[start]..byte_at[end]);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requires the system dictionary, so it is run manually: `cargo test spell -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn system_dictionary_marks_misspelled_word() {
        // In a test, COM is not initialized yet (in the application, the windowing library does it).
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let checker = SpellChecker::system().expect("the system has a dictionary for its language");
        println!("dictionary language: {}", checker.language);
        let text = "zażółć gęślą jaźń samooo koniec";
        let errors = checker.errors(text);
        let words: Vec<&str> = errors.iter().map(|r| &text[r.clone()]).collect();
        println!("misspelled words: {words:?}");
        assert!(words.contains(&"samooo"));
    }
}
