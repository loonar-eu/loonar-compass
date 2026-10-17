//! Bug reports and newsletter sign-ups: a ready-made message in the user's mail program.
//!
//! First via Simple MAPI (MAPISendMailW): Windows opens a new-message window in the default mail
//! program with the address, subject, body and attached images. When the mail program does not support MAPI,
//! a mailto link is used instead (without attachments) and the folder with the images is opened in Explorer for attaching them manually.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

/// Recipient of the reports.
pub const TO: &str = "mariusz@loonar.eu";
/// Total size limit for the images in a report (the typical e-mail attachment limit is 20-25 MB).
pub const MAX_ATTACHMENTS_BYTES: usize = 15 * 1024 * 1024;

/// Image attached to a report: file name and content.
#[derive(Clone)]
pub struct File {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// How the message reached the mail program.
pub enum Outcome {
    /// The message window (with attachments) was shown and closed: sent or saved by the user.
    Composed,
    /// The user closed the message window without sending.
    Cancelled,
    /// Mail program without MAPI: mailto was opened, and the images wait in a folder to be attached manually.
    Fallback { folder: Option<PathBuf> },
}

/// Prepares a message to the author in the mail program. Called from a background thread: with MAPI it waits
/// until the user sends or closes the message window.
pub fn compose(subject: &str, body: &str, files: &[File]) -> Result<Outcome> {
    let total: usize = files.iter().map(|f| f.bytes.len()).sum();
    if total > MAX_ATTACHMENTS_BYTES {
        bail!("{}", crate::i18n::fmt("the images total {0} MB, but the limit is {1} MB", &[(total / 1_048_576).to_string(), (MAX_ATTACHMENTS_BYTES / 1_048_576).to_string()]));
    }
    let folder = if files.is_empty() { None } else { Some(save_files(files)?) };
    let paths: Vec<PathBuf> = folder.as_ref().map(|dir| files.iter().map(|f| dir.join(&f.name)).collect()).unwrap_or_default();
    // The mail program has taken the images (or the user cancelled): the temporary copies are no longer needed.
    let remove_copies = || {
        if let Some(dir) = &folder {
            let _ = std::fs::remove_dir_all(dir);
        }
    };
    match mapi_send(subject, body, &paths) {
        Ok(true) => {
            remove_copies();
            Ok(Outcome::Composed)
        }
        Ok(false) => {
            remove_copies();
            Ok(Outcome::Cancelled)
        }
        Err(_) => {
            open_mailto(subject, body)?;
            if let Some(dir) = &folder {
                let _ = std::process::Command::new(crate::config::system_program("explorer.exe")).arg(dir).spawn();
            }
            Ok(Outcome::Fallback { folder })
        }
    }
}

/// Saves the images to a separate temporary folder (the mail program attaches files from disk).
fn save_files(files: &[File]) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(crate::i18n::fmt("Loonar bug report {0}", &[chrono::Local::now().format("%Y-%m-%d %H%M%S").to_string()]));
    std::fs::create_dir_all(&dir).with_context(|| crate::i18n::fmt("folder {0}", &[dir.display().to_string()]))?;
    for (i, f) in files.iter().enumerate() {
        let mut name = f.name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_");
        if files[..i].iter().any(|g| g.name == f.name) {
            name = format!("{}_{name}", i + 1);
        }
        std::fs::write(dir.join(&name), &f.bytes).with_context(|| crate::i18n::fmt("saving {0}", &[name.clone()]))?;
    }
    Ok(dir)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[repr(C)]
struct MapiRecipDescW {
    reserved: u32,
    recip_class: u32,
    name: *const u16,
    address: *const u16,
    eid_size: u32,
    entry_id: *mut std::ffi::c_void,
}

#[repr(C)]
struct MapiFileDescW {
    reserved: u32,
    flags: u32,
    position: u32,
    path_name: *const u16,
    file_name: *const u16,
    file_type: *mut std::ffi::c_void,
}

#[repr(C)]
struct MapiMessageW {
    reserved: u32,
    subject: *const u16,
    note_text: *const u16,
    message_type: *const u16,
    date_received: *const u16,
    conversation_id: *const u16,
    flags: u32,
    originator: *mut MapiRecipDescW,
    recip_count: u32,
    recips: *mut MapiRecipDescW,
    file_count: u32,
    files: *mut MapiFileDescW,
}

type MapiSendMailW = unsafe extern "system" fn(usize, usize, *mut MapiMessageW, u32, u32) -> u32;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *const std::ffi::c_void;
}

