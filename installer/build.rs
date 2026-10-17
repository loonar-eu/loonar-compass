//! Installer resources: icon, version info and a manifest that runs the setup with the user's own rights
//! (an installation just for the current user needs no administrator rights; for all users the wizard asks for
//! them itself, only for the machine-wide part). Also checks that the installer contents exist.

use std::path::Path;

/// Files that src/main.rs embeds with include_bytes!, prepared by build_installer.ps1.
fn payload_files() -> Vec<String> {
    let mut files = vec!["app.exe".to_string(), "THIRD-PARTY-NOTICES.txt".to_string()];
    for lang in ["pl", "en", "de", "fr"] {
        files.push(format!("LICENSE.{lang}.txt"));
        files.push(format!("CHANGELOG.{lang}.md"));
    }
    files
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../assets/icon.ico");
    println!("cargo:rerun-if-changed=payload");
    let missing: Vec<String> = payload_files().into_iter().filter(|f| !Path::new("payload").join(f).is_file()).collect();
    if !missing.is_empty() {
        panic!(
            "\n\nMissing installer contents in installer\\payload: {}\n\
             Run build_installer.ps1 in the repository root: it builds the application, generates\n\
             THIRD-PARTY-NOTICES.txt and copies the application, licenses and changelogs into the payload.\n\n",
            missing.join(", ")
        );
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // DLLs the program imports statically are loaded from System32 only, never from the folder the setup
        // was downloaded to (LOAD_LIBRARY_SEARCH_SYSTEM32); main() does the same for DLLs loaded later.
        println!("cargo:rustc-link-arg=/DEPENDENTLOADFLAG:0x800");
    }

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../assets/icon.ico");
    resource.set("ProductName", "Loonar Compass");
    resource.set("FileDescription", "Loonar Compass Setup");
    resource.set("CompanyName", "Loonar Mariusz Głowacki");
    resource.set("LegalCopyright", "© 2026 Mariusz Głowacki (Loonar)");
    resource.set_manifest(MANIFEST);
    resource.compile().expect("installer Windows resources");
}

/// asInvoker: the setup starts without a UAC prompt (and without the installer-detection heuristics);
/// supportedOS: Windows 10/11 behaviour (also Windows Server 2016 and newer).
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>"#;
