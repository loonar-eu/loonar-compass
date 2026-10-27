//! The object model of the EA automation interface, read by reflection from EA's own type library (EA.tlb next to
//! EA.exe): classes, their properties and methods with parameter names and types, and the help strings. The assistant
//! uses it to write EA scripts against the API of the EA version that is installed, not from memory.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use windows::core::{BSTR, HSTRING};
use windows::Win32::System::Com::{ITypeInfo, ITypeLib, FUNCDESC, INVOKE_FUNC, INVOKE_PROPERTYGET, INVOKE_PROPERTYPUT, INVOKE_PROPERTYPUTREF, TKIND_DISPATCH, TKIND_ENUM, TKIND_INTERFACE, TYPEDESC};
use windows::Win32::System::Ole::LoadTypeLib;
use windows::Win32::System::Variant::*;

/// One member of a class.
pub struct Member {
    pub name: String,
    /// "property" (read), "property (read/write)", "method"
    pub kind: String,
    pub signature: String,
    pub doc: String,
}

pub struct Class {
    pub name: String,
    pub doc: String,
    pub members: Vec<Member>,
}

fn type_library(cfg: &crate::config::AppConfig) -> Result<PathBuf> {
    let dir = cfg.ea_path.parent().map(Path::to_path_buf).unwrap_or_default();
    for candidate in [dir.join("EA.tlb"), dir.join("EA.exe"), PathBuf::from(r"C:\Program Files\Sparx Systems\EA\EA.tlb")] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    bail!("{}", crate::i18n::tr("EA's type library (EA.tlb) was not found: check the path to EA.exe in Settings"))
}

fn vt_name(desc: &TYPEDESC, info: &ITypeInfo) -> String {
    let vt = desc.vt;
    match vt {
        VT_BSTR => "String".into(),
        VT_I2 => "Short".into(),
        VT_I4 | VT_INT => "Long".into(),
        VT_UI4 | VT_UINT => "Unsigned Long".into(),
        VT_BOOL => "Boolean".into(),
        VT_R4 => "Single".into(),
        VT_R8 => "Double".into(),
        VT_DATE => "Date".into(),
        VT_VARIANT => "Variant".into(),
        VT_DISPATCH => "Object".into(),
        VT_UNKNOWN => "Unknown".into(),
        VT_VOID => "void".into(),
        VT_HRESULT => "void".into(),
        VT_PTR => {
            let inner = unsafe { &*desc.Anonymous.lptdesc };
            vt_name(inner, info)
        }
        VT_SAFEARRAY => {
            let inner = unsafe { &*desc.Anonymous.lptdesc };
            format!("{}()", vt_name(inner, info))
        }
        VT_USERDEFINED => unsafe {
            info.GetRefTypeInfo(desc.Anonymous.hreftype)
                .ok()
                .and_then(|r| {
                    let mut name = BSTR::new();
                    r.GetDocumentation(-1, Some(&mut name), None, std::ptr::null_mut(), None).ok().map(|_| script_name(&name.to_string()))
                })
                .unwrap_or_else(|| "Object".into())
        },
        other => format!("VT{}", other.0),
    }
}

fn doc_of(info: &ITypeInfo, memid: i32) -> (String, String) {
    let mut name = BSTR::new();
    let mut doc = BSTR::new();
    unsafe {
        let _ = info.GetDocumentation(memid, Some(&mut name), Some(&mut doc), std::ptr::null_mut(), None);
    }
    (name.to_string(), doc.to_string())
}

