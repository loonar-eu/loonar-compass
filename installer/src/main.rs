//! Loonar Compass setup: a classic wizard (language, welcome, what's new, license, folder, configuration,
//! summary, installation, finish) and the uninstaller (the same program started with `--uninstall`).
//!
//! By default the application is installed just for the current user, without administrator rights:
//! `%LOCALAPPDATA%\Programs\Loonar Compass`, the user's Start menu and desktop, and an entry in the user's part
//! of the registry. An installation for all users goes to Program Files: for that part only (program files,
//! the machine-wide registry entry, the common Start menu and desktop) the wizard starts a second copy of itself
//! with administrator rights (`--machine-part <plan file>`), shows its progress and reads its result.
//! Settings, conversations and secrets always belong to the logged-in user: the wizard itself, never the
//! elevated copy, writes them to `%APPDATA%\Loonar Compass` and to Windows Credential Manager.
//!
//! The uninstaller is a byte-identical copy of the setup program (`uninstall.exe` in the program folder), so a
//! code signature of the setup also covers the uninstaller.

#![windows_subsystem = "windows"]

#[allow(dead_code)]
#[path = "../../src/config.rs"]
mod config;
#[allow(dead_code)]
#[path = "../../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../../src/i18n.rs"]
mod i18n;

use anyhow::{anyhow, bail, Context, Result};
use config::secrets;
use eframe::egui;
use i18n::Lang;
use std::ffi::OsString;
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

const APP_NAME: &str = config::APP_NAME;
const PUBLISHER: &str = "Loonar Mariusz Głowacki";
const APP_EXE: &str = "LoonarCompass.exe";
const UNINSTALL_EXE: &str = "uninstall.exe";
/// The Enterprise Architect add-in (a tab and a ribbon button of EA that host Compass).
const ADDIN_DLL: &str = "LoonarCompass.EAAddin.dll";
const LICENSE_FILE: &str = "LICENSE.txt";
/// The Polish license is the binding text; it is installed next to the translation in the chosen language.
const LICENSE_PL_FILE: &str = "LICENSE.pl.txt";
const CHANGELOG_FILE: &str = "CHANGELOG.md";
const NOTICES_FILE: &str = "THIRD-PARTY-NOTICES.txt";
const URL_INFO: &str = "https://loonar.eu";
const URL_HELP: &str = "https://loonar.eu/#faq";
const DEFAULT_EA_PATH: &str = r"C:\Program Files\Sparx Systems\EA\EA.exe";
const CRASH_LOG: &str = "loonar-compass-setup-crash.log";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const UNINSTALL_ARG: &str = "--uninstall";
/// With `--uninstall`: no window (the "QuietUninstallString" of the "Apps" entry).
const QUIET_ARG: &str = "--quiet";
/// Private argument of the elevated copy, followed by the path of the plan file.
const MACHINE_PART_ARG: &str = "--machine-part";

/// A product this setup knows: the current one and the pre-release name (versions 0.1–0.3).
struct Product {
    name: &'static str,
    exe: &'static str,
    uninstall_key: &'static str,
}

static CURRENT: Product = Product {
    name: APP_NAME,
    exe: APP_EXE,
    uninstall_key: r"Software\Microsoft\Windows\CurrentVersion\Uninstall\LoonarCompass",
};

/// "Loonar EA Assistant" 0.1–0.3: `%ProgramFiles%\Loonar EA Assistant` with an entry in HKLM (test builds of the
/// installer without administrator rights: the user's programs folder with an entry in HKCU).
static LEGACY: Product = Product {
    name: "Loonar EA Assistant",
    exe: "loonar-ea-assistant.exe",
    uninstall_key: r"Software\Microsoft\Windows\CurrentVersion\Uninstall\LoonarEaAssistant",
};

/// Version shown to the user and in "Apps": major.minor, plus .patch when it is not 0 ("0.3", "0.3.1"),
/// plus a pre-release suffix if there is one ("1.0-beta.1"). build_installer.ps1 names the setup file the same way.
fn display_version(full: &str) -> String {
    let split = full.find(['-', '+']).unwrap_or(full.len());
    let (core, suffix) = full.split_at(split);
    let short = match core.split('.').collect::<Vec<_>>().as_slice() {
        [major, minor, patch] if patch.trim() == "0" => format!("{major}.{minor}"),
        _ => core.to_string(),
    };
    format!("{short}{suffix}")
}

/// This setup's version (from Cargo.toml).
fn app_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| display_version(env!("CARGO_PKG_VERSION")))
}

// Installer contents, prepared by build_installer.ps1 (installer\build.rs checks that they exist).
const PAYLOAD_APP: &[u8] = include_bytes!("../payload/app.exe");
const PAYLOAD_ADDIN: &[u8] = include_bytes!("../payload/LoonarCompass.EAAddin.dll");
const PAYLOAD_NOTICES: &[u8] = include_bytes!("../payload/THIRD-PARTY-NOTICES.txt");

/// License and changelog in four languages (the user guide is on the website).
fn payload_license(lang: Lang) -> &'static [u8] {
    match lang {
        Lang::Pl => include_bytes!("../payload/LICENSE.pl.txt"),
        Lang::En => include_bytes!("../payload/LICENSE.en.txt"),
        Lang::De => include_bytes!("../payload/LICENSE.de.txt"),
        Lang::Fr => include_bytes!("../payload/LICENSE.fr.txt"),
    }
}

fn payload_changelog(lang: Lang) -> &'static [u8] {
    match lang {
        Lang::Pl => include_bytes!("../payload/CHANGELOG.pl.md"),
        Lang::En => include_bytes!("../payload/CHANGELOG.en.md"),
        Lang::De => include_bytes!("../payload/CHANGELOG.de.md"),
        Lang::Fr => include_bytes!("../payload/CHANGELOG.fr.md"),
    }
}

/// The user guide of this version on the website, in the installation language.
fn docs_url(lang: Lang) -> String {
    format!("https://loonar.eu/docs/{}/{}/", display_version(app_version()), lang.code())
}

/// File name of the user guide PDF that versions before 0.5 put into the program folder (removed on update).
fn manual_file(lang: Lang) -> String {
    format!("{}.pdf", i18n::tr_in(lang, "User guide"))
}

/// Every file the setup puts into the program folder (the application first: if it is running, an uninstallation
/// stops before touching anything else).
fn program_file_names() -> Vec<String> {
    [APP_EXE, ADDIN_DLL, LICENSE_FILE, LICENSE_PL_FILE, CHANGELOG_FILE, NOTICES_FILE, UNINSTALL_EXE]
        .into_iter()
        .map(String::from)
        .chain(Lang::ALL.into_iter().map(manual_file))
        .collect()
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_string()
}

const ICON_RGBA: &[u8] = include_bytes!("../../assets/icon_rgba.bin");
const BANNER_BG: &[u8] = include_bytes!("../../assets/splash_bg.png");
const BANNER_STARS: &[u8] = include_bytes!("../../assets/stars.png");

mod theme {
    use eframe::egui::Color32;
    pub const BACKGROUND: Color32 = Color32::from_rgb(255, 255, 255);
    pub const PANEL: Color32 = Color32::from_rgb(246, 245, 241);
    pub const TEXT: Color32 = Color32::from_rgb(31, 30, 27);
    pub const MUTED: Color32 = Color32::from_rgb(112, 109, 100);
    pub const BORDER: Color32 = Color32::from_rgb(220, 217, 208);
    pub const BUTTON: Color32 = Color32::from_rgb(240, 238, 232);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgb(228, 225, 216);
    pub const ACCENT: Color32 = Color32::from_rgb(13, 53, 92);
    pub const ERROR: Color32 = Color32::from_rgb(176, 48, 40);
    pub const SUCCESS: Color32 = Color32::from_rgb(30, 122, 62);
}

// ---------------------------------------------------------------------------------------------------------------
// Progress

#[derive(Default)]
struct ProgressState {
    value: f32,
    step: String,
    /// Value last written to the progress file (the elevated copy writes it only on visible changes).
    written: f32,
}

/// Installation or uninstallation progress, shared by the worker thread and the window: fraction done (0..1) and
/// the current step. A view made with `part` reports into a sub-range of the whole bar.
#[derive(Clone, Default)]
struct Progress {
    state: Arc<Mutex<ProgressState>>,
    range: Option<(f32, f32)>,
    /// The elevated copy: every visible change is also written to this file, which the wizard reads.
    sink: Option<Arc<Mutex<std::fs::File>>>,
}

impl Progress {
    fn to_file(file: std::fs::File) -> Self {
        Self { sink: Some(Arc::new(Mutex::new(file))), ..Default::default() }
    }

    fn part(&self, from: f32, to: f32) -> Self {
        let (a, b) = self.range.unwrap_or((0.0, 1.0));
        Self { range: Some((a + (b - a) * from, a + (b - a) * to)), ..self.clone() }
    }

    fn set(&self, done: f32, step: &str) {
        let (a, b) = self.range.unwrap_or((0.0, 1.0));
        let value = a + (b - a) * done.clamp(0.0, 1.0);
        let Ok(mut state) = self.state.lock() else { return };
        let visible = (value - state.written).abs() >= 0.005 || state.step != step || value >= 1.0;
        state.value = value;
        state.step = step.to_string();
        if visible {
            state.written = value;
            if let Some(sink) = &self.sink {
                if let Ok(mut file) = sink.lock() {
                    use std::io::{Seek, Write};
                    let text = format!("{value:.4}\t{step}");
                    let _ = file.seek(std::io::SeekFrom::Start(0)).and_then(|_| file.write_all(text.as_bytes()));
                    let _ = file.set_len(text.len() as u64);
                }
            }
        }
    }

    fn get(&self) -> (f32, String) {
        self.state.lock().map(|s| (s.value, s.step.clone())).unwrap_or_default()
    }
}

/// Progress file of the elevated copy: "fraction<TAB>step".
fn parse_progress(text: &str) -> Option<(f32, String)> {
    let (value, step) = text.split_once('\t')?;
    Some((value.trim().parse().ok()?, step.to_string()))
}

/// Writes a file in chunks, with progress from `from` to `to`.
fn write_with_progress(path: &Path, bytes: &[u8], progress: &Progress, from: f32, to: f32, step: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    let chunk = 512 * 1024;
    for (i, part) in bytes.chunks(chunk).enumerate() {
        file.write_all(part)?;
        let written = ((i + 1) * chunk).min(bytes.len()) as f32 / bytes.len().max(1) as f32;
        progress.set(from + (to - from) * written, step);
    }
    file.sync_all()
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).with_context(|| crate::i18n::fmt("saving {0}", &[path.display().to_string()]))
}

// ---------------------------------------------------------------------------------------------------------------
// System folders, paths and the registry

/// A known Windows folder (Start menu, desktop, Program Files…), read with the shell API: in Unicode and
/// without starting any helper program.
fn known_folder(id: &windows::core::GUID) -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    // SAFETY: the returned string is copied and then freed with CoTaskMemFree, as the API requires.
    let path = unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, windows::Win32::Foundation::HANDLE::default())
            .with_context(|| crate::i18n::tr("cannot read the system folder"))?;
        let path = PathBuf::from(OsString::from_wide(raw.as_wide()));
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        path
    };
    if path.as_os_str().is_empty() {
        bail!("{}", crate::i18n::fmt("the system did not provide the folder {0}", &[format!("{id:?}")]));
    }
    Ok(path)
}

fn cached_folder(cell: &'static OnceLock<PathBuf>, id: &windows::core::GUID, fallback: impl FnOnce() -> PathBuf) -> PathBuf {
    cell.get_or_init(|| known_folder(id).unwrap_or_else(|_| fallback())).clone()
}

fn env_dir(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(default))
}

/// `C:\Program Files` (the 64-bit one: the application is a 64-bit program).
fn program_files() -> PathBuf {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    cached_folder(&CELL, &windows::Win32::UI::Shell::FOLDERID_ProgramFiles, || env_dir("ProgramFiles", r"C:\Program Files"))
}

fn program_files_x86() -> PathBuf {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    cached_folder(&CELL, &windows::Win32::UI::Shell::FOLDERID_ProgramFilesX86, || env_dir("ProgramFiles(x86)", r"C:\Program Files (x86)"))
}

fn windows_dir() -> PathBuf {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    cached_folder(&CELL, &windows::Win32::UI::Shell::FOLDERID_Windows, || env_dir("SystemRoot", r"C:\Windows"))
}

/// `C:\Windows\System32`: the only place helper programs are started from (always by full path).
fn system_dir() -> PathBuf {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    cached_folder(&CELL, &windows::Win32::UI::Shell::FOLDERID_System, || windows_dir().join("System32"))
}

/// `%LOCALAPPDATA%\Programs`: programs installed just for the current user.
fn user_programs_dir() -> PathBuf {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    cached_folder(&CELL, &windows::Win32::UI::Shell::FOLDERID_UserProgramFiles, || {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join("Programs")
    })
}

/// Who the installation is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scope {
    /// The current user: no administrator rights, the user's folders and registry hive.
    User,
    /// All users of the computer: Program Files, the common Start menu and desktop, HKLM.
    Machine,
}

impl Scope {
    fn code(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Machine => "machine",
        }
    }

    fn root(self) -> reg::Root {
        match self {
            Scope::User => reg::Root::CurrentUser,
            Scope::Machine => reg::Root::LocalMachine,
        }
    }

    fn default_dir(self) -> PathBuf {
        match self {
            Scope::User => user_programs_dir().join(APP_NAME),
            Scope::Machine => program_files().join(APP_NAME),
        }
    }

    /// The application's folder in the Start menu (the user's or the common one).
    fn start_menu(self) -> Result<PathBuf> {
        use windows::Win32::UI::Shell::{FOLDERID_CommonPrograms, FOLDERID_Programs};
        Ok(known_folder(if self == Scope::Machine { &FOLDERID_CommonPrograms } else { &FOLDERID_Programs })?.join(APP_NAME))
    }

    fn desktop(self) -> Result<PathBuf> {
        use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_PublicDesktop};
        known_folder(if self == Scope::Machine { &FOLDERID_PublicDesktop } else { &FOLDERID_Desktop })
    }

    fn label(self) -> &'static str {
        match self {
            Scope::User => crate::i18n::tr("Just for me"),
            Scope::Machine => crate::i18n::tr("For all users of this computer"),
        }
    }
}

