//! EA scripts (Scripting window: groups of JScript / VBScript / JavaScript scripts kept in the model's t_script table):
//! listing, reading, saving and running them. EA's API has no call that runs a script, so a script runs in Windows
//! Script Host attached to the user's open EA window: `Repository` and `Session.Output` work as inside EA, and
//! `!INC Group.Script` lines are resolved from the model.

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// Category of a script group row and of a script row in t_script.
pub const GROUP_CATEGORY: &str = "3955A83E-9E54-4810-8053-FACC68CD4782";
pub const SCRIPT_CATEGORY: &str = "605A62F7-BCD0-4845-A8D0-7DC45B4D2E3F";
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

pub struct Script {
    pub guid: String,
    pub name: String,
    pub language: String,
    pub group: String,
    pub code: String,
}

fn attr(xml: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    xml.find(&key).map(|i| &xml[i + key.len()..]).and_then(|rest| rest.find('"').map(|j| rest[..j].to_string())).unwrap_or_default()
}

/// Script groups: guid -> name.
fn groups(cfg: &AppConfig) -> Result<Vec<(String, String)>> {
    Ok(query_rows(cfg, &format!("select scriptname as c0, coalesce(script,'') as c1 from t_script where scriptcategory = '{GROUP_CATEGORY}' order by 2"))?
        .into_iter()
        .map(|r| (r[0].clone(), r[1].trim().to_string()))
        .collect())
}

pub fn all(cfg: &AppConfig) -> Result<Vec<Script>> {
    let groups = groups(cfg)?;
    Ok(query_rows(cfg, &format!(
        "select scriptname as c0, coalesce(scriptauthor,'') as c1, coalesce(notes,'') as c2, coalesce(script,'') as c3 from t_script where scriptcategory = '{SCRIPT_CATEGORY}'"
    ))?
    .into_iter()
    .map(|r| Script {
        guid: r[0].clone(),
        name: attr(&r[2], "Name"),
        language: attr(&r[2], "Language"),
        group: groups.iter().find(|g| g.0 == r[1]).map(|g| g.1.clone()).unwrap_or_default(),
        code: r[3].clone(),
    })
    .collect())
}

/// A script by "Group.Name" or by its name alone (when unique).
pub fn find(cfg: &AppConfig, name: &str) -> Result<Script> {
    let name = name.trim();
    let mut found: Vec<Script> = all(cfg)?
        .into_iter()
        .filter(|s| s.name.eq_ignore_ascii_case(name) || format!("{}.{}", s.group, s.name).eq_ignore_ascii_case(name))
        .collect();
    match found.len() {
        0 => bail!("{}", crate::i18n::fmt("there is no script {0} in the model", &[name.to_string()])),
        1 => Ok(found.remove(0)),
        _ => bail!("{}", crate::i18n::fmt("several scripts are called {0}: give Group.Name", &[name.to_string()])),
    }
}

pub fn list(cfg: &AppConfig) -> Result<String> {
    let scripts = all(cfg)?;
    let groups = groups(cfg)?;
    let mut out = vec![crate::i18n::fmt("Script groups: {0}; scripts: {1}.", &[groups.len().to_string(), scripts.len().to_string()])];
    out.push("group | script | language | lines".into());
    let mut sorted: Vec<&Script> = scripts.iter().collect();
    sorted.sort_by(|a, b| (a.group.as_str(), a.name.as_str()).cmp(&(b.group.as_str(), b.name.as_str())));
    for s in sorted {
        out.push(format!("{} | {} | {} | {}", s.group, s.name, s.language, s.code.lines().count()));
    }
    for (_, g) in groups.iter().filter(|g| !scripts.iter().any(|s| s.group == g.1)) {
        out.push(format!("{g} | (empty group) | | 0"));
    }
    Ok(out.join("\n"))
}

pub fn new_guid() -> String {
    format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase())
}

