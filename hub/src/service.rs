//! Compass Hub as a Windows service ("CompassHub"): installed with `--service-install <hub.json>` (as administrator),
//! started with Windows, stopped by the service manager. The service runs `compass-hub --service <hub.json>`.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::sync::OnceLock;
use std::time::Duration;
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
    ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

const NAME: &str = "CompassHub";
const DISPLAY: &str = "Compass Hub (Loonar Compass team service)";

static FILE: OnceLock<String> = OnceLock::new();
/// The status handle, so the control handler can report the stop before the process ends.
static STATUS: OnceLock<service_control_handler::ServiceStatusHandle> = OnceLock::new();

pub fn install(file: &str) -> Result<()> {
    let file = std::fs::canonicalize(file).with_context(|| format!("{file} does not exist"))?;
    let file = file.display().to_string();
    let file = file.trim_start_matches(r"\\?\").to_string();
    let exe = std::env::current_exe()?;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
        .context("the service manager (run as administrator)")?;
    let info = ServiceInfo {
        name: OsString::from(NAME),
        display_name: OsString::from(DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("--service"), OsString::from(&file)],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START).context("creating the service")?;
    service.set_description("Team models, policy, audit trail, change sets, web view and MCP server of Loonar Compass.")?;
    service.start::<&str>(&[]).context("starting the service")?;
    println!("Service {NAME} installed and started with {file}. It starts with Windows; stop it in Services or with: sc stop {NAME}");
    println!("Secrets for the service: system environment variables LOONAR_SECRET_<NAME> (or Credential Manager of the service account).");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).context("the service manager (run as administrator)")?;
    let service = manager.open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE).context("the service is not installed")?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        let _ = service.stop();
        std::thread::sleep(Duration::from_secs(2));
    }
    service.delete()?;
    println!("Service {NAME} removed.");
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

pub fn run(file: String) {
    let _ = FILE.set(file);
    if let Err(e) = service_dispatcher::start(NAME, ffi_service_main) {
        eprintln!("not started by the service manager: {e}");
    }
}

fn status(state: ServiceState, code: u32) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running { ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN } else { ServiceControlAccept::empty() },
        exit_code: ServiceExitCode::Win32(code),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    }
}

fn service_main(_: Vec<OsString>) {
    let status_handle = match service_control_handler::register(NAME, |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            // The hub keeps no state in memory that is not on disk: stopping ends the process. The service manager
            // is told first (stop pending, then stopped), so a stop or a restart is a normal stop and not a crash.
            if let Some(handle) = STATUS.get() {
                let _ = handle.set_service_status(status(ServiceState::StopPending, 0));
                let _ = handle.set_service_status(status(ServiceState::Stopped, 0));
            }
            std::process::exit(0);
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    }) {
        Ok(handle) => handle,
        Err(_) => return,
    };
    let _ = STATUS.set(status_handle);
    let _ = status_handle.set_service_status(status(ServiceState::Running, 0));
    let file = FILE.get().cloned().unwrap_or_default();
    let result = crate::run(&file);
    let _ = status_handle.set_service_status(status(ServiceState::Stopped, if result.is_ok() { 0 } else { 1 }));
}