/// A path split into lowercase parts for comparisons (Windows paths are case-insensitive; trailing spaces and dots
/// of a name are ignored by Windows too). `None` for a path that is not a full path or contains "." or "..".
fn path_parts(path: &Path) -> Option<Vec<String>> {
    use std::path::Prefix;
    let mut parts = Vec::new();
    let (mut prefix, mut root) = (false, false);
    for component in path.components() {
        match component {
            Component::Prefix(p) => {
                prefix = true;
                parts.push(match p.kind() {
                    Prefix::Disk(d) | Prefix::VerbatimDisk(d) => format!("{}:", (d as char).to_ascii_lowercase()),
                    Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                        format!(r"\\{}\{}", server.to_string_lossy().to_lowercase(), share.to_string_lossy().to_lowercase())
                    }
                    _ => return None,
                });
            }
            Component::RootDir => root = true,
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                let name = name.trim_end_matches([' ', '.']);
                if name.is_empty() {
                    return None;
                }
                parts.push(name.to_lowercase());
            }
            Component::CurDir | Component::ParentDir => return None,
        }
    }
    (prefix && root).then_some(parts)
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (path_parts(a), path_parts(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase(),
    }
}

/// Whether `path` is `base` or a folder inside it.
fn within(path: &[String], base: &Path) -> bool {
    path_parts(base).is_some_and(|base| path.starts_with(&base))
}

/// Checks the installation folder for the chosen installation type; the error is a message for the user.
/// For all users the folder must be inside Program Files (only administrators can change the files there, which
/// keeps the program that every user starts safe); just for the current user it may be anywhere the user can
/// write, except the folders reserved for administrators.
fn check_folder(text: &str, scope: Scope) -> Result<PathBuf, String> {
    let path = PathBuf::from(text.trim());
    let example = scope.default_dir().display().to_string();
    let invalid_name = path.components().any(|c| matches!(c, Component::Normal(n) if n.to_string_lossy().contains(['<', '>', ':', '"', '|', '?', '*'])));
    let parts = match path_parts(&path) {
        // At least one folder below the drive or network share.
        Some(parts) if parts.len() >= 2 && !invalid_name => parts,
        _ => return Err(crate::i18n::fmt("Enter the full path of the installation folder, for example {0}.", &[example])),
    };
    match scope {
        Scope::Machine if !(within(&parts, &program_files()) && parts.len() > path_parts(&program_files()).map_or(0, |p| p.len())) => {
            Err(crate::i18n::fmt("An installation for all users must be in the Program Files folder ({0}), where only administrators can change files. Choose a folder there or choose \"Just for me\".", &[program_files().display().to_string()]))
        }
        Scope::User if within(&parts, &program_files()) || within(&parts, &program_files_x86()) || within(&parts, &windows_dir()) => {
            Err(crate::i18n::fmt("This folder can only be changed with administrator rights. Choose \"For all users of this computer\", or a folder you can write to, for example {0}.", &[example]))
        }
        _ => Ok(path),
    }
}

/// Registry access with the Win32 API (Unicode, no `reg.exe`). The program is 64-bit, so it sees the
/// registry view that "Apps" and "Programs and Features" use.
mod reg {
    use anyhow::Result;
    use windows::core::{HRESULT, HSTRING};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegDeleteKeyW, RegDeleteTreeW, RegGetValueW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_DWORD, REG_SZ, RRF_RT_REG_SZ,
    };

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Root {
        CurrentUser,
        LocalMachine,
    }

    fn hkey(root: Root) -> HKEY {
        match root {
            Root::CurrentUser => HKEY_CURRENT_USER,
            Root::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }

    fn check(rc: WIN32_ERROR, what: &str) -> Result<()> {
        if rc == ERROR_SUCCESS {
            return Ok(());
        }
        Err(anyhow::anyhow!("{what}: {}", windows::core::Error::from(HRESULT::from_win32(rc.0)).message()))
    }

    /// Text value; `None` when the key or the value does not exist.
    pub fn get_string(root: Root, subkey: &str, name: &str) -> Option<String> {
        let (key, value) = (HSTRING::from(subkey), HSTRING::from(name));
        for _ in 0..3 {
            let mut size = 0u32;
            // SAFETY: size query without a buffer; the strings live until the call returns.
            let rc = unsafe { RegGetValueW(hkey(root), &key, &value, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
            if rc != ERROR_SUCCESS {
                return None;
            }
            let mut buffer = vec![0u16; size as usize / 2 + 1];
            let mut bytes = (buffer.len() * 2) as u32;
            // SAFETY: the buffer holds `bytes` bytes and lives until the call returns.
            let rc = unsafe {
                RegGetValueW(hkey(root), &key, &value, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast()), Some(&mut bytes))
            };
            if rc == ERROR_MORE_DATA {
                continue;
            }
            if rc != ERROR_SUCCESS {
                return None;
            }
            let len = (bytes as usize / 2).min(buffer.len());
            return Some(String::from_utf16_lossy(&buffer[..len]).trim_end_matches('\0').to_string());
        }
        None
    }

    pub fn set_string(root: Root, subkey: &str, name: &str, value: &str) -> Result<()> {
        let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: zero-terminated UTF-16 data of the given size; creates the key when it does not exist.
        let rc = unsafe {
            RegSetKeyValueW(hkey(root), &HSTRING::from(subkey), &HSTRING::from(name), REG_SZ.0, Some(data.as_ptr().cast()), (data.len() * 2) as u32)
        };
        check(rc, name)
    }

    pub fn set_dword(root: Root, subkey: &str, name: &str, value: u32) -> Result<()> {
        // SAFETY: a 4-byte value that lives until the call returns.
        let rc = unsafe {
            RegSetKeyValueW(hkey(root), &HSTRING::from(subkey), &HSTRING::from(name), REG_DWORD.0, Some((&value as *const u32).cast()), 4)
        };
        check(rc, name)
    }

    /// Deletes the key with all its values and subkeys; a missing key is not an error.
    pub fn delete_key(root: Root, subkey: &str) -> Result<()> {
        let key = HSTRING::from(subkey);
        // SAFETY: plain calls with a zero-terminated key name.
        let rc = unsafe { RegDeleteTreeW(hkey(root), &key) };
        if rc != ERROR_SUCCESS && rc != ERROR_FILE_NOT_FOUND {
            return check(rc, subkey);
        }
        let rc = unsafe { RegDeleteKeyW(hkey(root), &key) };
        if rc != ERROR_SUCCESS && rc != ERROR_FILE_NOT_FOUND {
            return check(rc, subkey);
        }
        Ok(())
    }
}

/// Registration of the EA add-in for the current user: COM classes in HKCU\Software\Classes (64- and 32-bit
/// views) and the entry EA reads at start. EA finds add-ins in the registry, so the folder EA is installed in
/// does not matter (also for a trial).
mod ea_addin {
    use super::reg::{self, Root};
    use anyhow::Result;
    use std::path::Path;

    const ASSEMBLY: &str = "LoonarCompass.EAAddin, Version=1.0.0.0, Culture=neutral, PublicKeyToken=null";
    const CLASSES: [(&str, &str, &str); 2] = [
        ("LoonarCompass.Addin", "LoonarCompass.Addin", "{6C1E5B7A-3D2F-4B8E-9A41-2F7C0D5E8B13}"),
        ("LoonarCompass.Tab", "LoonarCompass.CompassTab", "{9B4D2E61-7A3C-4F15-8E2B-5D0C6A1F9E47}"),
    ];
    const ADDINS: [&str; 2] = [r"Software\Sparx Systems\EAAddins64\LoonarCompass", r"Software\Sparx Systems\EAAddins\LoonarCompass"];

    pub fn register(dll: &Path) -> Result<()> {
        let code_base = format!("file:///{}", dll.display().to_string().replace('\\', "/").replace(' ', "%20"));
        let root = Root::CurrentUser;
        for (prog_id, class, clsid) in CLASSES {
            reg::set_string(root, &format!(r"Software\Classes\{prog_id}"), "", class)?;
            reg::set_string(root, &format!(r"Software\Classes\{prog_id}\CLSID"), "", clsid)?;
            for view in [r"Software\Classes\CLSID", r"Software\Classes\Wow6432Node\CLSID"] {
                let key = format!(r"{view}\{clsid}");
                reg::set_string(root, &key, "", class)?;
                reg::set_string(root, &format!(r"{key}\ProgId"), "", prog_id)?;
                let server = format!(r"{key}\InprocServer32");
                reg::set_string(root, &server, "", "mscoree.dll")?;
                reg::set_string(root, &server, "ThreadingModel", "Both")?;
                reg::set_string(root, &server, "Class", class)?;
                reg::set_string(root, &server, "Assembly", ASSEMBLY)?;
                reg::set_string(root, &server, "RuntimeVersion", "v4.0.30319")?;
                reg::set_string(root, &server, "CodeBase", &code_base)?;
                reg::set_string(root, &format!(r"{key}\Implemented Categories\{{62C8FE65-4EBB-45e7-B440-6E39B2CDBF29}}"), "", "")?;
            }
        }
        for key in ADDINS {
            reg::set_string(root, key, "", "LoonarCompass.Addin")?;
        }
        Ok(())
    }

    pub fn unregister() {
        let root = Root::CurrentUser;
        for (prog_id, _, clsid) in CLASSES {
            let _ = reg::delete_key(root, &format!(r"Software\Classes\{prog_id}"));
            let _ = reg::delete_key(root, &format!(r"Software\Classes\CLSID\{clsid}"));
            let _ = reg::delete_key(root, &format!(r"Software\Classes\Wow6432Node\CLSID\{clsid}"));
        }
        for key in ADDINS {
            let _ = reg::delete_key(root, key);
        }
    }
}

/// Whether this process runs with administrator rights (elevated token).
fn elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        // SAFETY: the token handle is closed after use; the output buffer has the size passed in.
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut length = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut length,
            )
            .is_ok();
            let _ = CloseHandle(token);
            ok && elevation.TokenIsElevated != 0
        }
    })
}

/// An installation found in the registry.
#[derive(Clone, Debug, PartialEq)]
struct Installed {
    scope: Scope,
    dir: PathBuf,
    version: Option<String>,
}

/// Program path from an "UninstallString" (`"C:\…\uninstall.exe" --uninstall` or without quotes).
fn uninstaller_from_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let exe = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => &command[..command.to_ascii_lowercase().find(".exe")? + 4],
    };
    let path = PathBuf::from(exe);
    path.is_absolute().then_some(path)
}

/// The product's installation of the given type, from its "Apps" entry.
fn read_install(product: &Product, scope: Scope) -> Option<Installed> {
    let root = scope.root();
    let location = reg::get_string(root, product.uninstall_key, "InstallLocation").filter(|l| !l.trim().is_empty()).map(PathBuf::from);
    let uninstaller = reg::get_string(root, product.uninstall_key, "UninstallString");
    // Version 0.1 did not save the folder: it is then the folder of the uninstaller.
    let dir = location.or_else(|| uninstaller.as_deref().and_then(uninstaller_from_command).and_then(|p| p.parent().map(Path::to_path_buf)))?;
    Some(Installed { scope, dir, version: reg::get_string(root, product.uninstall_key, "DisplayVersion").filter(|v| !v.trim().is_empty()) })
}

/// Whether this program is the uninstaller in `dir` (it cannot delete itself while it runs).
fn running_from(dir: &Path) -> bool {
    std::env::current_exe().is_ok_and(|me| same_path(&me, &dir.join(UNINSTALL_EXE)))
}

/// What the uninstaller removes: the installation whose folder it is in; otherwise the registered one
/// (the user's first); otherwise the folder it is in.
fn uninstall_target() -> Installed {
    let here = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let registered = [read_install(&CURRENT, Scope::User), read_install(&CURRENT, Scope::Machine)];
    if let Some(found) = registered.iter().flatten().find(|i| same_path(&i.dir, &here)) {
        return found.clone();
    }
    if let Some(found) = registered.into_iter().flatten().next() {
        return found;
    }
    let scope = if path_parts(&here).is_some_and(|p| within(&p, &program_files())) { Scope::Machine } else { Scope::User };
    Installed { scope, dir: here, version: None }
}

// ---------------------------------------------------------------------------------------------------------------
// The elevated part (installation for all users)

/// Program part of an installation: files, shortcuts and the "Apps" entry (for all users done by the elevated copy).
#[derive(Clone, Debug, PartialEq)]
struct ProgramPlan {
    scope: Scope,
    dir: PathBuf,
    lang: Lang,
    start_menu: bool,
    desktop: bool,
    /// Folder of the same installation before an update to a different folder: its program files are removed.
    previous_dir: Option<PathBuf>,
}

/// What the elevated copy does, read from the plan file.
#[derive(Clone, Debug, PartialEq)]
enum MachineTask {
    Install(ProgramPlan),
    Uninstall { dir: PathBuf, lang: Lang, parent_pid: u32 },
}

impl MachineTask {
    fn lang(&self) -> Lang {
        match self {
            MachineTask::Install(plan) => plan.lang,
            MachineTask::Uninstall { lang, .. } => *lang,
        }
    }

    fn to_text(&self) -> String {
        let flag = |b: bool| if b { "1" } else { "0" };
        match self {
            MachineTask::Install(p) => format!(
                "task=install\nscope={}\ndir={}\nlang={}\nstart_menu={}\ndesktop={}\nprevious_dir={}\n",
                p.scope.code(),
                p.dir.display(),
                p.lang.code(),
                flag(p.start_menu),
                flag(p.desktop),
                p.previous_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default()
            ),
            MachineTask::Uninstall { dir, lang, parent_pid } => {
                format!("task=uninstall\ndir={}\nlang={}\nparent={parent_pid}\n", dir.display(), lang.code())
            }
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let p = config::parse_properties(text);
        let dir = PathBuf::from(p.get("dir").filter(|d| !d.is_empty())?);
        let lang = Lang::parse(p.get("lang")?);
        match p.get("task")?.as_str() {
            "install" => Some(MachineTask::Install(ProgramPlan {
                scope: if p.get("scope")? == "machine" { Scope::Machine } else { Scope::User },
                dir,
                lang,
                start_menu: p.get("start_menu").is_some_and(|v| v == "1"),
                desktop: p.get("desktop").is_some_and(|v| v == "1"),
                previous_dir: p.get("previous_dir").filter(|d| !d.is_empty()).map(PathBuf::from),
            })),
            "uninstall" => Some(MachineTask::Uninstall { dir, lang, parent_pid: p.get("parent")?.parse().ok()? }),
            _ => None,
        }
    }
}

/// Files next to the plan file: progress and result of the elevated copy.
struct MachineFiles {
    progress: PathBuf,
    result: PathBuf,
}

impl MachineFiles {
    fn of(plan: &Path) -> Self {
        Self { progress: plan.with_extension("progress"), result: plan.with_extension("result") }
    }
}

/// The user cancelled the administrator rights prompt (UAC).
#[derive(Debug)]
struct ElevationCancelled;

impl std::fmt::Display for ElevationCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(crate::i18n::tr("administrator rights were not granted"))
    }
}

impl std::error::Error for ElevationCancelled {}