fn sql_text(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// SQL statements that create the group (when missing) and create or replace the script.
pub fn save_statements(cfg: &AppConfig, group: &str, name: &str, language: &str, code: &str) -> Result<(Vec<String>, bool)> {
    let language = match language.to_ascii_lowercase().as_str() {
        "vbscript" => "VBScript",
        "javascript" => "JavaScript",
        _ => "JScript",
    };
    if name.trim().is_empty() || name.contains('"') || name.contains('<') {
        bail!("{}", crate::i18n::tr("a script name must not be empty or contain quotes or <"));
    }
    let mut statements = Vec::new();
    let group_guid = match groups(cfg)?.into_iter().find(|g| g.1.eq_ignore_ascii_case(group.trim())) {
        Some(g) => g.0,
        None => {
            let guid = new_guid();
            statements.push(format!(
                "insert into t_script (scriptcategory, scriptname, scriptauthor, notes, script) values ('{GROUP_CATEGORY}', '{guid}', null, '<Group Type=\"NORMAL\" Notes=\"\"/>', {})",
                sql_text(group.trim())
            ));
            guid
        }
    };
    let notes = format!("<Script Name=\"{}\" Type=\"Internal\" Language=\"{language}\"/>", name.trim());
    let existing = all(cfg)?.into_iter().find(|s| s.name.eq_ignore_ascii_case(name.trim()) && s.group.eq_ignore_ascii_case(group.trim()));
    let updated = existing.is_some();
    match existing {
        Some(s) => statements.push(format!("update t_script set notes = {}, script = {} where scriptname = '{}'", sql_text(&notes), sql_text(code), s.guid)),
        None => statements.push(format!(
            "insert into t_script (scriptcategory, scriptname, scriptauthor, notes, script) values ('{SCRIPT_CATEGORY}', '{}', '{group_guid}', {}, {})",
            new_guid(),
            sql_text(&notes),
            sql_text(code)
        )),
    }
    Ok((statements, updated))
}

/// The script code with `!INC Group.Script` lines replaced by those scripts (once each, recursively).
pub fn resolve_includes(cfg: &AppConfig, script: &Script) -> Result<String> {
    fn go(cfg: &AppConfig, code: &str, seen: &mut Vec<String>, depth: usize) -> Result<String> {
        if depth > 10 {
            bail!("!INC nested too deeply");
        }
        let mut out = String::new();
        for line in code.lines() {
            if let Some(name) = line.trim().strip_prefix("!INC ") {
                let name = name.trim().to_string();
                if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                seen.push(name.clone());
                match find(cfg, &name) {
                    Ok(included) => {
                        out.push_str(&go(cfg, &included.code, seen, depth + 1)?);
                        out.push('\n');
                    }
                    // EA's built-in libraries (EAConstants-JScript and similar) are not in the model.
                    Err(_) => out.push_str(&format!("// !INC {name} (not in the model, skipped)\n")),
                }
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        Ok(out)
    }
    go(cfg, &script.code, &mut vec![format!("{}.{}", script.group, script.name)], 0)
}

fn xml_text(text: &str) -> String {
    // The script goes into a CDATA section; "]]>" inside it is split.
    text.replace("]]>", "]]]]><![CDATA[>")
}

/// Runs the script in Windows Script Host attached to the open EA window, or without one in the application's own
/// EA session in the background; returns what it printed.
pub fn run(cfg: &AppConfig, name: &str) -> Result<String> {
    let script = find(cfg, name)?;
    let background = !crate::ea_ui::file_or_db_open_in_window(cfg);
    if background {
        // The background session registers as EA.App as well: the script attaches to it.
        crate::ea::run_batch(cfg, &[serde_json::json!({ "op": "ping" })], &crate::ea::BatchOptions::default())?;
    }
    let result = run_attached(cfg, &script);
    if background && cfg.uses_file() {
        // The script may have changed the file: closing the session writes it to disk.
        crate::ea::shutdown();
    }
    result
}

fn run_attached(cfg: &AppConfig, script: &Script) -> Result<String> {
    let script = Script { guid: script.guid.clone(), name: script.name.clone(), language: script.language.clone(), group: script.group.clone(), code: script.code.clone() };
    let language = match script.language.as_str() {
        "VBScript" => "VBScript",
        // EA's JavaScript (SpiderMonkey) scripts run as JScript: most use only ES3/ES5 features.
        _ => "JScript",
    };
    let code = resolve_includes(cfg, &script)?;
    let job = format!(
        r#"<?xml version="1.0"?>
<job>
<script language="VBScript"><![CDATA[
Option Explicit
Class CompassSession
  Public Sub Output(text)
    Dim fso, f
    Set fso = CreateObject("Scripting.FileSystemObject")
    Set f = fso.OpenTextFile("{{out}}", 8, True, -1)
    f.WriteLine CStr(text)
    f.Close
  End Sub
  Public Function Prompt(text, kind)
    Prompt = 0
  End Function
End Class
Dim Session, Repository, EAApp
Set Session = New CompassSession
On Error Resume Next
Set EAApp = GetObject(, "EA.App")
If Err.Number <> 0 Then
  WScript.StdErr.WriteLine "NO_EA_WINDOW"
  WScript.Quit 3
End If
On Error GoTo 0
Set Repository = EAApp.Repository
If InStr(1, Repository.ConnectionString, "{{model}}", vbTextCompare) = 0 Then
  WScript.StdErr.WriteLine "OTHER_MODEL " & Repository.ConnectionString
  WScript.Quit 4
End If
]]></script>
<script language="{language}"><![CDATA[
{}
]]></script>
</job>"#,
        xml_text(&code)
    );
    let id = uuid::Uuid::new_v4();
    let file = std::env::temp_dir().join(format!("loonar-script-{id}.wsf"));
    // Session.Output goes to a Unicode file (the console output of cscript cannot carry every character).
    let out_file = std::env::temp_dir().join(format!("loonar-script-{id}.out"));
    // The model the script must run on: the file name or the database name of the settings.
    let model = if cfg.uses_file() {
        cfg.model_file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        cfg.db_name.clone()
    };
    let job = job.replace("{out}", &out_file.to_string_lossy()).replace("{model}", &model.replace('"', ""));
    std::fs::write(&file, job).context("script file")?;
    let mut cmd = std::process::Command::new("cscript.exe");
    cmd.arg("//nologo").arg(&file).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().context("cscript.exe")?;
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = std::fs::remove_file(&file);
            bail!("{}", crate::i18n::fmt("the script {0} did not finish within {1} s and was stopped", &[script.name.clone(), RUN_TIMEOUT.as_secs().to_string()]));
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let output = child.wait_with_output()?;
    let _ = std::fs::remove_file(&file);
    let printed = std::fs::read(&out_file).ok().map(|bytes| {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).skip_while(|u| *u == 0xFEFF).collect();
        String::from_utf16_lossy(&units)
    });
    let _ = std::fs::remove_file(&out_file);
    // cscript reports script errors on its console output, in the OEM code page.
    let out = printed.unwrap_or_default();
    let err = format!("{}{}", oem_text(&output.stderr), if status.success() { String::new() } else { oem_text(&output.stdout) });
    if err.contains("OTHER_MODEL") {
        bail!("{}", crate::i18n::tr("another model is open in Enterprise Architect: open this model there (or close EA) and run the script again"));
    }
    if err.contains("NO_EA_WINDOW") {
        bail!("{}", crate::i18n::tr("a script runs in the open EA window: open this model in Enterprise Architect first"));
    }
    if !status.success() || !err.trim().is_empty() {
        bail!("{}", crate::i18n::fmt("the script {0} failed: {1}{2}", &[script.name.clone(), err.trim().to_string(), if out.trim().is_empty() { String::new() } else { format!("\nOutput before the error:\n{}", out.trim()) }]));
    }
    Ok(if out.trim().is_empty() { crate::i18n::fmt("The script {0} ran without output.", &[script.name]) } else { crate::i18n::fmt("Output of the script {0}:\n{1}", &[script.name, out.trim_end().to_string()]) })
}

/// Tool definition for the model.
pub fn definition() -> Value {
    json!({
        "name": "ea_script",
        "description": "EA scripts (the Scripting window: groups of JScript, VBScript and JavaScript scripts stored in the model). \
action list: groups and scripts; read: the code of a script (name = Group.Name or the name); save: creates or replaces a script \
(group is created when missing, language JScript (default), VBScript or JavaScript); run: runs a script in the user's open EA window \
(EA's Repository object and Session.Output work; the user approves every run; JavaScript scripts run as JScript). \
Write scripts that change the model only when the user asks for a script; for ordinary changes use ea_write.",
        "input_schema": {
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "read", "save", "run"] },
                "name": { "type": "string" },
                "group": { "type": "string", "description": "Script group for save (default \"Compass\")." },
                "language": { "type": "string", "enum": ["JScript", "VBScript", "JavaScript"] },
                "code": { "type": "string" }
            },
            "required": ["action"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_cdata() {
        assert_eq!(attr("<Script Name=\"Fix names\" Type=\"Internal\" Language=\"VBScript\"/>", "Name"), "Fix names");
        assert_eq!(attr("<Script Name=\"A\" Language=\"JScript\"/>", "Language"), "JScript");
        assert_eq!(xml_text("a]]>b"), "a]]]]><![CDATA[>b");
        assert!(new_guid().starts_with('{') && new_guid().len() == 38);
        let _ = anyhow!("x");
    }
}


/// Text in the console's OEM code page (cscript's error messages).
fn oem_text(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    use windows::Win32::Globalization::{MultiByteToWideChar, CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};
    // SAFETY: the buffers are sized by the first call.
    unsafe {
        let n = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if n <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; n as usize];
        MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut wide));
        String::from_utf16_lossy(&wide)
    }
}