fn class_of(info: &ITypeInfo) -> Result<Class> {
    let (name, doc) = doc_of(info, -1);
    let attr = unsafe { info.GetTypeAttr()? };
    let (funcs, kind, vars) = unsafe { ((*attr).cFuncs, (*attr).typekind, (*attr).cVars) };
    let mut members: Vec<Member> = Vec::new();
    if kind == TKIND_ENUM {
        for i in 0..vars as u32 {
            if let Ok(var) = unsafe { info.GetVarDesc(i) } {
                let memid = unsafe { (*var).memid };
                let value = unsafe { (*var).Anonymous.lpvarValue.as_ref().map(|v| i32::try_from(&*v).unwrap_or_default()).unwrap_or_default() };
                let (n, d) = doc_of(info, memid);
                members.push(Member { name: n, kind: "value".into(), signature: value.to_string(), doc: d });
                unsafe { info.ReleaseVarDesc(var) };
            }
        }
    }
    for i in 0..funcs as u32 {
        let Ok(desc) = (unsafe { info.GetFuncDesc(i) }) else { continue };
        let f: &FUNCDESC = unsafe { &*desc };
        let (member, member_doc) = doc_of(info, f.memid);
        // IUnknown / IDispatch plumbing is not part of the EA API.
        if ["QueryInterface", "AddRef", "Release", "GetTypeInfoCount", "GetTypeInfo", "GetIDsOfNames", "Invoke"].contains(&member.as_str()) {
            unsafe { info.ReleaseFuncDesc(desc) };
            continue;
        }
        let mut names = vec![BSTR::new(); f.cParams as usize + 1];
        let mut count = 0u32;
        unsafe {
            let _ = info.GetNames(f.memid, &mut names, &mut count);
        }
        let params: Vec<String> = (0..f.cParams as usize)
            .map(|p| {
                let elem = unsafe { &*f.lprgelemdescParam.add(p) };
                let pname = names.get(p + 1).map(|n| n.to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("p{p}"));
                format!("{pname}: {}", vt_name(&elem.tdesc, info))
            })
            .collect();
        let returns = vt_name(&f.elemdescFunc.tdesc, info);
        let invkind = f.invkind;
        unsafe { info.ReleaseFuncDesc(desc) };
        if invkind == INVOKE_PROPERTYPUT || invkind == INVOKE_PROPERTYPUTREF {
            if let Some(existing) = members.iter_mut().find(|m| m.name == member && m.kind.starts_with("property")) {
                existing.kind = "property (read/write)".into();
            } else {
                let value = params.last().cloned().unwrap_or_default();
                members.push(Member { name: member, kind: "property (write)".into(), signature: value, doc: member_doc });
            }
            continue;
        }
        if invkind == INVOKE_PROPERTYGET {
            let read_write = members.iter().position(|m| m.name == member && m.kind == "property (write)");
            if let Some(i) = read_write {
                members.remove(i);
                members.push(Member { name: member, kind: "property (read/write)".into(), signature: returns, doc: member_doc });
            } else {
                members.push(Member { name: member, kind: "property".into(), signature: if params.is_empty() { returns } else { format!("({}) : {returns}", params.join(", ")) }, doc: member_doc });
            }
            continue;
        }
        let _ = INVOKE_FUNC;
        members.push(Member { name: member, kind: "method".into(), signature: format!("({}) : {returns}", params.join(", ")), doc: member_doc });
    }
    unsafe { info.ReleaseTypeAttr(attr) };
    Ok(Class { name, doc, members })
}

/// All classes and enumerations of the EA type library: (name, kind, help string, type info).
fn library(cfg: &crate::config::AppConfig) -> Result<Vec<(String, String, String, ITypeInfo)>> {
    let path = type_library(cfg)?;
    let lib: ITypeLib = unsafe { LoadTypeLib(&HSTRING::from(path.as_os_str())) }.with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?;
    let mut out = Vec::new();
    for i in 0..unsafe { lib.GetTypeInfoCount() } {
        let Ok(info) = (unsafe { lib.GetTypeInfo(i) }) else { continue };
        let Ok(kind) = (unsafe { lib.GetTypeInfoType(i) }) else { continue };
        let kind = match kind {
            k if k == TKIND_DISPATCH || k == TKIND_INTERFACE => "class",
            k if k == TKIND_ENUM => "enumeration",
            _ => continue,
        };
        let (name, doc) = doc_of(&info, -1);
        out.push((name, kind.to_string(), doc, info));
    }
    Ok(out)
}

/// EA's interfaces are "IDualElement" etc.; scripts know them as Element.
fn script_name(name: &str) -> String {
    name.strip_prefix("IDual").or_else(|| name.strip_prefix("IEA")).unwrap_or(name).to_string()
}