/// Creates a new file that the elevated copy writes in the user's temporary folder. It never opens an existing file
/// or follows a link planted under that name.
fn create_private_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new().write(true).create_new(true).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)
}

/// Result file: "ok" or "error", the lines (done steps or the error message) and "end" as the last line,
/// so that a partly written file is never taken for the result.
fn result_text(result: &Result<Vec<String>, String>) -> String {
    match result {
        Ok(lines) => format!("ok\n{}\nend\n", lines.join("\n")),
        Err(error) => format!("error\n{error}\nend\n"),
    }
}

fn parse_result(text: &str) -> Option<Result<Vec<String>, String>> {
    let mut lines: Vec<&str> = text.lines().collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.pop()? != "end" || lines.is_empty() {
        return None;
    }
    let body: Vec<String> = lines[1..].iter().filter(|l| !l.is_empty()).map(|l| l.to_string()).collect();
    match lines[0] {
        "ok" => Some(Ok(body)),
        "error" => Some(Err(body.join("\n"))),
        _ => None,
    }
}

fn write_result(path: &Path, result: &Result<Vec<String>, String>) {
    use std::io::Write;
    if let Ok(mut file) = create_private_file(path) {
        let _ = file.write_all(result_text(result).as_bytes());
        let _ = file.sync_all();
    }
}

/// Starts a program with administrator rights (the system prompt, UAC) and returns its process handle.
/// Cancelling the prompt gives the `ElevationCancelled` error.
fn start_elevated(file: &Path, parameters: &str, hwnd: isize, show: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD) -> Result<windows::Win32::Foundation::HANDLE> {
    use windows::core::{w, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_UNICODE, SHELLEXECUTEINFOW};
    let (file, parameters, directory) = (HSTRING::from(file.as_os_str()), HSTRING::from(parameters), HSTRING::from(system_dir().as_os_str()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC | SEE_MASK_UNICODE,
        hwnd: HWND(hwnd as *mut core::ffi::c_void),
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: show.0,
        ..Default::default()
    };
    // SAFETY: COM for the shell call in this thread; all strings live until the call returns.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        if let Err(e) = ShellExecuteExW(&mut info) {
            if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Err(ElevationCancelled.into());
            }
            return Err(anyhow!("{}", e.message()));
        }
    }
    Ok(info.hProcess)
}

/// Runs the machine-wide part in a copy of this program with administrator rights: writes the plan file, waits
/// for the copy while showing its progress and returns its result (the done steps, in the installation language).
fn run_elevated(task: &MachineTask, hwnd: isize, progress: &Progress) -> Result<Vec<String>> {
    use std::io::Write;
    use windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows::Win32::System::Threading::WaitForSingleObject;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let plan = std::env::temp_dir().join(format!("loonar-compass-setup-{}-{stamp}.plan", std::process::id()));
    let files = MachineFiles::of(&plan);
    let cleanup = || {
        for path in [&plan, &files.progress, &files.result] {
            let _ = std::fs::remove_file(path);
        }
    };
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&plan)
        .and_then(|mut file| file.write_all(task.to_text().as_bytes()))
        .with_context(|| crate::i18n::fmt("saving {0}", &[plan.display().to_string()]))?;

    progress.set(0.0, crate::i18n::tr("Waiting for administrator rights…"));
    let me = std::env::current_exe().context(crate::i18n::tr("installer path"))?;
    let parameters = format!("{MACHINE_PART_ARG} \"{}\"", plan.display());
    let process = match start_elevated(&me, &parameters, hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE) {
        Ok(process) => process,
        Err(e) => {
            cleanup();
            if e.is::<ElevationCancelled>() {
                return Err(e);
            }
            bail!("{}", crate::i18n::fmt("could not start the setup with administrator rights: {0}", &[format!("{e:#}")]));
        }
    };
    let started = std::time::Instant::now();
    let result = loop {
        let exited = if process.is_invalid() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            started.elapsed() > std::time::Duration::from_secs(4 * 3600)
        } else {
            // SAFETY: a process handle from ShellExecuteExW, closed below.
            unsafe { WaitForSingleObject(process, 100) != WAIT_TIMEOUT }
        };
        if let Some((value, step)) = std::fs::read_to_string(&files.progress).ok().as_deref().and_then(parse_progress) {
            progress.set(value, &step);
        }
        let result = std::fs::read_to_string(&files.result).ok().as_deref().and_then(parse_result);
        if result.is_some() || exited {
            break result;
        }
    };
    if !process.is_invalid() {
        // SAFETY: the handle is not used after this.
        unsafe {
            let _ = CloseHandle(process);
        }
    }
    cleanup();
    match result {
        Some(Ok(done)) => Ok(done),
        Some(Err(error)) => Err(anyhow!(error)),
        None => {
            let log = std::env::temp_dir().join(CRASH_LOG).display().to_string();
            bail!("{}", crate::i18n::fmt("the part of the setup that runs with administrator rights stopped without a result (details may be in {0})", &[log]))
        }
    }
}

/// The elevated copy (`--machine-part <plan>`): no window; does the machine-wide part, writes the result file and
/// returns the exit code. After an uninstallation it waits until the wizard closes and then removes the uninstaller.
fn machine_part(plan: &Path) -> i32 {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    let files = MachineFiles::of(plan);
    {
        let result = files.result.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            write_result(&result, &Err(info.to_string()));
            previous(info);
        }));
    }
    // The plan comes from the user's temporary folder: a folder that is itself a link is refused.
    let linked_folder = plan.parent().and_then(|d| std::fs::symlink_metadata(d).ok()).map_or(true, |m| {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    });
    let task = std::fs::read_to_string(plan).ok().and_then(|t| MachineTask::parse(&t));
    let Some(task) = task.filter(|_| !linked_folder) else {
        write_result(&files.result, &Err(crate::i18n::fmt("invalid setup instructions file {0}", &[plan.display().to_string()])));
        return 2;
    };
    i18n::set(task.lang());
    // The wizard waits for the result; its handle is opened now, while it surely still runs.
    let parent = match &task {
        // SAFETY: plain call; the handle is closed below.
        MachineTask::Uninstall { parent_pid, .. } => unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, *parent_pid).ok() },
        MachineTask::Install(_) => None,
    };
    let outcome = (|| -> Result<Vec<String>> {
        if !elevated() {
            bail!("{}", crate::i18n::tr("this step requires administrator rights"));
        }
        let progress = Progress::to_file(create_private_file(&files.progress)?);
        match &task {
            MachineTask::Install(plan) => {
                if plan.scope != Scope::Machine {
                    bail!("{}", crate::i18n::tr("this step requires administrator rights"));
                }
                check_folder(&plan.dir.display().to_string(), Scope::Machine).map_err(|m| anyhow!(m))?;
                let mut plan = plan.clone();
                plan.previous_dir = plan.previous_dir.filter(|d| check_folder(&d.display().to_string(), Scope::Machine).is_ok());
                install_program(&plan, &progress)
            }
            MachineTask::Uninstall { dir, .. } => {
                check_folder(&dir.display().to_string(), Scope::Machine).map_err(|m| anyhow!(m))?;
                remove_program(Scope::Machine, dir, &progress)
            }
        }
    })();
    write_result(&files.result, &outcome.as_ref().map(Clone::clone).map_err(|e| format!("{e:#}")));
    if let (Ok(_), MachineTask::Uninstall { dir, .. }) = (&outcome, &task) {
        if let Some(parent) = parent {
            // SAFETY: a handle from OpenProcess, closed after the wait (at most 4 hours).
            unsafe {
                WaitForSingleObject(parent, 4 * 3600 * 1000);
                let _ = CloseHandle(parent);
            }
        }
        if running_from(dir) {
            remove_uninstaller_later(dir);
        }
    } else if let Some(parent) = parent {
        // SAFETY: the handle is not used after this.
        unsafe {
            let _ = CloseHandle(parent);
        }
    }
    if outcome.is_ok() {
        0
    } else {
        1
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Installation and uninstallation

/// Creates a shortcut (.lnk) with the system ShellLink object, which stores paths in Unicode. The previously used
/// WScript.Shell stores them in the system code page and, on Windows with English regional settings,
/// could not save a shortcut with a Polish letter in its name (verified on Windows Server 2022).
fn create_shortcut(link: &Path, target: &Path, arguments: &str, description: &str) -> Result<()> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let working_dir = target.parent().unwrap_or(Path::new("."));
    let icon = working_dir.join(APP_EXE);
    // SAFETY: standard COM calls in the current thread; all strings live until the calls end.
    let saved = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        (|| -> windows::core::Result<()> {
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            shell_link.SetPath(&HSTRING::from(target.as_os_str()))?;
            shell_link.SetArguments(&HSTRING::from(arguments))?;
            shell_link.SetWorkingDirectory(&HSTRING::from(working_dir.as_os_str()))?;
            shell_link.SetDescription(&HSTRING::from(description))?;
            shell_link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)?;
            let file: IPersistFile = shell_link.cast()?;
            file.Save(&HSTRING::from(link.as_os_str()), true)
        })()
    };
    if let Err(e) = saved {
        bail!("{}", crate::i18n::fmt("could not create shortcut {0}: {1}", &[(link.display()).to_string(), (e).to_string()]));
    }
    if !link.is_file() {
        bail!("{}", crate::i18n::fmt("could not create shortcut {0}: the system did not save the file", &[(link.display()).to_string()]));
    }
    Ok(())
}

/// Start menu and desktop shortcuts as chosen in the wizard. Shortcuts of an earlier installation (possibly in
/// another language, or no longer wanted) are removed first.
fn update_shortcuts(plan: &ProgramPlan, app: &Path, uninstaller: &Path) -> Result<Vec<String>> {
    let mut done = Vec::new();
    let folder = plan.scope.start_menu()?;
    if folder.file_name().is_some_and(|n| n == APP_NAME) && folder.is_dir() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    if plan.start_menu {
        create_shortcut(&folder.join(format!("{APP_NAME}.lnk")), app, "", crate::i18n::tr("AI assistant for Enterprise Architect"))?;
        // An Internet shortcut to the user guide of this version on the website.
        let guide = folder.join(format!("{}.url", crate::i18n::tr("User guide")));
        std::fs::write(&guide, format!("[InternetShortcut]\r\nURL={}\r\n", docs_url(plan.lang)))
            .with_context(|| crate::i18n::fmt("saving {0}", &[guide.display().to_string()]))?;
        create_shortcut(&folder.join(crate::i18n::fmt("Uninstall {0}.lnk", &[APP_NAME.to_string()])), uninstaller, UNINSTALL_ARG, crate::i18n::tr("Uninstall the application"))?;
        done.push(crate::i18n::tr("Start menu shortcuts").to_string());
    }
    let desktop = plan.scope.desktop()?.join(format!("{APP_NAME}.lnk"));
    if plan.desktop {
        create_shortcut(&desktop, app, "", crate::i18n::tr("AI assistant for Enterprise Architect"))?;
        done.push(crate::i18n::tr("Desktop shortcut").to_string());
    } else {
        let _ = std::fs::remove_file(&desktop);
    }
    Ok(done)
}

/// Date for "InstallDate" (yyyymmdd, local time).
fn install_date() -> String {
    // SAFETY: plain call without pointers.
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format_date(now.wYear, now.wMonth, now.wDay)
}

fn format_date(year: u16, month: u16, day: u16) -> String {
    format!("{year:04}{month:02}{day:02}")
}

/// The "Apps" / "Programs and Features" entry (for the current user in HKCU, for all users in HKLM).
fn register(plan: &ProgramPlan, app: &Path, uninstaller: &Path) -> Result<()> {
    let (root, key) = (plan.scope.root(), CURRENT.uninstall_key);
    // Values of an earlier version are not carried over.
    let _ = reg::delete_key(root, key);
    let size: u64 = program_file_names().iter().filter_map(|f| std::fs::metadata(plan.dir.join(f)).ok()).map(|m| m.len()).sum();
    let uninstall = format!("\"{}\" {UNINSTALL_ARG}", uninstaller.display());
    let strings = [
        ("DisplayName", APP_NAME.to_string()),
        ("DisplayVersion", app_version().to_string()),
        ("Publisher", PUBLISHER.to_string()),
        ("DisplayIcon", app.display().to_string()),
        ("InstallLocation", plan.dir.display().to_string()),
        ("UninstallString", uninstall.clone()),
        ("QuietUninstallString", format!("{uninstall} {QUIET_ARG}")),
        ("URLInfoAbout", URL_INFO.to_string()),
        ("HelpLink", URL_HELP.to_string()),
        ("InstallDate", install_date()),
    ];
    for (name, value) in &strings {
        reg::set_string(root, key, name, value)?;
    }
    let size_kb = u32::try_from(size.div_ceil(1024)).unwrap_or(u32::MAX);
    for (name, value) in [("EstimatedSize", size_kb), ("NoModify", 1), ("NoRepair", 1)] {
        reg::set_dword(root, key, name, value)?;
    }
    Ok(())
}

/// Program part of an installation: program files, shortcuts and the "Apps" entry. For all users this runs in the
/// elevated copy, otherwise in the wizard.
fn install_program(plan: &ProgramPlan, progress: &Progress) -> Result<Vec<String>> {
    let mut done = Vec::new();
    let dir = &plan.dir;
    progress.set(0.02, crate::i18n::tr("Creating the program folder…"));
    std::fs::create_dir_all(dir).with_context(|| crate::i18n::fmt("cannot create folder {0}", &[(dir.display()).to_string()]))?;

    let app = dir.join(APP_EXE);
    write_with_progress(&app, PAYLOAD_APP, progress, 0.04, 0.62, &crate::i18n::fmt("Copying {0}…", &[(APP_EXE).to_string()])).with_context(|| {
        crate::i18n::fmt("cannot write {0} (if the application is running, close it and try again)", &[(app.display()).to_string()])
    })?;
    progress.set(0.64, crate::i18n::tr("Copying the license and changelog…"));
    write_file(&dir.join(LICENSE_FILE), payload_license(plan.lang))?;
    if plan.lang == Lang::Pl {
        let _ = std::fs::remove_file(dir.join(LICENSE_PL_FILE));
    } else {
        write_file(&dir.join(LICENSE_PL_FILE), payload_license(Lang::Pl))?;
    }
    write_file(&dir.join(CHANGELOG_FILE), payload_changelog(plan.lang))?;
    write_file(&dir.join(NOTICES_FILE), PAYLOAD_NOTICES)?;
    // Written always (it is registered in EA only when chosen): an update of the program updates the add-in too.
    write_file(&dir.join(ADDIN_DLL), PAYLOAD_ADDIN).with_context(|| {
        crate::i18n::fmt("cannot write {0} (if Enterprise Architect is running, close it and try again)", &[ADDIN_DLL.to_string()])
    })?;
    // The user guide is on the website now: PDFs left by earlier versions are removed.
    for lang in Lang::ALL {
        let _ = std::fs::remove_file(dir.join(manual_file(lang)));
    }
    progress.set(0.74, crate::i18n::tr("Copying the uninstaller…"));
    let uninstaller = dir.join(UNINSTALL_EXE);
    let me = std::env::current_exe().context(crate::i18n::tr("installer path"))?;
    if !same_path(&me, &uninstaller) {
        // An exact copy (not CopyFile, which would also copy the "downloaded from the Internet" mark).
        let bytes = std::fs::read(&me).context(crate::i18n::tr("reading the installer"))?;
        write_with_progress(&uninstaller, &bytes, progress, 0.74, 0.84, crate::i18n::tr("Copying the uninstaller…"))
            .context(crate::i18n::tr("copying the uninstaller"))?;
    }
    if let Some(previous) = plan.previous_dir.as_ref().filter(|p| !same_path(p, dir)) {
        // The same application moved to another folder: the old copy goes away.
        for file in program_file_names() {
            let _ = std::fs::remove_file(previous.join(file));
        }
        let _ = std::fs::remove_dir(previous);
    }
    done.push(crate::i18n::fmt("Program files: {0}", &[(dir.display()).to_string()]));

    progress.set(0.86, crate::i18n::tr("Creating shortcuts…"));
    done.extend(update_shortcuts(plan, &app, &uninstaller)?);

    progress.set(0.94, crate::i18n::tr("Adding the entry to \"Programs and Features\"…"));
    register(plan, &app, &uninstaller)?;
    done.push(crate::i18n::tr("Entry in \"Programs and Features\"").to_string());
    progress.set(1.0, crate::i18n::tr("Done"));
    Ok(done)
}

