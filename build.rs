//! Embeds the application icon and version info into the exe file. This icon is used by Windows Explorer,
//! shortcuts and the taskbar once the application is pinned.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
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
