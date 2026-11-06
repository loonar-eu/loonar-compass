//! Embeds the application icon and version info into the exe file. This icon is used by Windows Explorer,
//! shortcuts and the taskbar once the application is pinned.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    build_identity();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // DLLs imported by the exe are loaded from System32 only, never from the program's or the current folder.
    println!("cargo:rustc-link-arg=/DEPENDENTLOADFLAG:0x800");
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon.ico");
    resource.set("ProductName", "Loonar Compass");
    resource.set("FileDescription", "Loonar Compass");
    resource.set("CompanyName", "Loonar Mariusz Głowacki");
    resource.set("LegalCopyright", "© 2026 Mariusz Głowacki (Loonar)");
    resource.compile().expect("Windows resources: icon and version info");
}

/// The build's own name and source (LC-483): LOONAR_BUILD (e.g. "0.9.3-dev27", given by build_installer.ps1 -Label)
/// and the short commit hash, with "+changes" when the working tree differs from it. Shown in Info and the footer, so
/// a tester knows what is installed (the version number changes only at a release).
fn build_identity() {
    println!("cargo:rerun-if-env-changed=LOONAR_BUILD");
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    // Rebuilt when the commit or the staged files change.
    for path in ["HEAD", "index"] {
        if let Some(p) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]).and_then(|r| git(&["rev-parse", "--git-path", &r])) {
        println!("cargo:rerun-if-changed={head}");
    }
    let commit = git(&["rev-parse", "--short=8", "HEAD"]).unwrap_or_default();
    // The add-in DLL and the notices are rewritten by the build itself: they do not count as changes.
    let dirty = git(&["status", "--porcelain", "--untracked-files=no", "--", ".", ":!eaaddin/LoonarCompass.EAAddin.dll", ":!THIRD-PARTY-NOTICES.txt"]).is_some_and(|s| !s.is_empty());
    let commit = if commit.is_empty() { String::new() } else if dirty { format!("{commit}+changes") } else { commit };
    println!("cargo:rustc-env=LOONAR_COMMIT={commit}");
    println!("cargo:rustc-env=LOONAR_BUILD={}", std::env::var("LOONAR_BUILD").unwrap_or_default());
}