/// Program part of an uninstallation: program files, shortcuts and the "Apps" entry. The uninstaller that is
/// running is removed after it closes (`remove_uninstaller_later`).
fn remove_program(scope: Scope, dir: &Path, progress: &Progress) -> Result<Vec<String>> {
    let mut done = Vec::new();
    // Only this installation's entry (the registry may already describe an installation in another folder).
    let registered = reg::get_string(scope.root(), CURRENT.uninstall_key, "InstallLocation");
    let registered_here = registered.as_deref().is_some_and(|location| same_path(Path::new(location), dir));
    // A registered folder whose files were deleted by hand still gets its shortcuts and entry removed.
    if !dir.join(APP_EXE).is_file() && !dir.join(UNINSTALL_EXE).is_file() && !registered_here {
        bail!("{}", crate::i18n::fmt("the application is not installed in folder {0}", &[(dir.display()).to_string()]));
    }
    progress.set(0.05, crate::i18n::tr("Removing program files…"));
    let keep_uninstaller = running_from(dir);
    for file in program_file_names() {
        let path = dir.join(&file);
        if (file == UNINSTALL_EXE && keep_uninstaller) || !path.is_file() {
            continue;
        }
        std::fs::remove_file(&path).with_context(|| {
            crate::i18n::fmt("cannot remove {0} (if the application is running, close it and try again)", &[(path.display()).to_string()])
        })?;
    }
    if !keep_uninstaller {
        let _ = std::fs::remove_dir(dir);
    }
    done.push(crate::i18n::tr("Program files removed").to_string());

    progress.set(0.60, crate::i18n::tr("Removing shortcuts…"));
    if let Ok(folder) = scope.start_menu() {
        if folder.file_name().is_some_and(|n| n == APP_NAME) && folder.is_dir() {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
    if let Ok(desktop) = scope.desktop() {
        let _ = std::fs::remove_file(desktop.join(format!("{APP_NAME}.lnk")));
    }
    done.push(crate::i18n::tr("Shortcuts removed").to_string());

    progress.set(0.85, crate::i18n::tr("Removing the \"Programs and Features\" entry…"));
    if registered_here || registered.is_none() {
        reg::delete_key(scope.root(), CURRENT.uninstall_key)?;
    }
    done.push(crate::i18n::tr("\"Programs and Features\" entry removed").to_string());
    progress.set(1.0, crate::i18n::tr("Done"));
    Ok(done)
}

/// Command line that removes the uninstaller and the then empty program folder a few seconds after the uninstaller
/// closes (a running program cannot delete itself). `None` for a folder name the command line cannot carry safely.
fn cleanup_command(dir: &Path, system: &Path) -> Option<String> {
    let folder = dir.display().to_string();
    if folder.contains(['%', '"']) {
        return None;
    }
    // Outer quotes: cmd /c removes the first and the last quote of the line, the inner ones stay.
    Some(format!(
        "\"\"{}\" -n 4 127.0.0.1 >nul & del /f /q \"{}\" & rmdir \"{}\"\"",
        system.join("PING.EXE").display(),
        dir.join(UNINSTALL_EXE).display(),
        folder
    ))
}

fn remove_uninstaller_later(dir: &Path) {
    let system = system_dir();
    let Some(line) = cleanup_command(dir, &system) else { return };
    // Full path of cmd.exe, without AutoRun commands (/d); working folder outside the program folder
    // (the folder a command "stands" in cannot be deleted).
    let _ = Command::new(system.join("cmd.exe"))
        .raw_arg("/d")
        .raw_arg("/c")
        .raw_arg(&line)
        .current_dir(&system)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// Configuration data entered in the wizard. An empty field means "unchanged" (matters on an update).
#[derive(Clone, Default)]
struct ConfigForm {
    configure_now: bool,
    api_key: String,
    /// "postgres" or "file" (an EA project file .qea/.qeax).
    model_source: String,
    model_file: String,
    db_host: String,
    db_port: String,
    db_name: String,
    db_user: String,
    db_password: String,
    ea_path: String,
    ea_user: String,
    ea_password: String,
    approval_mode: String,
}

impl ConfigForm {
    /// A value that cannot be saved (shown on the configuration page, which then cannot be left with "Next").
    fn problem(&self) -> Option<&'static str> {
        if !self.configure_now {
            return None;
        }
        if self.uses_file() {
            let file = self.model_file.trim();
            return (!file.is_empty() && !config::is_model_file(Path::new(file))).then(|| crate::i18n::tr("Choose an Enterprise Architect project file .qea or .qeax"));
        }
        self.db_port.trim().parse::<u16>().is_err().then(|| crate::i18n::tr("Database port: enter a number"))
    }

    fn uses_file(&self) -> bool {
        self.model_source == "file"
    }
}

/// User settings and secrets: always written by the wizard itself, never by the elevated copy, so they land in
/// the profile of the user who runs the wizard.
fn save_user_settings(c: &ConfigForm, lang: Lang) -> Result<Vec<String>> {
    let props = config::user_data_dir().join("app.properties");
    let new_file = !props.is_file();
    // Language chosen in the wizard: always, also on an update and when configuration is skipped now.
    config::save_properties(&props, &[("ui.language", lang.code().to_string())])?;
    if !c.configure_now {
        return Ok(vec![crate::i18n::tr("Settings: to be completed in the application, in the \"Settings\" view").to_string()]);
    }
    let mode = config::ApprovalMode::parse(&c.approval_mode)?;
    let mut updates: Vec<(&str, String)> = vec![
        ("ea.path", c.ea_path.trim().to_string()),
        ("ea.user", c.ea_user.trim().to_string()),
        ("approval.mode", mode.as_str().to_string()),
    ];
    if c.uses_file() {
        updates.push(("model.source", "file".to_string()));
        updates.push(("model.file", c.model_file.trim().trim_matches('"').to_string()));
    } else {
        let port: u16 = c.db_port.trim().parse().context(crate::i18n::tr("Database port: enter a number"))?;
        updates.push(("model.source", "postgres".to_string()));
        updates.push(("db.host", c.db_host.trim().to_string()));
        updates.push(("db.port", port.to_string()));
        updates.push(("db.name", c.db_name.trim().to_string()));
        updates.push(("db.user", c.db_user.trim().to_string()));
    }
    // On an update, existing values the wizard does not provide (e.g. the token limit) are kept.
    if new_file {
        updates.push(("claude.max_tokens", "10000".to_string()));
    }
    config::save_properties(&props, &updates)?;
    // The database password lives only in Credential Manager; one left in the file by a version before 0.3 moves there.
    let in_file = std::fs::read_to_string(&props).map(|t| config::parse_properties(&t).get("db.password").cloned()).unwrap_or_default();
    let db_password = if c.db_password.is_empty() { in_file.clone().filter(|p| !p.is_empty()) } else { Some(c.db_password.clone()) };
    if let Some(password) = db_password {
        secrets::set(secrets::DB_PASSWORD, &password)?;
    }
    if in_file.is_some() {
        config::remove_properties(&props, &["db.password"])?;
    }
    if !c.api_key.trim().is_empty() {
        secrets::set(secrets::CLAUDE_API_KEY, c.api_key.trim())?;
    }
    if !c.ea_password.is_empty() {
        secrets::set(secrets::EA_PASSWORD, &c.ea_password)?;
    }
    Ok(vec![crate::i18n::fmt("User settings: {0}", &[(props.display()).to_string()])])
}

/// Removes an installation just for the current user that the new one replaces (the pre-release version, or this
/// application when it is now installed for all users): its program files, shortcuts and "Apps" entry.
/// User data and secrets are never touched. The error is a message for the user (the installation itself succeeded).
fn remove_user_install(product: &Product, old: &Installed, new_dir: &Path) -> Result<String, String> {
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_QuickLaunch};
    let dir = &old.dir;
    let failed = |error: String| {
        crate::i18n::fmt("The earlier installation in {0} could not be removed completely: {1}. Remove it in Settings → Apps.", &[dir.display().to_string(), error])
    };
    let same_dir = same_path(dir, new_dir);
    // The program first: if it is running, the earlier installation stays complete (and can be removed later).
    let exe = dir.join(product.exe);
    if !(same_dir && product.exe == APP_EXE) && exe.is_file() {
        std::fs::remove_file(&exe).map_err(|e| failed(e.to_string()))?;
    }
    if !same_dir {
        for file in program_file_names() {
            let _ = std::fs::remove_file(dir.join(file));
        }
        let _ = std::fs::remove_dir(dir);
    }
    if let Ok(programs) = known_folder(&FOLDERID_Programs) {
        let folder = programs.join(product.name);
        if folder.is_dir() {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
    let link = format!("{}.lnk", product.name);
    for id in [&FOLDERID_Desktop, &FOLDERID_QuickLaunch] {
        if let Ok(folder) = known_folder(id) {
            let _ = std::fs::remove_file(folder.join(&link));
        }
    }
    reg::delete_key(reg::Root::CurrentUser, product.uninstall_key).map_err(|e| failed(format!("{e:#}")))?;
    Ok(crate::i18n::fmt("Earlier installation removed: {0}", &[dir.display().to_string()]))
}

/// What a finished installation or uninstallation reports on the last page.
#[derive(Clone, Default)]
struct Report {
    done: Vec<String>,
    /// Problems that did not stop the work (e.g. an earlier version that could not be removed).
    warnings: Vec<String>,
}

#[derive(Clone)]
struct InstallPlan {
    program: ProgramPlan,
    /// Register the add-in in Enterprise Architect for the user who installs.
    ea_addin: bool,
    config: ConfigForm,
    /// Installations just for the current user that the new one replaces.
    replaces: Vec<(&'static Product, Installed)>,
}

/// Installation: the program part (elevated for all users), then the user's settings and secrets, then
/// the removal of what the new installation replaces.
fn install(plan: &InstallPlan, hwnd: isize, progress: &Progress) -> Result<Report> {
    let program = &plan.program;
    check_folder(&program.dir.display().to_string(), program.scope).map_err(|m| anyhow!(m))?;
    if let Some(problem) = plan.config.problem() {
        bail!("{problem}");
    }
    let part = progress.part(0.0, 0.84);
    let mut done = if program.scope == Scope::Machine && !elevated() {
        run_elevated(&MachineTask::Install(program.clone()), hwnd, &part)?
    } else {
        install_program(program, &part)?
    };
    progress.set(0.86, crate::i18n::tr("Saving settings…"));
    done.extend(save_user_settings(&plan.config, program.lang)?);
    // The add-in is registered for the user (not in the elevated copy: its registry part would belong to the administrator).
    if plan.ea_addin {
        ea_addin::register(&program.dir.join(ADDIN_DLL))?;
        done.push(crate::i18n::tr("Loonar Compass in Enterprise Architect (Start ribbon → Add-Ins)").to_string());
    } else {
        ea_addin::unregister();
    }
    let mut warnings = Vec::new();
    for (product, old) in &plan.replaces {
        progress.set(0.92, crate::i18n::tr("Removing the earlier version…"));
        match remove_user_install(product, old, &program.dir) {
            Ok(line) => done.push(line),
            Err(warning) => warnings.push(warning),
        }
    }
    progress.set(1.0, crate::i18n::tr("Done"));
    Ok(Report { done, warnings })
}

/// Uninstallation: the program part (elevated for all users), then — for the user who runs the uninstaller —
/// the user data and secrets, if chosen.
fn uninstall(target: &Installed, remove_data: bool, remove_secrets: bool, hwnd: isize, progress: &Progress) -> Result<Report> {
    let part = progress.part(0.0, 0.80);
    let mut done = if target.scope == Scope::Machine && !elevated() {
        let task = MachineTask::Uninstall { dir: target.dir.clone(), lang: i18n::current(), parent_pid: std::process::id() };
        run_elevated(&task, hwnd, &part)?
    } else {
        remove_program(target.scope, &target.dir, &part)?
    };
    let mut warnings = Vec::new();
    ea_addin::unregister();
    progress.set(0.85, crate::i18n::tr("Cleaning up user data…"));
    // Read before the data folder goes: the names of the secrets saved for conversation connections.
    let connection_secrets = secrets::connection_names(&config::user_data_dir());
    if remove_data {
        let data = config::user_data_dir();
        match std::fs::remove_dir_all(&data) {
            Err(e) if data.exists() => warnings.push(format!("{}: {e}", crate::i18n::fmt("cannot remove {0}", &[data.display().to_string()]))),
            _ => done.push(crate::i18n::tr("Settings, conversations, attachments, memory, templates and the audit log removed").to_string()),
        }
    }
    if remove_secrets {
        let failed: Vec<String> = secrets::ALL
            .iter()
            .map(|name| name.to_string())
            .chain(connection_secrets)
            .filter_map(|name| secrets::delete(&name).err().map(|e| crate::i18n::fmt("could not remove {0} from Credential Manager: {1}", &[name.to_string(), format!("{e:#}")])))
            .collect();
        if failed.is_empty() {
            done.push(crate::i18n::tr("API key, passwords and tokens removed from Credential Manager").to_string());
        }
        warnings.extend(failed);
    }
    progress.set(1.0, crate::i18n::tr("Done"));
    Ok(Report { done, warnings })
}

/// Uninstaller of the pre-release version installed for all users (in its folder, or from its "Apps" entry).
fn legacy_uninstaller(old: &Installed) -> Option<PathBuf> {
    let in_folder = old.dir.join(UNINSTALL_EXE);
    if in_folder.is_file() {
        return Some(in_folder);
    }
    reg::get_string(old.scope.root(), LEGACY.uninstall_key, "UninstallString").and_then(|c| uninstaller_from_command(&c)).filter(|p| p.is_file())
}

/// Runs the uninstaller of the pre-release version installed for all users (it asks for administrator rights)
/// and waits until it closes.
fn remove_legacy_machine(hwnd: isize) -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
    // Secrets still saved under the pre-release name move to the current one first: the old uninstaller offers to delete them.
    for name in secrets::ALL {
        let _ = secrets::get(name);
    }
    let Some(old) = read_install(&LEGACY, Scope::Machine) else { return Ok(()) };
    let fail = |detail: String| anyhow!(crate::i18n::fmt("could not start the uninstaller of the earlier version: {0}", &[detail]));
    let uninstaller = legacy_uninstaller(&old).ok_or_else(|| fail(old.dir.join(UNINSTALL_EXE).display().to_string()))?;
    let process = match start_elevated(&uninstaller, UNINSTALL_ARG, hwnd, windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL) {
        Ok(process) => process,
        Err(e) if e.is::<ElevationCancelled>() => return Err(e),
        Err(e) => return Err(fail(format!("{e:#}"))),
    };
    if !process.is_invalid() {
        // SAFETY: a process handle from ShellExecuteExW, closed after the wait.
        unsafe {
            WaitForSingleObject(process, INFINITE);
            let _ = CloseHandle(process);
        }
    }
    Ok(())
}

/// `--uninstall --quiet`: uninstallation without a window (user data and secrets stay). Exit code 0 = done,
/// 1223 = administrator rights not granted, 1 = error.
fn quiet_uninstall() -> i32 {
    let target = uninstall_target();
    let progress = Progress::default();
    let result = if target.scope == Scope::Machine && !elevated() {
        let task = MachineTask::Uninstall { dir: target.dir.clone(), lang: i18n::current(), parent_pid: std::process::id() };
        run_elevated(&task, 0, &progress)
    } else {
        remove_program(target.scope, &target.dir, &progress)
    };
    match result {
        Ok(_) => {
            // For all users without rights, the elevated copy removes the uninstaller after this process ends.
            if (target.scope == Scope::User || elevated()) && running_from(&target.dir) {
                remove_uninstaller_later(&target.dir);
            }
            0
        }
        Err(e) if e.is::<ElevationCancelled>() => 1223,
        Err(_) => 1,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The wizard

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Page {
    Language,
    Welcome,
    News,
    License,
    Folder,
    Config,
    Summary,
    Working,
    Finish,
}

/// Background work failure as the window shows it.
enum Failure {
    /// The administrator rights prompt was cancelled: the wizard goes back with a message.
    Cancelled,
    Error(String),
}

fn failure(e: anyhow::Error) -> Failure {
    if e.is::<ElevationCancelled>() {
        Failure::Cancelled
    } else {
        Failure::Error(format!("{e:#}"))
    }
}

/// Removal of the pre-release version installed for all users (button on the summary page).
enum OldRemoval {
    Idle,
    Running(mpsc::Receiver<Result<(), Failure>>),
    Removed,
    Cancelled,
    Failed(String),
}

struct Setup {
    /// Language of the wizard, the application and the default documents (first wizard page).
    lang: Lang,
    uninstall: bool,
    page: Page,
    accepted: bool,
    scope: Scope,
    dir: String,
    start_menu: bool,
    desktop: bool,
    ea_addin: bool,
    config: ConfigForm,
    /// Uninstallation: whether to also remove user data and secrets.
    remove_data: bool,
    remove_secrets: bool,
    work: Option<mpsc::Receiver<Result<Report, Failure>>>,
    outcome: Option<Result<Report, String>>,
    /// Message on the folder or uninstall page (e.g. administrator rights not granted).
    notice: Option<String>,
    run_after: bool,
    license: String,
    /// Existing installations of this application and of the pre-release version.
    user_install: Option<Installed>,
    machine_install: Option<Installed>,
    legacy_user: Option<Installed>,
    legacy_machine: Option<Installed>,
    old_removal: OldRemoval,
    /// Installed version when this is an update.
    installed_version: Option<String>,
    /// What's new: changelog sections since the installed version.
    news: Vec<(String, Vec<String>)>,
    /// Background work progress and the value shown on the bar (catches up with the progress smoothly).
    progress: Progress,
    shown: f32,
}

/// Window size that fits the screen: the wizard's own size, made smaller (in points) when the work area of the
/// primary monitor is smaller, e.g. 1366×768 at 125% or 1920×1080 at 175%; centred in the work area, above the
/// taskbar. The page content scrolls, so the buttons stay visible at any size.
fn window_geometry() -> (egui::Vec2, Option<egui::Pos2>) {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSY};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SystemParametersInfoW, SM_CXPADDEDBORDER, SM_CYCAPTION, SM_CYSIZEFRAME, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let wanted = egui::vec2(760.0, 640.0);
    let mut work = RECT::default();
    // SAFETY: plain queries; the RECT outlives the call and the screen DC is released.
    let (dpi, caption, frame) = unsafe {
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, Some((&mut work as *mut RECT).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_err() {
            return (wanted, None);
        }
        let dc = GetDC(HWND::default());
        let dpi = GetDeviceCaps(dc, LOGPIXELSY);
        ReleaseDC(HWND::default(), dc);
        (dpi, GetSystemMetrics(SM_CYCAPTION), GetSystemMetrics(SM_CYSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER))
    };
    let scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
    let (work_w, work_h) = ((work.right - work.left) as f32, (work.bottom - work.top) as f32);
    if work_w <= 0.0 || work_h <= 0.0 {
        return (wanted, None);
    }
    // Window frame and title bar in pixels, with a margin.
    let (chrome_w, chrome_h) = ((2 * frame) as f32 + 8.0, (caption + 2 * frame) as f32 + 8.0);
    let size = egui::vec2(
        wanted.x.min((work_w - chrome_w) / scale).max(520.0),
        wanted.y.min((work_h - chrome_h) / scale).max(380.0),
    );
    let x = work.left as f32 + ((work_w - (size.x * scale + chrome_w)) / 2.0).max(0.0);
    let y = work.top as f32 + ((work_h - (size.y * scale + chrome_h)) / 2.0).max(0.0);
    (size, Some(egui::pos2(x / scale, y / scale)))
}

fn main() -> eframe::Result {
    // Libraries loaded on demand come from System32 only, never from the download folder.
    // SAFETY: plain call, first thing in the program.
    unsafe {
        let _ = windows::Win32::System::LibraryLoader::SetDefaultDllDirectories(windows::Win32::System::LibraryLoader::LOAD_LIBRARY_SEARCH_SYSTEM32);
    }
    window::use_system_dpi_awareness();
    window::install_panic_hook(APP_NAME, CRASH_LOG);
    let args: Vec<OsString> = std::env::args_os().collect();
    if let Some(i) = args.iter().position(|a| a == MACHINE_PART_ARG) {
        std::process::exit(args.get(i + 1).map_or(2, |plan| machine_part(Path::new(plan))));
    }
    let uninstall = args.iter().any(|a| a == UNINSTALL_ARG);
    // Data of the pre-release name move to the current folder before the settings are read.
    config::migrate_legacy_data_dir();
    i18n::set(saved_language());
    if uninstall && args.iter().any(|a| a == QUIET_ARG) {
        std::process::exit(quiet_uninstall());
    }
    let (size, position) = window_geometry();
    let options = move || {
        let mut viewport = egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(Arc::new(egui::IconData { rgba: ICON_RGBA.to_vec(), width: 256, height: 256 }));
        if let Some(position) = position {
            viewport = viewport.with_position(position);
        }
        eframe::NativeOptions { viewport, centered: position.is_none(), ..Default::default() }
    };
    let title = window_title(uninstall);
    window::run(&title, options, move |cc| {
        apply_theme(&cc.egui_ctx);
        Setup::new(uninstall)
    })
}

/// Language saved in the user settings (update). Settings without a language come from versions before 0.3,
/// which were Polish only; a new installation starts in English.
fn saved_language() -> Lang {
    match std::fs::read_to_string(config::user_data_dir().join("app.properties")) {
        Ok(text) => Lang::parse(config::parse_properties(&text).get("ui.language").map_or("pl", String::as_str)),
        Err(_) => Lang::En,
    }
}

/// Wizard window title in the current language.
fn window_title(uninstall: bool) -> String {
    if uninstall {
        crate::i18n::fmt("Uninstall {0}", &[APP_NAME.to_string()])
    } else {
        crate::i18n::fmt("{0} Setup", &[APP_NAME.to_string()])
    }
}

/// Version as numbers for comparison ("0.10" > "0.2", "0.3.0" = "0.3").
fn version_key(v: &str) -> Vec<u32> {
    let core = v.trim().split(['-', '+', ' ']).next().unwrap_or("");
    let mut key: Vec<u32> = core.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect();
    while key.len() > 1 && key.last() == Some(&0) {
        key.pop();
    }
    key
}

/// Changelog sections newer than the installed version up to `current` (on a new installation: only `current`),
/// as pairs (version heading, content lines).
fn whats_new(changelog: &str, installed: Option<&str>, current: &str) -> Vec<(String, Vec<String>)> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in changelog.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            sections.push((head.trim().to_string(), Vec::new()));
        } else if let Some((_, lines)) = sections.last_mut() {
            lines.push(line.to_string());
        }
    }
    let version_of = |head: &str| head.split_whitespace().find(|w| w.starts_with(|c: char| c.is_ascii_digit())).unwrap_or("").to_string();
    let current = version_key(current);
    sections
        .into_iter()
        .filter(|(head, _)| {
            let v = version_key(&version_of(head));
            match installed {
                Some(old) => v > version_key(old) && v <= current,
                None => v == current,
            }
        })
        .map(|(head, lines)| {
            let mut lines: Vec<String> = lines.into_iter().map(|l| l.trim_end().to_string()).collect();
            while lines.first().is_some_and(|l| l.is_empty()) {
                lines.remove(0);
            }
            while lines.last().is_some_and(|l| l.is_empty()) {
                lines.pop();
            }
            (head, lines)
        })
        .collect()
}

/// Disk space the installation needs, in MB (application, uninstaller = this program, user guide, texts).
fn required_mb() -> u64 {
    static MB: OnceLock<u64> = OnceLock::new();
    *MB.get_or_init(|| {
        let setup = std::env::current_exe().and_then(std::fs::metadata).map(|m| m.len()).unwrap_or(PAYLOAD_APP.len() as u64);
        (PAYLOAD_APP.len() as u64 + setup + PAYLOAD_NOTICES.len() as u64 + 256 * 1024).div_ceil(1_048_576)
    })
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = theme::BACKGROUND;
    v.window_fill = theme::BACKGROUND;
    v.extreme_bg_color = theme::BACKGROUND;
    v.override_text_color = Some(theme::TEXT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, theme::BORDER);
    v.widgets.inactive.weak_bg_fill = theme::BUTTON;
    v.widgets.inactive.bg_fill = theme::BUTTON;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, theme::BORDER);
    v.widgets.hovered.weak_bg_fill = theme::BUTTON_HOVER;
    v.widgets.hovered.bg_fill = theme::BUTTON_HOVER;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.rounding = egui::Rounding::same(6.0);
    }
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_visuals_of(egui::Theme::Light, v);

    // System fonts: native Windows look and Polish characters.
    let mut fonts = egui::FontDefinitions::default();
    let font_dir = windows_dir().join("Fonts");
    if let Ok(bytes) = std::fs::read(font_dir.join("segoeui.ttf")) {
        fonts.font_data.insert("segoe_ui".to_owned(), egui::FontData::from_owned(bytes));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "segoe_ui".to_owned());
    }
    // Characters missing from Segoe UI (e.g. ✓) come from the symbol font.
    if let Ok(bytes) = std::fs::read(font_dir.join("seguisym.ttf")) {
        fonts.font_data.insert("segoe_symbol".to_owned(), egui::FontData::from_owned(bytes));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().push("segoe_symbol".to_owned());
    }
    let mut bold = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    if let Ok(bytes) = std::fs::read(font_dir.join("segoeuib.ttf")) {
        fonts.font_data.insert("segoe_ui_bold".to_owned(), egui::FontData::from_owned(bytes));
        bold.insert(0, "segoe_ui_bold".to_owned());
    }
    fonts.families.insert(egui::FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(14.0, 6.0);
    for (text_style, font) in style.text_styles.iter_mut() {
        font.size = match text_style {
            egui::TextStyle::Heading => 21.0,
            egui::TextStyle::Small => 12.0,
            _ => 14.5,
        };
    }
    ctx.set_style(style);
}

