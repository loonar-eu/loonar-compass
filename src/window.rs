//! Opening the window with a fallback rendering method. OpenGL first; when the system does not provide it
//! (typical on Windows Server in a virtual machine or over remote desktop, where there is only software
//! OpenGL 1.1), the window is rendered with DirectX 12, which also works without a graphics card. When both fail,
//! the program shows a message with the cause and saves it to a file instead of closing without a trace.

/// Starts the application window. `options` and `make` are called separately for each attempt.
pub fn run<A: eframe::App + 'static>(
    title: &str,
    options: impl Fn() -> eframe::NativeOptions,
    make: impl Fn(&eframe::CreationContext<'_>) -> A + Clone + 'static,
) -> eframe::Result {
    let mut errors = Vec::new();
    // LOONAR_RENDERER=wgpu skips OpenGL (to test the fallback path on a computer that does have OpenGL).
    let renderers: &[eframe::Renderer] = if std::env::var("LOONAR_RENDERER").as_deref() == Ok("wgpu") {
        &[eframe::Renderer::Wgpu]
    } else {
        &[eframe::Renderer::Glow, eframe::Renderer::Wgpu]
    };
    for &renderer in renderers {
        let mut native = options();
        native.renderer = renderer;
        // Graphics card requirements = exactly what the card reports. The library's default, fixed requirements
        // exceed what some cards can do (e.g. in Wine on macOS via Vulkan/MoltenVK: 4 storage textures,
        // while the card reports 2), and any limits are enough to draw the UI.
        native.wgpu_options.device_descriptor = std::sync::Arc::new(|adapter| wgpu::DeviceDescriptor {
            label: Some("loonar-compass"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::default(),
        });
        let make = make.clone();
        match eframe::run_native(title, native, Box::new(move |cc| Ok(Box::new(make(cc)) as Box<dyn eframe::App>))) {
            Ok(()) => return Ok(()),
            Err(e) => errors.push(format!("{renderer:?}: {e}")),
        }
    }
    let log = std::env::temp_dir().join("loonar-compass-start.log");
    let details = errors.join("\n");
    let _ = std::fs::write(&log, format!("{}\n{details}\n", chrono_free_stamp()));
    show_error(
        title,
        &crate::i18n::fmt("Could not open the program window: the system provides neither OpenGL 2.0 nor DirectX 12.\n\nThe most common cause: a server or virtual machine without a graphics driver, or a remote desktop with graphics support turned off.\n\nDetails:\n{0}\n\nAlso saved to the file:\n{1}", &[details.clone(), log.display().to_string()]),
    );
    Err(eframe::Error::AppCreation(details.into()))
}

/// Timestamp without extra libraries (seconds since 1970), good enough for the diagnostic file.
fn chrono_free_stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("time (unix): {secs}")
}

/// Shows a message instead of disappearing silently when the program panics (release builds abort on panic,
/// which would close the window without a trace): writes the details to `%TEMP%\<log_name>` and shows them
/// in a system message box. Call once, at the start of `main`.
pub fn install_panic_hook(title: &'static str, log_name: &'static str, dialog: bool) {
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let details = format!("{} {}\n{message}\n{location}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let log = std::env::temp_dir().join(log_name);
        let _ = std::fs::write(&log, format!("{}\n{details}\n", chrono_free_stamp()));
        if !dialog {
            eprintln!("{details}");
            return;
        }
        show_error(
            title,
            &crate::i18n::fmt(
                "The program hit an unexpected error and has to close. Your conversations and settings are saved.\n\nPlease report the problem (Report a bug, or mariusz@loonar.eu) and attach the file:\n{0}\n\nDetails:\n{1}",
                &[log.display().to_string(), details],
            ),
        );
    }));
}

/// Marks the process as "system DPI aware" BEFORE the windowing library does (the first call wins).
/// `SetProcessDpiAwarenessContext` exists from Windows 10 1703; it is looked up at run time so the program
/// still starts on older systems (there the older `SetProcessDPIAware` is used).
pub fn use_system_dpi_awareness() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> Option<unsafe extern "system" fn(isize) -> i32>;
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDPIAware() -> i32;
    }
    const DPI_AWARENESS_CONTEXT_SYSTEM_AWARE: isize = -2;
    let user32: Vec<u16> = "user32.dll".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: zero-terminated names; the function pointer has the documented signature
    // BOOL SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT); called once, before any window exists.
    unsafe {
        let module = GetModuleHandleW(user32.as_ptr());
        match GetProcAddress(module, b"SetProcessDpiAwarenessContext\0".as_ptr()) {
            Some(set_context) if module != 0 => {
                set_context(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
            }
            _ => {
                SetProcessDPIAware();
            }
        }
    }
}

/// Windows message box (works without the program window and without 3D graphics).
pub fn show_error(title: &str, text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, flags: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide(title));
    const MB_ICONERROR: u32 = 0x10;
    // SAFETY: both buffers are zero-terminated and live until the end of the call.
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_ICONERROR);
    }
}

