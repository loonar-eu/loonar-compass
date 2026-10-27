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
        let _ = word.call("Quit", &[]);
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
        let _ = powerpoint.call("Quit", &[]);
        result
    })
}