fn bold(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(text).font(egui::FontId::new(size, egui::FontFamily::Name("bold".into())))
}

fn muted(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(theme::MUTED)).wrap());
}

fn error_text(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).color(theme::ERROR)).wrap());
}

fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let id = egui::Id::new(name);
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let rgba = image::load_from_memory(bytes).expect("embedded image").to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let handle = ctx.load_texture(name, egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, secret: bool) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).password(secret).desired_width(250.0));
    ui.end_row();
}

/// The window that owns the wizard (parent of the administrator rights prompt), as a number that can be sent to
/// the worker thread.
fn wizard_window() -> isize {
    // SAFETY: plain call on the UI thread, which owns the wizard window.
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow() }.0 as isize
}

impl Setup {
    fn new(uninstall: bool) -> Self {
        let user_install = read_install(&CURRENT, Scope::User);
        let machine_install = read_install(&CURRENT, Scope::Machine);
        let legacy_user = read_install(&LEGACY, Scope::User);
        let legacy_machine = read_install(&LEGACY, Scope::Machine);
        // An update keeps the type and folder of the existing installation; otherwise just for the current user.
        let (scope, dir) = if uninstall {
            let target = uninstall_target();
            (target.scope, target.dir)
        } else if let (None, Some(machine)) = (&user_install, &machine_install) {
            (Scope::Machine, machine.dir.clone())
        } else {
            (Scope::User, user_install.as_ref().map_or_else(|| Scope::User.default_dir(), |i| i.dir.clone()))
        };
        let installed_version = [&user_install, &machine_install, &legacy_user, &legacy_machine].into_iter().flatten().find_map(|i| i.version.clone());
        // Initial field values: from the existing user settings (update) or the defaults.
        let saved = std::fs::read_to_string(config::user_data_dir().join("app.properties")).map(|t| config::parse_properties(&t)).unwrap_or_default();
        let get = |key: &str, default: &str| saved.get(key).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| default.to_string());
        // Older files have the EA path with doubled backslashes (C:\\Program Files\\...), as the application reads it.
        let ea_path = get("ea.path", DEFAULT_EA_PATH);
        let ea_path = if ea_path.contains(":\\\\") { ea_path.replace("\\\\", "\\") } else { ea_path };
        let approval_mode = config::ApprovalMode::parse(&get("approval.mode", "ask")).map_or("ask", |m| m.as_str()).to_string();
        let config = ConfigForm {
            configure_now: get("db.host", "").is_empty() && get("model.file", "").is_empty(),
            model_source: if get("model.source", "postgres") == "file" { "file".to_string() } else { "postgres".to_string() },
            model_file: get("model.file", ""),
            db_host: get("db.host", ""),
            db_port: get("db.port", "5432"),
            db_name: get("db.name", ""),
            db_user: get("db.user", "postgres"),
            ea_path,
            ea_user: get("ea.user", ""),
            approval_mode,
            ..Default::default()
        };
        let lang = i18n::current();
        Self {
            lang,
            uninstall,
            // Development build: LOONAR_SETUP_PAGE=news opens the "What's new" page right away (for previewing).
            page: if cfg!(debug_assertions) && std::env::var("LOONAR_SETUP_PAGE").as_deref() == Ok("news") {
                Page::News
            } else if uninstall {
                Page::Welcome
            } else {
                Page::Language
            },
            accepted: false,
            scope,
            dir: dir.display().to_string(),
            start_menu: true,
            desktop: true,
            ea_addin: true,
            config,
            remove_data: false,
            remove_secrets: false,
            work: None,
            outcome: None,
            notice: None,
            run_after: true,
            license: text_of(payload_license(lang)),
            news: whats_new(&text_of(payload_changelog(lang)), installed_version.as_deref(), app_version()),
            installed_version,
            user_install,
            machine_install,
            legacy_user,
            legacy_machine,
            old_removal: OldRemoval::Idle,
            progress: Progress::default(),
            shown: 0.0,
        }
    }

    /// Folder suggested for an installation type: the existing installation's or the default.
    fn suggested_dir(&self, scope: Scope) -> PathBuf {
        let existing = if scope == Scope::Machine { &self.machine_install } else { &self.user_install };
        existing.as_ref().map_or_else(|| scope.default_dir(), |i| i.dir.clone())
    }

    /// Change of the installation type: a suggested folder follows it, a folder the user typed stays.
    fn set_scope(&mut self, scope: Scope) {
        if self.dir.trim().is_empty() || same_path(Path::new(self.dir.trim()), &self.suggested_dir(self.scope)) {
            self.dir = self.suggested_dir(scope).display().to_string();
        }
        self.scope = scope;
        self.notice = None;
    }

    fn install_plan(&self) -> InstallPlan {
        let dir = PathBuf::from(self.dir.trim());
        let same_type = if self.scope == Scope::Machine { &self.machine_install } else { &self.user_install };
        let program = ProgramPlan {
            scope: self.scope,
            previous_dir: same_type.as_ref().map(|i| i.dir.clone()).filter(|previous| !same_path(previous, &dir)),
            dir,
            lang: self.lang,
            start_menu: self.start_menu,
            desktop: self.desktop,
        };
        let mut replaces: Vec<(&'static Product, Installed)> = Vec::new();
        if let Some(old) = &self.legacy_user {
            replaces.push((&LEGACY, old.clone()));
        }
        if self.scope == Scope::Machine {
            if let Some(own) = &self.user_install {
                replaces.push((&CURRENT, own.clone()));
            }
        }
        InstallPlan { program, ea_addin: self.ea_addin, config: self.config.clone(), replaces }
    }

    /// Language change on the first page: the wizard, license and changelog switch to the new language at once.
    fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
        i18n::set(lang);
        self.license = text_of(payload_license(lang));
        self.news = whats_new(&text_of(payload_changelog(lang)), self.installed_version.as_deref(), app_version());
    }

    fn start_work(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        let hwnd = wizard_window();
        self.progress = Progress::default();
        self.shown = 0.0;
        self.notice = None;
        self.outcome = None;
        let progress = self.progress.clone();
        if self.uninstall {
            let target = Installed { scope: self.scope, dir: PathBuf::from(self.dir.trim()), version: None };
            let (data, secrets_too) = (self.remove_data, self.remove_secrets);
            std::thread::spawn(move || {
                let _ = tx.send(uninstall(&target, data, secrets_too, hwnd, &progress).map_err(failure));
                wake.request_repaint();
            });
        } else {
            let plan = self.install_plan();
            std::thread::spawn(move || {
                let _ = tx.send(install(&plan, hwnd, &progress).map_err(failure));
                wake.request_repaint();
            });
        }
        self.work = Some(rx);
        self.page = Page::Working;
    }

    fn start_old_removal(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        let hwnd = wizard_window();
        std::thread::spawn(move || {
            let _ = tx.send(remove_legacy_machine(hwnd).map_err(failure));
            wake.request_repaint();
        });
        self.old_removal = OldRemoval::Running(rx);
    }

    /// Results of the background work.
    fn poll(&mut self) {
        if let Some(rx) = &self.work {
            if let Ok(result) = rx.try_recv() {
                self.work = None;
                match result {
                    Ok(report) => self.outcome = Some(Ok(report)),
                    Err(Failure::Error(error)) => self.outcome = Some(Err(error)),
                    Err(Failure::Cancelled) => {
                        // Nothing was done: back to the page where the choice was made, with an explanation.
                        self.notice = Some(if self.uninstall {
                            crate::i18n::tr("Administrator rights were not granted, so nothing was removed.").to_string()
                        } else {
                            crate::i18n::tr("Administrator rights were not granted, so nothing was installed. To install without them, choose \"Just for me\".").to_string()
                        });
                        self.page = if self.uninstall { Page::Welcome } else { Page::Folder };
                    }
                }
            }
        }
        if let OldRemoval::Running(rx) = &self.old_removal {
            if let Ok(result) = rx.try_recv() {
                self.legacy_machine = read_install(&LEGACY, Scope::Machine);
                self.old_removal = match result {
                    Ok(()) if self.legacy_machine.is_none() => OldRemoval::Removed,
                    // The old uninstaller was closed without removing the version.
                    Ok(()) => OldRemoval::Idle,
                    Err(Failure::Cancelled) => OldRemoval::Cancelled,
                    Err(Failure::Error(error)) => OldRemoval::Failed(error),
                };
            }
        }
    }
}

