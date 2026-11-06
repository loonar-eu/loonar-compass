//! PDF from documents through the installed Microsoft Office (Word for DOCX, PowerPoint for PPTX), driven through
//! COM in its own thread, invisibly. Without Office the document stays in its own format and the assistant says so.

use crate::com::{b, i, s, Disp};
use anyhow::{anyhow, Context, Result};
use std::path::Path;

fn office_app(prog_id: &str) -> Result<Disp> {
    use windows::Win32::System::Com::{CLSIDFromProgID, CoCreateInstance, IDispatch, CLSCTX_LOCAL_SERVER};
    let clsid = unsafe { CLSIDFromProgID(&windows::core::HSTRING::from(prog_id)) }.map_err(|_| anyhow!("{}", crate::i18n::fmt("{0} is not installed on this computer", &[prog_id.split('.').next().unwrap_or(prog_id).to_string()])))?;
    let app: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER) }.with_context(|| crate::i18n::fmt("cannot start {0}", &[prog_id.to_string()]))?;
    Ok(Disp(app))
}

/// Runs the work in a thread of its own with COM initialised (the caller's thread may belong to EA's session).
fn in_com_thread<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    std::thread::spawn(move || {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let result = work();
        unsafe { CoUninitialize() };
        result
    })
    .join()
    .map_err(|_| anyhow!("the Office conversion stopped unexpectedly"))?
}

/// DOCX → PDF with Word (read-only, invisible).
pub fn docx_to_pdf(docx: &Path, pdf: &Path) -> Result<()> {
    let (docx, pdf) = (docx.display().to_string(), pdf.display().to_string());
    in_com_thread(move || {
        let word = office_app("Word.Application")?;
        let _ = word.set("DisplayAlerts", i(0));
        let result = (|| -> Result<()> {
            let documents = word.obj("Documents")?;
            // Open(FileName, ConfirmConversions, ReadOnly)
            let document = documents.call_obj("Open", &[s(&docx), b(false), b(true)])?;
            // SaveAs2(FileName, FileFormat = 17: PDF)
            let saved = document.call("SaveAs2", &[s(&pdf), i(17)]);
            let _ = document.call("Close", &[b(false)]);
            saved.map(|_| ())
        })();
        // Word left open when the user has documents in it (an instance shared with the user's Word).
        if word.obj("Documents").and_then(|d| d.int("Count")).unwrap_or(1) == 0 {
            let _ = word.call("Quit", &[]);
        }
        result
    })
}

/// PPTX → PDF with PowerPoint (without a window).
pub fn pptx_to_pdf(pptx: &Path, pdf: &Path) -> Result<()> {
    let (pptx, pdf) = (pptx.display().to_string(), pdf.display().to_string());
    in_com_thread(move || {
        let powerpoint = office_app("PowerPoint.Application")?;
        let result = (|| -> Result<()> {
            let presentations = powerpoint.obj("Presentations")?;
            // Open(FileName, ReadOnly = msoTrue (-1), Untitled = msoFalse, WithWindow = msoFalse)
            let presentation = presentations.call_obj("Open", &[s(&pptx), i(-1), i(0), i(0)])?;
            // SaveAs(FileName, ppSaveAsPDF = 32)
            let saved = presentation.call("SaveAs", &[s(&pdf), i(32)]);
            let _ = presentation.call("Close", &[]);
            saved.map(|_| ())
        })();
        // PowerPoint runs as one instance: when the user has presentations open, it is theirs and stays open (I17).
        if powerpoint.obj("Presentations").and_then(|p| p.int("Count")).unwrap_or(1) == 0 {
            let _ = powerpoint.call("Quit", &[]);
        }
        result
    })
}

/// Pictures of slides of a PPTX with PowerPoint (without a window): `slides` numbered from 1, each saved as
/// `<dir>/<stem>-<n>.png`, 1600 px wide (LC-484: the assistant had only the text of the slides).
pub fn pptx_slides_png(pptx: &Path, slides: &[i64], dir: &Path, stem: &str) -> Result<Vec<(i64, std::path::PathBuf)>> {
    let (pptx, slides, dir, stem) = (pptx.display().to_string(), slides.to_vec(), dir.to_path_buf(), stem.to_string());
    in_com_thread(move || {
        let powerpoint = office_app("PowerPoint.Application")?;
        let result = (|| -> Result<Vec<(i64, std::path::PathBuf)>> {
            let presentation = powerpoint.obj("Presentations")?.call_obj("Open", &[s(&pptx), i(-1), i(0), i(0)])?;
            let all = presentation.obj("Slides")?;
            let count = all.int("Count")? as i64;
            let mut out = Vec::new();
            for n in slides.iter().copied().filter(|n| *n >= 1 && *n <= count) {
                let path = dir.join(format!("{stem}-{n}.png"));
                // Export(FileName, FilterName, ScaleWidth, ScaleHeight)
                let slide = all.call_obj("Item", &[i(n as i32)])?;
                slide.call("Export", &[s(&path.display().to_string()), s("PNG"), i(1600), i(900)])?;
                if path.is_file() {
                    out.push((n, path));
                }
            }
            let _ = presentation.call("Close", &[]);
            Ok(out)
        })();
        // PowerPoint runs as one instance: when the user has presentations open, it is theirs and stays open (I17).
        if powerpoint.obj("Presentations").and_then(|p| p.int("Count")).unwrap_or(1) == 0 {
            let _ = powerpoint.call("Quit", &[]);
        }
        result
    })
}

#[cfg(test)]
mod tests {
    /// I17: converting with PowerPoint while the user has a presentation open leaves their PowerPoint open.
    /// Start PowerPoint with a presentation first, then `LOONAR_TEST_PPTX=… cargo test office_keeps_users_powerpoint -- --ignored`.
    #[test]
    #[ignore = "requires PowerPoint with an open presentation"]
    fn office_keeps_users_powerpoint() {
        let pptx = std::path::PathBuf::from(std::env::var("LOONAR_TEST_PPTX").unwrap());
        let pdf = std::env::temp_dir().join(format!("loonar-i17-{}.pdf", std::process::id()));
        super::pptx_to_pdf(&pptx, &pdf).unwrap();
        assert!(std::fs::metadata(&pdf).unwrap().len() > 1000);
        let _ = std::fs::remove_file(&pdf);
    }

    /// LC-484: PowerPoint draws chosen slides as PNG. `LOONAR_TEST_PPTX=… cargo test slides_as_png -- --ignored --nocapture`
    #[test]
    #[ignore = "requires PowerPoint and a presentation"]
    fn slides_as_png() {
        let pptx = std::path::PathBuf::from(std::env::var("LOONAR_TEST_PPTX").unwrap());
        let dir = std::env::temp_dir().join(format!("loonar-lc484-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let drawn = super::pptx_slides_png(&pptx, &[1, 2, 999], &dir, "deck").unwrap();
        println!("{drawn:?}");
        assert!(!drawn.is_empty() && drawn.iter().all(|(n, p)| *n != 999 && std::fs::metadata(p).unwrap().len() > 1000));
    }
}