/// New-message window in the default mail program (Simple MAPI). `Ok(false)` = the user cancelled.
fn mapi_send(subject: &str, body: &str, attachments: &[PathBuf]) -> Result<bool> {
    const MAPI_TO: u32 = 1;
    const MAPI_LOGON_UI: u32 = 0x1;
    const MAPI_DIALOG: u32 = 0x8;
    const MAPI_USER_ABORT: u32 = 1;
    // Only from System32: a mapi32.dll placed in the program's or the current folder must never be loaded.
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0800;
    // SAFETY: loading a system library and getting the address of a function with a known signature (Simple MAPI).
    let send: MapiSendMailW = unsafe {
        let module = LoadLibraryExW(wide("mapi32.dll").as_ptr(), 0, LOAD_LIBRARY_SEARCH_SYSTEM32);
        if module == 0 {
            bail!("mapi32.dll not found");
        }
        let proc = GetProcAddress(module, b"MAPISendMailW\0".as_ptr());
        if proc.is_null() {
            bail!("MAPISendMailW not found");
        }
        std::mem::transmute::<*const std::ffi::c_void, MapiSendMailW>(proc)
    };
    let (subject_w, body_w, to_w, to_name_w) = (wide(subject), wide(body), wide(&format!("SMTP:{TO}")), wide(TO));
    let mut recip = MapiRecipDescW {
        reserved: 0,
        recip_class: MAPI_TO,
        name: to_name_w.as_ptr(),
        address: to_w.as_ptr(),
        eid_size: 0,
        entry_id: std::ptr::null_mut(),
    };
    let paths_w: Vec<(Vec<u16>, Vec<u16>)> = attachments
        .iter()
        .map(|p| (wide(&p.to_string_lossy()), wide(&p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())))
        .collect();
    let mut files: Vec<MapiFileDescW> = paths_w
        .iter()
        .map(|(path, name)| MapiFileDescW {
            reserved: 0,
            flags: 0,
            position: u32::MAX,
            path_name: path.as_ptr(),
            file_name: name.as_ptr(),
            file_type: std::ptr::null_mut(),
        })
        .collect();
    let mut message = MapiMessageW {
        reserved: 0,
        subject: subject_w.as_ptr(),
        note_text: body_w.as_ptr(),
        message_type: std::ptr::null(),
        date_received: std::ptr::null(),
        conversation_id: std::ptr::null(),
        flags: 0,
        originator: std::ptr::null_mut(),
        recip_count: 1,
        recips: &mut recip,
        file_count: files.len() as u32,
        files: if files.is_empty() { std::ptr::null_mut() } else { files.as_mut_ptr() },
    };
    // SAFETY: all pointers point to buffers that live until the end of the call.
    let code = unsafe { send(0, 0, &mut message, MAPI_DIALOG | MAPI_LOGON_UI, 0) };
    match code {
        0 => Ok(true),
        MAPI_USER_ABORT => Ok(false),
        other => bail!("MAPISendMail returned code {other}"),
    }
}

/// mailto link in the default mail program (without attachments).
fn open_mailto(subject: &str, body: &str) -> Result<()> {
    let encode = |text: &str| -> String {
        text.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let url = format!("mailto:{TO}?subject={}&body={}", encode(subject), encode(body));
    std::process::Command::new(crate::config::system_program("rundll32.exe")).arg("url.dll,FileProtocolHandler").arg(&url).spawn().context(crate::i18n::tr("mail program"))?;
    Ok(())
}