impl eframe::App for Setup {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // For all users without administrator rights, the elevated copy removes the uninstaller after this window closes.
        if self.uninstall && matches!(self.outcome, Some(Ok(_))) && (self.scope == Scope::User || elevated()) {
            let dir = PathBuf::from(self.dir.trim());
            if running_from(&dir) {
                remove_uninstaller_later(&dir);
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.page == Page::Working && ctx.input(|i| i.viewport().close_requested()) {
            // Closing in the middle of copying would leave a half-installed application.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if cfg!(debug_assertions) && self.page == Page::Welcome && std::env::var("LOONAR_SETUP_PAGE").as_deref() == Ok("working") {
            // Development build: preview of the progress page without installing (the bar stays at 45%).
            self.progress.set(0.45, &crate::i18n::fmt("Copying {0}…", &[(APP_EXE).to_string()]));
            self.shown = 0.45;
            self.page = Page::Working;
            self.work = Some(mpsc::channel().1);
        }
        if self.page == Page::Working {
            // The bar catches up with the actual progress smoothly (installation usually takes 1-3 s, jumps would be unreadable);
            // after completion it runs to the end and only then does the wizard move on. An error stops it at once.
            let (target, _) = self.progress.get();
            let target = if matches!(self.outcome, Some(Ok(_))) { 1.0 } else { target };
            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            self.shown = (self.shown + dt * 1.2).min(target).max(self.shown);
            match &self.outcome {
                Some(Err(_)) => self.page = Page::Finish,
                Some(Ok(_)) if self.shown >= 0.999 => self.page = Page::Finish,
                _ => ctx.request_repaint(),
            }
        }

        // Left strip with the Loonar artwork, as in classic installers.
        egui::SidePanel::left("banner").exact_width(200.0).resizable(false).frame(egui::Frame::none()).show(ctx, |ui| {
            let rect = ui.max_rect();
            let background = texture(ctx, "banner_bg", BANNER_BG);
            // Background crop with the strip's proportions, from the right part of the artwork (keyboard), so the image is not stretched.
            let uv = egui::Rect::from_min_max(egui::pos2(0.62, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(background.id(), rect, uv, egui::Color32::WHITE);
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(8, 30, 54, 90));
            let stars = texture(ctx, "banner_stars", BANNER_STARS);
            let stars_rect = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 26.0), egui::vec2(184.0, 137.0));
            ui.painter().image(stars.id(), stars_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            let white = egui::Color32::WHITE;
            let soft = egui::Color32::from_rgb(200, 216, 232);
            ui.painter().text(rect.left_top() + egui::vec2(16.0, 196.0), egui::Align2::LEFT_TOP, "Loonar", egui::FontId::new(26.0, egui::FontFamily::Name("bold".into())), white);
            ui.painter().text(rect.left_top() + egui::vec2(16.0, 228.0), egui::Align2::LEFT_TOP, "Compass", egui::FontId::proportional(20.0), white);
            ui.painter().text(rect.left_top() + egui::vec2(16.0, 258.0), egui::Align2::LEFT_TOP, crate::i18n::fmt("version {0}", &[app_version().to_string()]), egui::FontId::proportional(13.0), soft);
            ui.painter().text(rect.left_bottom() + egui::vec2(16.0, -38.0), egui::Align2::LEFT_TOP, "loonar.eu", egui::FontId::proportional(13.0), soft);
            ui.painter().text(rect.left_bottom() + egui::vec2(16.0, -20.0), egui::Align2::LEFT_TOP, "© 2026 Loonar", egui::FontId::proportional(11.0), soft);
        });

        // Bottom strip with the Back / Next / Cancel buttons (always visible: the page above scrolls).
        let mut go: Option<Page> = None;
        let mut start = false;
        let mut close = false;
        let old_removal_running = matches!(self.old_removal, OldRemoval::Running(_));
        egui::TopBottomPanel::bottom("buttons")
            .frame(egui::Frame::none().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(16.0, 12.0)))
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let working = self.page == Page::Working;
                    match self.page {
                        Page::Finish => {
                            if ui.button(crate::i18n::tr("Finish")).clicked() {
                                close = true;
                            }
                        }
                        _ => {
                            if ui.add_enabled(!working && !old_removal_running, egui::Button::new(crate::i18n::tr("Cancel"))).clicked() {
                                close = true;
                            }
                            let (next_label, next): (&str, Option<Page>) = match (self.uninstall, self.page) {
                                (true, Page::Welcome) => (crate::i18n::tr("Uninstall"), None),
                                (_, Page::Language) => (crate::i18n::tr("Next >"), Some(Page::Welcome)),
                                (false, Page::Welcome) => (crate::i18n::tr("Next >"), Some(Page::News)),
                                (_, Page::News) => (crate::i18n::tr("Next >"), Some(Page::License)),
                                (_, Page::License) => (crate::i18n::tr("Next >"), Some(Page::Folder)),
                                (_, Page::Folder) => (crate::i18n::tr("Next >"), Some(Page::Config)),
                                (_, Page::Config) => (crate::i18n::tr("Next >"), Some(Page::Summary)),
                                (_, Page::Summary) => (crate::i18n::tr("Install"), None),
                                _ => (crate::i18n::tr("Next >"), None),
                            };
                            let allowed = !working
                                && match self.page {
                                    Page::License => self.accepted,
                                    Page::Folder => check_folder(&self.dir, self.scope).is_ok(),
                                    Page::Config => self.config.problem().is_none(),
                                    Page::Summary => !old_removal_running,
                                    _ => true,
                                };
                            if ui.add_enabled(allowed, egui::Button::new(bold(next_label, 14.5))).clicked() {
                                match next {
                                    Some(page) => go = Some(page),
                                    None => start = true,
                                }
                            }
                            let back = match self.page {
                                Page::Welcome => Some(Page::Language),
                                Page::News => Some(Page::Welcome),
                                Page::License => Some(Page::News),
                                Page::Folder => Some(Page::License),
                                Page::Config => Some(Page::Folder),
                                Page::Summary => Some(Page::Config),
                                _ => None,
                            };
                            if !self.uninstall
                                && ui.add_enabled(back.is_some() && !working && !old_removal_running, egui::Button::new(crate::i18n::tr("< Back"))).clicked()
                            {
                                go = back;
                            }
                        }
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BACKGROUND).inner_margin(egui::Margin::symmetric(24.0, 20.0)))
            .show(ctx, |ui| {
                // Height of the visible page area: boxes with long texts (license, changelog) fit into it.
                let height = ui.available_height();
                egui::ScrollArea::vertical().id_salt(self.page).auto_shrink([false, false]).show(ui, |ui| match (self.uninstall, self.page) {
                    (_, Page::Language) => self.language_page(ui),
                    (true, Page::Welcome) => self.uninstall_page(ui),
                    (false, Page::Welcome) => self.welcome_page(ui),
                    (_, Page::News) => self.news_page(ui, height),
                    (_, Page::License) => self.license_page(ui, height),
                    (_, Page::Folder) => self.folder_page(ui),
                    (_, Page::Config) => self.config_page(ui),
                    (_, Page::Summary) => self.summary_page(ui),
                    (_, Page::Working) => self.working_page(ui),
                    (_, Page::Finish) => self.finish_page(ui),
                });
            });

        if let Some(page) = go {
            if page != self.page {
                self.notice = None;
            }
            self.page = page;
        }
        if start {
            self.start_work(ctx);
        }
        if close {
            if self.page == Page::Finish && !self.uninstall && self.run_after && matches!(self.outcome, Some(Ok(_))) {
                launch(&PathBuf::from(self.dir.trim()).join(APP_EXE));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Starts the installed application as the user who runs the wizard. When the wizard itself has administrator
/// rights, the system shell starts it, so the application runs as a regular user (otherwise it would not accept
/// files dragged from Explorer).
fn launch(app: &Path) {
    let _ = if elevated() {
        Command::new(windows_dir().join("explorer.exe")).arg(app).spawn()
    } else {
        Command::new(app).current_dir(app.parent().unwrap_or(Path::new("."))).spawn()
    };
}

impl Setup {
    /// First page: language selection (English by default, the saved one on an update).
    fn language_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(crate::i18n::tr("Choose language"), 21.0));
        ui.add_space(4.0);
        ui.add(egui::Label::new(crate::i18n::tr("Language of the application, the assistant's answers and the default document templates. You can change it later in Settings.")).wrap());
        ui.add_space(14.0);
        let mut chosen = self.lang;
        for lang in Lang::ALL {
            ui.radio_value(&mut chosen, lang, egui::RichText::new(lang.native_name()).size(16.0));
            ui.add_space(4.0);
        }
        if chosen != self.lang {
            self.set_lang(chosen);
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(window_title(self.uninstall)));
        }
    }