pub fn run(input: &Value, cfg: &crate::config::AppConfig) -> Result<String> {
    let wanted = input["class"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let member = input["member"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_lowercase);
    let lib = library(cfg)?;
    let Some(wanted) = wanted else {
        let mut classes: Vec<String> = lib.iter().filter(|(n, k, _, _)| k == "class" && n.starts_with("IDual")).map(|(n, _, _, _)| script_name(n)).collect();
        classes.sort();
        classes.dedup();
        let mut enums: Vec<String> = lib.iter().filter(|(_, k, _, _)| k == "enumeration").map(|(n, _, _, _)| n.clone()).collect();
        enums.sort();
        return Ok(format!(
            "EA automation interface (from EA.tlb of the installed EA).\nClasses ({}): {}\nEnumerations ({}): {}\nCall again with class (e.g. Element, Repository, Diagram) for its properties and methods; add member to see one.",
            classes.len(), classes.join(", "), enums.len(), enums.join(", ")
        ));
    };
    let w = wanted.to_lowercase();
    let hit = lib
        .iter()
        .find(|(n, k, _, _)| k == "class" && n.starts_with("IDual") && script_name(n).to_lowercase() == w)
        .or_else(|| lib.iter().find(|(n, _, _, _)| n.to_lowercase() == w || script_name(n).to_lowercase() == w))
        .with_context(|| crate::i18n::fmt("the EA API has no class \"{0}\": call ea_api without arguments for the list", &[wanted.to_string()]))?;
    let class = class_of(&hit.3)?;
    let mut out = vec![format!("{} – {}", script_name(&class.name), if class.doc.is_empty() { hit.2.clone() } else { class.doc.clone() })];
    let mut members: Vec<&Member> = class.members.iter().filter(|m| member.as_ref().is_none_or(|w| m.name.to_lowercase().contains(w.as_str()))).collect();
    members.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
    if members.is_empty() {
        bail!("{}", crate::i18n::fmt("{0} has no member \"{1}\"", &[script_name(&class.name), member.unwrap_or_default()]));
    }
    for m in members {
        let doc = if m.doc.is_empty() { String::new() } else { format!(" – {}", m.doc) };
        out.push(match m.kind.as_str() {
            "method" => format!("method {}{}{doc}", m.name, m.signature.replace("IDual", "")),
            "value" => format!("{} = {}{doc}", m.name, m.signature),
            kind => format!("{kind} {} : {}{doc}", m.name, m.signature),
        });
    }
    Ok(out.join("\n"))
}

pub fn definition() -> Value {
    json!({
        "name": "ea_api",
        "description": "The EA automation interface (the object model used by EA scripts and add-ins), read from the type library of the EA installed on this computer: \
the list of classes and enumerations, the properties (read or read/write) and methods of a class with parameter names and types, and the values of an enumeration. \
Use it before writing an EA script (ea_script) when you are not sure of a property or method name, its parameters or what a collection's AddNew takes.",
        "input_schema": {
            "type": "object",
            "properties": {
                "class": { "type": "string", "description": "A class (Element, Repository, Diagram, Connector, Package, Attribute, Method, TaggedValue, Collection…) or an enumeration (e.g. ObjectType); empty = the list." },
                "member": { "type": "string", "description": "Only members whose name contains this text." }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    /// Reads the type library of the installed EA: `cargo test ea_api_reflection -- --ignored --nocapture`
    #[test]
    #[ignore = "requires Enterprise Architect installed"]
    fn ea_api_reflection() {
        crate::i18n::set(crate::i18n::Lang::En);
        let props = std::env::temp_dir().join(format!("loonar-ea-api-{}.properties", std::process::id()));
        std::fs::write(&props, "model.source=file
model.file=C:/none.qea
ea.path=C:/Program Files/Sparx Systems/EA/EA.exe
").unwrap();
        let cfg = crate::config::AppConfig::load(&props).unwrap();
        let list = super::run(&serde_json::json!({}), &cfg).unwrap();
        println!("{}", &list[..list.len().min(600)]);
        assert!(list.contains("Element") && list.contains("Repository"));
        let element = super::run(&serde_json::json!({ "class": "Element" }), &cfg).unwrap();
        println!("{}", element.lines().take(15).collect::<Vec<_>>().join("\n"));
        assert!(element.contains("Name : String"), "{element}");
        assert!(element.lines().any(|l| l.starts_with("method Update")));
        for member in ["VersionControl", "IsControlled", "IsVersionControlled"] {
            println!("{}", super::run(&serde_json::json!({ "class": "Package", "member": member }), &cfg).unwrap_or_default());
        }
        let tagged = super::run(&serde_json::json!({ "class": "Repository", "member": "GetElementByID" }), &cfg).unwrap();
        println!("{tagged}");
        assert!(tagged.contains("ElementID") || tagged.contains("Long"));
    }
}