    fn welcome_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(&crate::i18n::fmt("Welcome to the {0} Setup Wizard", &[APP_NAME.to_string()]), 21.0));
        ui.add_space(12.0);
        ui.add(egui::Label::new(crate::i18n::fmt("This wizard will install {0} on your computer: an AI assistant for Enterprise Architect that reads the model from a PostgreSQL database or an EA file, saves changes through COM and shows diagrams in the chat.", &[APP_NAME.to_string()])).wrap());
        ui.add_space(10.0);
        ui.label(bold(crate::i18n::tr("Prerequisites:"), 14.5));
        for line in [
            crate::i18n::tr("• the EA model is a file (.qea, .qeax) or is in a PostgreSQL database,"),
            crate::i18n::tr("• a model in a database has user security enabled (Enable Security) and a user account for the assistant (any name)."),
        ] {
            ui.label(line);
        }
        ui.add_space(8.0);
        ui.label(bold(crate::i18n::tr("Before installing, have the following ready:"), 14.5));
        for line in [
            crate::i18n::tr("• a Claude API key (Anthropic Console),"),
            crate::i18n::tr("• the model file, or the database details: server, port, database name, user and password,"),
            crate::i18n::tr("• for a model in a database: the login and password of the assistant's account in the EA model,"),
            crate::i18n::tr("• Enterprise Architect, installed and licensed."),
        ] {
            ui.label(line);
        }
        ui.add_space(10.0);
        ui.add(egui::Label::new(crate::i18n::tr("You can enter these details in the wizard or later in the application, on the \"Settings\" tab. If the application is already installed, the wizard will update it and keep your settings and conversation history.")).wrap());
        ui.add_space(10.0);
        ui.label(egui::RichText::new(crate::i18n::tr("Click \"Next\" to continue.")).color(theme::MUTED));
    }

    fn news_page(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.label(bold(&crate::i18n::fmt("What's new in version {0}", &[app_version().to_string()]), 21.0));
        ui.label(match &self.installed_version {
            Some(old) if version_key(old) < version_key(app_version()) => crate::i18n::fmt("Changes since the installed version {0}.", &[(old).to_string()]),
            Some(old) => crate::i18n::fmt("Version {0} is already installed; below are the changes in this version.", &[(old).to_string()]),
            None => crate::i18n::tr("The main changes in this version.").to_string(),
        });
        ui.add_space(6.0);
        let news = if self.news.is_empty() { whats_new(&text_of(payload_changelog(self.lang)), None, app_version()) } else { self.news.clone() };
        egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, theme::BORDER)).rounding(6.0).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("news").max_height((height - 130.0).clamp(140.0, 360.0)).auto_shrink([false, false]).show(ui, |ui| {
                let several = news.len() > 1;
                for (head, lines) in &news {
                    if several {
                        ui.label(bold(head, 15.5));
                        ui.add_space(2.0);
                    }
                    for line in lines {
                        let text = line.replace('`', "");
                        if text.is_empty() {
                            ui.add_space(4.0);
                        } else if let Some(item) = text.strip_prefix("- ") {
                            ui.add(egui::Label::new(egui::RichText::new(format!("•  {item}")).size(13.0)).wrap());
                        } else if let Some(sub) = text.strip_prefix("### ") {
                            ui.label(bold(sub, 14.0));
                        } else if text.ends_with(':') {
                            ui.label(bold(&text, 14.0));
                        } else {
                            ui.add(egui::Label::new(egui::RichText::new(text).size(13.0)).wrap());
                        }
                    }
                    ui.add_space(8.0);
                }
            });
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new(crate::i18n::tr("After installation, the full list of changes is in the application, in the \"Changelog\" view.")).small().color(theme::MUTED));
    }

    fn license_page(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.label(bold(crate::i18n::tr("License agreement"), 21.0));
        ui.label(crate::i18n::tr("Please read the license terms. You must accept them to install the application."));
        ui.add_space(6.0);
        egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, theme::BORDER)).rounding(6.0).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("license").max_height((height - 170.0).clamp(120.0, 320.0)).auto_shrink([false, false]).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&self.license).size(13.0)).wrap());
            });
        });
        ui.add_space(8.0);
        ui.radio_value(&mut self.accepted, true, crate::i18n::tr("I accept the terms of the license agreement"));
        ui.radio_value(&mut self.accepted, false, crate::i18n::tr("I do not accept the terms of the license agreement"));
    }

    fn folder_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(crate::i18n::tr("Destination folder"), 21.0));
        ui.add_space(8.0);
        ui.label(bold(crate::i18n::tr("Installation type"), 14.5));
        let mut scope = self.scope;
        ui.radio_value(&mut scope, Scope::User, crate::i18n::tr("Just for me (recommended, no administrator rights)"));
        ui.radio_value(&mut scope, Scope::Machine, crate::i18n::tr("For all users of this computer (requires administrator rights)"));
        if scope != self.scope {
            self.set_scope(scope);
        }
        if let Some(notice) = &self.notice {
            ui.add_space(4.0);
            error_text(ui, notice);
        }
        ui.add_space(12.0);
        ui.label(crate::i18n::tr("The application will be installed in the folder below."));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width((ui.available_width() - 110.0).max(200.0)));
            if ui.button(crate::i18n::tr("Browse…")).clicked() {
                let mut dialog = rfd::FileDialog::new().set_title(crate::i18n::tr("Installation folder"));
                if let Some(parent) = Path::new(self.dir.trim()).parent().filter(|p| p.is_dir()) {
                    dialog = dialog.set_directory(parent);
                }
                if let Some(folder) = dialog.pick_folder() {
                    // The chosen parent folder gets the application subfolder, unless it already has that name.
                    let folder = if folder.file_name().is_some_and(|n| n == APP_NAME) { folder } else { folder.join(APP_NAME) };
                    self.dir = folder.display().to_string();
                }
            }
        });
        if let Err(problem) = check_folder(&self.dir, self.scope) {
            ui.add(egui::Label::new(egui::RichText::new(problem).small().color(theme::ERROR)).wrap());
        }
        ui.label(egui::RichText::new(crate::i18n::fmt("Required disk space: about {0} MB.", &[required_mb().to_string()])).small().color(theme::MUTED));
        ui.add_space(14.0);
        ui.label(bold(crate::i18n::tr("Shortcuts"), 14.5));
        ui.checkbox(&mut self.start_menu, crate::i18n::tr("Start menu (application, user guide, uninstall)"));
        ui.checkbox(&mut self.desktop, crate::i18n::tr("Desktop"));
        ui.add_space(10.0);
        ui.label(bold(crate::i18n::tr("Enterprise Architect"), 14.5));
        ui.checkbox(&mut self.ea_addin, crate::i18n::tr("Add Loonar Compass to Enterprise Architect (a tab next to the model, button on the Start ribbon)"));
        muted(ui, crate::i18n::tr("Works with any Enterprise Architect installation, also a trial. Restart EA after the installation."));
        ui.add_space(6.0);
        muted(ui, crate::i18n::tr("Windows 11 does not allow installers to pin programs to the taskbar. After installation, start the application, right-click its icon on the taskbar and choose \"Pin to taskbar\"."));
        ui.add_space(6.0);
        muted(ui, &crate::i18n::fmt("Settings, conversation history and attachments are separate for each user: {0}", &[(config::user_data_dir().display()).to_string()]));
    }

    fn config_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(crate::i18n::tr("Configuration"), 21.0));
        ui.radio_value(&mut self.config.configure_now, true, crate::i18n::tr("Enter the details now"));
        ui.radio_value(&mut self.config.configure_now, false, crate::i18n::tr("Configure later in the application (\"Settings\" view)"));
        ui.add_space(4.0);
        let problem = self.config.problem();
        ui.add_enabled_ui(self.config.configure_now, |ui| {
            let c = &mut self.config;
            ui.label(bold("Claude", 14.5));
            egui::Grid::new("cfg_claude").num_columns(2).min_col_width(190.0).spacing([10.0, 6.0]).show(ui, |ui| {
                field(ui, crate::i18n::tr("Claude API key"), &mut c.api_key, true);
            });
            muted(ui, crate::i18n::tr("To answer, the app sends the conversation, attachments, remembered notes, templates, model query results and diagram images to Anthropic (Claude API) using your key. Loonar does not receive this data."));
            ui.add_space(6.0);
            ui.label(bold(crate::i18n::tr("Model"), 14.5));
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut c.model_source, "postgres".to_string(), crate::i18n::tr("PostgreSQL database"));
                ui.radio_value(&mut c.model_source, "file".to_string(), crate::i18n::tr("EA file (.qea, .qeax)"));
            });
            if c.model_source == "file" {
                egui::Grid::new("cfg_file").num_columns(2).min_col_width(190.0).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Model file"));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut c.model_file).desired_width(250.0));
                        if ui.button("…").clicked() {
                            if let Some(file) = rfd::FileDialog::new()
                                .set_title(crate::i18n::tr("Choose the model file"))
                                .add_filter(crate::i18n::tr("Enterprise Architect project"), &config::MODEL_FILE_EXTENSIONS)
                                .pick_file()
                            {
                                c.model_file = file.display().to_string();
                            }
                        }
                    });
                    ui.end_row();
                });
            } else {
                egui::Grid::new("cfg_db").num_columns(2).min_col_width(190.0).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Server"));
                    ui.add(egui::TextEdit::singleline(&mut c.db_host).hint_text(crate::i18n::tr("e.g. 127.0.0.1")).desired_width(250.0));
                    ui.end_row();
                    field(ui, crate::i18n::tr("Port"), &mut c.db_port, false);
                    field(ui, crate::i18n::tr("Database name"), &mut c.db_name, false);
                    field(ui, crate::i18n::tr("Database user"), &mut c.db_user, false);
                    field(ui, crate::i18n::tr("Database password"), &mut c.db_password, true);
                });
            }
            if let Some(problem) = problem {
                ui.label(egui::RichText::new(problem).small().color(theme::ERROR));
            }
            ui.add_space(6.0);
            ui.label(bold("Enterprise Architect", 14.5));
            egui::Grid::new("cfg_ea").num_columns(2).min_col_width(190.0).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(crate::i18n::tr("Path to EA.exe"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut c.ea_path).desired_width(250.0));
                    if ui.button("…").clicked() {
                        if let Some(file) = rfd::FileDialog::new().set_title(crate::i18n::tr("Choose EA.exe")).add_filter("EA.exe", &["exe"]).pick_file() {
                            c.ea_path = file.display().to_string();
                        }
                    }
                });
                ui.end_row();
                field(ui, crate::i18n::tr("EA user account"), &mut c.ea_user, false);
                if c.model_source == "file" {
                    ui.label("");
                    ui.label(egui::RichText::new(crate::i18n::tr("only if the file has security enabled")).small().color(theme::MUTED));
                    ui.end_row();
                }
                field(ui, crate::i18n::tr("EA account password"), &mut c.ea_password, true);
            });
            ui.add_space(6.0);
            ui.label(bold(crate::i18n::tr("Approving changes to the model"), 14.5));
            egui::Grid::new("cfg_mode").num_columns(2).min_col_width(190.0).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(crate::i18n::tr("Approval mode"));
                egui::ComboBox::from_id_salt("mode").width(250.0).selected_text(mode_label(&c.approval_mode)).show_ui(ui, |ui| {
                    for mode in ["ask", "plan", "auto"] {
                        ui.selectable_value(&mut c.approval_mode, mode.to_string(), mode_label(mode));
                    }
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            muted(ui, crate::i18n::tr("Leave a password or key field empty to keep the previously saved value. You can test the details in the application: Settings → \"Save and test…\"."));
        });
    }

    fn summary_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(crate::i18n::tr("Ready to install"), 21.0));
        ui.label(crate::i18n::tr("Check the settings. Click \"Install\" to start, or \"Back\" to change something."));
        ui.add_space(10.0);
        egui::Frame::none().fill(theme::PANEL).stroke(egui::Stroke::new(1.0_f32, theme::BORDER)).rounding(6.0).inner_margin(egui::Margin::same(12.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(bold(crate::i18n::tr("Installation type"), 14.5));
            ui.label(self.scope.label());
            ui.add_space(6.0);
            ui.label(bold(crate::i18n::tr("Installation folder"), 14.5));
            ui.label(self.dir.trim());
            ui.add_space(6.0);
            ui.label(bold(crate::i18n::tr("Shortcuts"), 14.5));
            let mut shortcuts = Vec::new();
            if self.start_menu {
                shortcuts.push(crate::i18n::tr("Start menu"));
            }
            if self.desktop {
                shortcuts.push(crate::i18n::tr("desktop"));
            }
            ui.label(if shortcuts.is_empty() { crate::i18n::tr("none").to_string() } else { shortcuts.join(", ") });
            ui.add_space(6.0);
            ui.label(bold(crate::i18n::tr("Configuration"), 14.5));
            if self.config.configure_now {
                let c = &self.config;
                if c.uses_file() {
                    ui.label(crate::i18n::fmt("Model file: {0}", &[c.model_file.trim().to_string()]));
                } else {
                    ui.label(crate::i18n::fmt("Database: {0}@{1}:{2} (user {3})", &[(c.db_name.trim()).to_string(), (c.db_host.trim()).to_string(), (c.db_port.trim()).to_string(), (c.db_user.trim()).to_string()]));
                }
                ui.label(crate::i18n::fmt("EA account: {0}", &[(c.ea_user.trim()).to_string()]));
                ui.label(crate::i18n::fmt("Approval mode: {0}", &[(mode_label(&c.approval_mode)).to_string()]));
                ui.label(crate::i18n::fmt("API key: {0}", &[(if c.api_key.trim().is_empty() { crate::i18n::tr("unchanged") } else { crate::i18n::tr("entered") }).to_string()]));
            } else {
                ui.label(crate::i18n::tr("Later, in the application's \"Settings\" view."));
            }
        });
        if self.scope == Scope::Machine && !elevated() {
            ui.add_space(8.0);
            muted(ui, crate::i18n::tr("Windows will ask for administrator rights when you click \"Install\"."));
        }
        if let (Scope::User, Some(machine)) = (self.scope, &self.machine_install) {
            ui.add_space(8.0);
            muted(ui, &crate::i18n::fmt("{0} is also installed for all users in {1}. This installation just for you does not replace it.", &[APP_NAME.to_string(), machine.dir.display().to_string()]));
        }
        self.old_version_note(ui);
    }

    /// Note about the pre-release version installed for all users: it can stay, or be removed with its own uninstaller.
    fn old_version_note(&mut self, ui: &mut egui::Ui) {
        let status = match &self.old_removal {
            OldRemoval::Removed => Some((crate::i18n::tr("The earlier version has been removed.").to_string(), theme::SUCCESS)),
            OldRemoval::Cancelled => Some((crate::i18n::tr("Administrator rights were not granted; the earlier version was not removed.").to_string(), theme::ERROR)),
            OldRemoval::Failed(error) => Some((error.clone(), theme::ERROR)),
            OldRemoval::Idle | OldRemoval::Running(_) => None,
        };
        let Some(old) = self.legacy_machine.clone() else {
            if let Some((text, color)) = status {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).color(color));
            }
            return;
        };
        ui.add_space(10.0);
        let running = matches!(self.old_removal, OldRemoval::Running(_));
        let mut remove = false;
        egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, theme::BORDER)).rounding(6.0).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let name = match &old.version {
                Some(version) => format!("{} {version}", LEGACY.name),
                None => LEGACY.name.to_string(),
            };
            ui.add(egui::Label::new(crate::i18n::fmt("An earlier version ({0}) is installed for all users in {1}. It is no longer needed: you can remove it now or later in Settings → Apps. Your settings and conversations are kept.", &[name, old.dir.display().to_string()])).wrap());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!running, egui::Button::new(crate::i18n::tr("Remove the old version (administrator rights)"))).clicked() {
                    remove = true;
                }
                if running {
                    ui.spinner();
                    ui.label(egui::RichText::new(crate::i18n::tr("Removing the earlier version…")).color(theme::MUTED));
                }
            });
            if let Some((text, color)) = &status {
                ui.add(egui::Label::new(egui::RichText::new(text).small().color(*color)).wrap());
            }
        });
        if remove {
            self.start_old_removal(ui.ctx());
        }
    }

    fn working_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(if self.uninstall { crate::i18n::tr("Uninstalling") } else { crate::i18n::tr("Installing") }, 21.0));
        ui.label(if self.uninstall {
            crate::i18n::fmt("Removing {0} from this computer.", &[(APP_NAME).to_string()])
        } else {
            crate::i18n::fmt("Installing {0}. Please wait.", &[(APP_NAME).to_string()])
        });
        ui.add_space(40.0);
        let (_, step) = self.progress.get();
        ui.horizontal(|ui| {
            ui.label(if self.shown >= 0.999 { crate::i18n::tr("Done").to_string() } else { step });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{:.0}%", self.shown * 100.0)).color(theme::MUTED));
            });
        });
        ui.add_space(6.0);
        // Classic bar: grey background with a border and a fill in the accent color.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect(rect, 3.0, theme::BUTTON, egui::Stroke::new(1.0_f32, theme::BORDER));
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * self.shown, rect.height())).shrink(2.0);
        if fill.width() > 0.0 {
            painter.rect_filled(fill, 2.0, theme::ACCENT);
        }
    }

    fn finish_page(&mut self, ui: &mut egui::Ui) {
        match &self.outcome {
            Some(Ok(report)) => {
                ui.label(bold(if self.uninstall { crate::i18n::tr("Uninstallation complete") } else { crate::i18n::tr("Installation complete") }, 21.0));
                ui.add_space(10.0);
                for line in &report.done {
                    ui.add(egui::Label::new(egui::RichText::new(format!("✓ {line}")).color(theme::SUCCESS)).wrap());
                }
                for line in &report.warnings {
                    ui.add(egui::Label::new(egui::RichText::new(format!("! {line}")).color(theme::ERROR)).wrap());
                }
                if !self.uninstall {
                    ui.add_space(14.0);
                    ui.checkbox(&mut self.run_after, crate::i18n::fmt("Launch {0}", &[APP_NAME.to_string()]));
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr("The user guide is on loonar.eu: in the Start menu, choose \"User guide\". After the first start, check the details on the \"Settings\" tab with the \"Save and test\" buttons.")).color(theme::MUTED)).wrap());
                }
            }
            Some(Err(error)) => {
                ui.label(bold(if self.uninstall { crate::i18n::tr("Uninstallation failed") } else { crate::i18n::tr("Installation failed") }, 21.0));
                ui.add_space(10.0);
                error_text(ui, error);
            }
            None => {}
        }
    }

    fn uninstall_page(&mut self, ui: &mut egui::Ui) {
        ui.label(bold(&crate::i18n::fmt("Uninstall {0}", &[APP_NAME.to_string()]), 21.0));
        ui.add_space(10.0);
        ui.label(crate::i18n::tr("The application will be removed from the following folder:"));
        ui.label(bold(self.dir.trim(), 14.5));
        if self.scope == Scope::Machine && !elevated() {
            ui.add_space(4.0);
            muted(ui, crate::i18n::tr("This installation is for all users of this computer: Windows will ask for administrator rights."));
        }
        if let Some(notice) = &self.notice {
            ui.add_space(6.0);
            error_text(ui, notice);
        }
        ui.add_space(14.0);
        ui.label(crate::i18n::tr("Your data stays on the computer unless you choose otherwise:"));
        ui.checkbox(&mut self.remove_data, crate::i18n::tr("Remove settings, conversations, attachments, memory, templates and the audit log"));
        ui.indent("data_folder", |ui| muted(ui, &crate::i18n::fmt("This deletes the whole folder {0}.", &[config::user_data_dir().display().to_string()])));
        ui.checkbox(&mut self.remove_secrets, crate::i18n::tr("Remove the API key, passwords and tokens from Credential Manager"));
        if self.scope == Scope::Machine {
            ui.add_space(4.0);
            muted(ui, crate::i18n::tr("Only your own data is removed; the data of other users stays in their profiles."));
        }
        ui.add_space(14.0);
        ui.label(egui::RichText::new(crate::i18n::tr("Close the application before uninstalling.")).color(theme::MUTED));
    }
}

fn mode_label(mode: &str) -> &'static str {
    match mode {
        "plan" => crate::i18n::tr("plan - asks once per assistant answer"),
        "auto" => crate::i18n::tr("auto - saves without asking"),
        _ => crate::i18n::tr("ask - asks before every save"),
    }
}

#[cfg(test)]
mod tests {
    /// Registers the add-in of the installed copy (manual: `cargo test register_installed_addin -- --ignored`).
    #[test]
    #[ignore]
    fn register_installed_addin() {
        let dll = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join(r"Programs\Loonar Compass").join(super::ADDIN_DLL);
        super::ea_addin::register(&dll).expect("registration");
        println!("{:?}", super::reg::get_string(super::reg::Root::CurrentUser, r"Software\Classes\CLSID\{6C1E5B7A-3D2F-4B8E-9A41-2F7C0D5E8B13}\InprocServer32", "CodeBase"));
    }

    use super::*;

    const LOG: &str = "# Changelog\n\nIntroduction.\n\n## Version 0.3 (2026-11-01)\n\n- three\n\n## Version 0.2 (2026-10-15)\n\nGroup:\n\n- two\n\n## Version 0.1 (2026-10-13)\n\n- one\n";

    fn heads(installed: Option<&str>, current: &str) -> Vec<String> {
        whats_new(LOG, installed, current).into_iter().map(|(h, _)| h).collect()
    }

    #[test]
    fn whats_new_picks_versions_after_installed() {
        // Sections newer than the setup's own version (here 0.3 when the setup is 0.2) are skipped.
        assert_eq!(heads(None, "0.2"), vec!["Version 0.2 (2026-10-15)"]);
        assert_eq!(heads(Some("0.1"), "0.2"), vec!["Version 0.2 (2026-10-15)"]);
        assert!(heads(Some("0.2"), "0.2").is_empty());
        assert!(heads(Some("0.2.0"), "0.2").is_empty());
        assert_eq!(heads(Some("0.1"), "0.3"), vec!["Version 0.3 (2026-11-01)", "Version 0.2 (2026-10-15)"]);
        assert_eq!(whats_new(LOG, None, "0.2")[0].1, vec!["Group:", "", "- two"]);
    }

    #[test]
    fn versions_are_shown_and_compared_like_the_build_script() {
        assert_eq!(display_version("0.3.0"), "0.3");
        assert_eq!(display_version("0.3.1"), "0.3.1");
        assert_eq!(display_version("2.10.0"), "2.10");
        assert_eq!(display_version("1.0.0-beta.1"), "1.0-beta.1");
        assert_eq!(app_version(), display_version(env!("CARGO_PKG_VERSION")));
        assert!(version_key("0.10") > version_key("0.2"));
        assert_eq!(version_key("0.3.0"), version_key("0.3"));
        assert!(version_key("0.3.1") > version_key("0.3"));
        assert!(version_key("1.0-beta.1") == version_key("1.0"));
    }

    #[test]
    fn embedded_changelogs_describe_this_version() {
        for lang in Lang::ALL {
            assert!(!whats_new(&text_of(payload_changelog(lang)), None, app_version()).is_empty(), "CHANGELOG {lang:?} has no section for {}", app_version());
        }
    }

    #[test]
    fn installation_folder_rules() {
        let pf = program_files();
        let ok = |p: &str, s| check_folder(p, s).is_ok();
        let text = |p: PathBuf| p.display().to_string();
        assert!(ok(&text(pf.join(APP_NAME)), Scope::Machine));
        assert!(ok(&text(pf.join(APP_NAME)).to_uppercase(), Scope::Machine));
        assert!(ok(&text(Scope::Machine.default_dir()), Scope::Machine));
        assert!(ok(&text(Scope::User.default_dir()), Scope::User));
        // All users: only below Program Files, never Program Files itself or a way out of it.
        assert!(!ok(&text(pf.clone()), Scope::Machine));
        assert!(!ok(&format!(r"{}\..\Windows\Loonar Compass", pf.display()), Scope::Machine));
        assert!(!ok(r"D:\Tools\Loonar Compass", Scope::Machine));
        // Just for me: anywhere the user can write, but not the administrators' folders.
        assert!(ok(r"D:\Tools\Loonar Compass", Scope::User));
        assert!(ok(r"\\server\share\Loonar Compass", Scope::User));
        assert!(!ok(&text(pf.join(APP_NAME)), Scope::User));
        assert!(!ok(&format!(r"{} \Loonar Compass", pf.display()), Scope::User));
        assert!(!ok(&text(windows_dir().join(APP_NAME)), Scope::User));
        // Not a full path of a folder.
        for bad in ["", "Loonar Compass", r"C:\", r"C:Loonar", r"C:\a<b\c", r"C:\Tools\.."] {
            assert!(!ok(bad, Scope::User), "{bad}");
        }
    }

    #[test]
    fn paths_compare_case_insensitively() {
        assert!(same_path(Path::new(r"C:\Program Files\Loonar Compass"), Path::new(r"c:\program files\LOONAR COMPASS\")));
        assert!(!same_path(Path::new(r"C:\Program Files\Loonar Compass"), Path::new(r"C:\Program Files\Loonar EA Assistant")));
    }

    #[test]
    fn uninstall_command_parsing() {
        assert_eq!(
            uninstaller_from_command("\"C:\\Program Files\\Loonar EA Assistant\\uninstall.exe\" --uninstall"),
            Some(PathBuf::from(r"C:\Program Files\Loonar EA Assistant\uninstall.exe"))
        );
        assert_eq!(uninstaller_from_command(r"C:\Program Files\X\Uninstall.EXE --uninstall"), Some(PathBuf::from(r"C:\Program Files\X\Uninstall.EXE")));
        assert_eq!(uninstaller_from_command("uninstall.exe --uninstall"), None);
        assert_eq!(uninstaller_from_command(""), None);
    }

    #[test]
    fn machine_task_round_trip() {
        let install = MachineTask::Install(ProgramPlan {
            scope: Scope::Machine,
            dir: PathBuf::from(r"C:\Program Files\Loonar Compass"),
            lang: Lang::De,
            start_menu: true,
            desktop: false,
            previous_dir: Some(PathBuf::from(r"C:\Program Files\Old = folder")),
        });
        assert_eq!(MachineTask::parse(&install.to_text()), Some(install));
        let uninstall = MachineTask::Uninstall { dir: PathBuf::from(r"C:\Program Files\Loonar Compass"), lang: Lang::Fr, parent_pid: 4242 };
        assert_eq!(MachineTask::parse(&uninstall.to_text()), Some(uninstall));
        assert_eq!(MachineTask::parse("task=format\ndir=C:\\\nlang=en\n"), None);
        assert_eq!(MachineTask::parse(""), None);
    }

    #[test]
    fn results_are_complete_or_ignored() {
        let ok: Result<Vec<String>, String> = Ok(vec!["one".into(), "two".into()]);
        assert_eq!(parse_result(&result_text(&ok)), Some(ok));
        let error: Result<Vec<String>, String> = Err("cannot write x".into());
        assert_eq!(parse_result(&result_text(&error)), Some(error));
        // A file read while it is still being written.
        assert_eq!(parse_result("ok\none\n"), None);
        assert_eq!(parse_result(""), None);
        assert_eq!(parse_progress("0.4200\tCopying…"), Some((0.42, "Copying…".to_string())));
        assert_eq!(parse_progress("0.4"), None);
    }

    #[test]
    fn progress_parts_report_into_their_range() {
        let progress = Progress::default();
        let part = progress.part(0.2, 0.6);
        part.set(0.5, "x");
        assert!((progress.get().0 - 0.4).abs() < 1e-6);
        part.part(0.5, 1.0).set(1.0, "y");
        assert!((progress.get().0 - 0.6).abs() < 1e-6);
        assert_eq!(progress.get().1, "y");
    }

    #[test]
    fn install_date_and_cleanup_command() {
        assert_eq!(format_date(2026, 3, 7), "20260307");
        let line = cleanup_command(Path::new(r"C:\Users\A B\AppData\Local\Programs\Loonar Compass"), Path::new(r"C:\Windows\System32")).unwrap();
        assert!(line.starts_with("\"\"C:\\Windows\\System32\\PING.EXE\" "), "{line}");
        assert!(line.contains("del /f /q \"C:\\Users\\A B\\AppData\\Local\\Programs\\Loonar Compass\\uninstall.exe\""), "{line}");
        assert!(line.ends_with("rmdir \"C:\\Users\\A B\\AppData\\Local\\Programs\\Loonar Compass\"\""), "{line}");
        assert_eq!(cleanup_command(Path::new(r"C:\100%\Loonar Compass"), Path::new(r"C:\Windows\System32")), None);
    }
}
