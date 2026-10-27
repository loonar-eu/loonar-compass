//! MDG technologies (profiles of stereotypes with their tagged values) the assistant can read:
//! - the technology files of EA (`<EA folder>\MDGTechnologies\*.xml`) and of the user (`%APPDATA%\Sparx Systems\EA\MDGTechnologies`),
//! - technologies imported into the model (`t_document`, DocType "MDG"),
//! - which profiles the model really uses (`t_xref` "Stereotypes": FQName = Profile::Stereotype).
//!
//! The XML is read with a small tag scanner (attributes only): stereotypes, what they apply to, their tagged values
//! with type, allowed values and default.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One stereotype of a technology.
#[derive(Debug, Clone, Default)]
pub struct Stereotype {
    pub name: String,
    pub metatype: String,
    pub applies_to: Vec<String>,
    pub notes: String,
    /// (name, type, values, default, description)
    pub tags: Vec<(String, String, String, String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct Technology {
    pub id: String,
    pub name: String,
    pub version: String,
    pub notes: String,
    pub source: String,
    pub profiles: Vec<String>,
    pub stereotypes: Vec<Stereotype>,
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Tags of the document in order: (name, attributes, closing). Text between tags is skipped.
fn tags(xml: &str) -> Vec<(String, BTreeMap<String, String>, bool)> {
    let mut out = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0;
    while let Some(start) = xml[i..].find('<').map(|p| p + i) {
        let Some(end) = xml[start..].find('>').map(|p| p + start) else { break };
        let inner = &xml[start + 1..end];
        i = end + 1;
        if inner.starts_with('?') || inner.starts_with('!') {
            continue;
        }
        let closing = inner.starts_with('/');
        let inner = inner.trim_start_matches('/').trim_end_matches('/');
        let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        let name = inner[..name_end].to_string();
        let mut attrs = BTreeMap::new();
        let mut rest = &inner[name_end..];
        while let Some(eq) = rest.find('=') {
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
            let Some(close) = after[1..].find(quote) else { break };
            attrs.insert(key, decode(&after[1..close + 1]));
            rest = &after[close + 2..];
        }
        let _ = bytes;
        out.push((name, attrs, closing));
    }
    out
}

/// Parses one MDG technology file (or the XML of one stored in the model).
pub fn parse(xml: &str, source: &str) -> Technology {
    let mut tech = Technology { source: source.to_string(), ..Default::default() };
    let mut current: Option<Stereotype> = None;
    let mut in_profile_doc = false;
    for (name, attrs, closing) in tags(xml) {
        let get = |k: &str| attrs.get(k).cloned().unwrap_or_default();
        match (name.as_str(), closing) {
            ("Documentation", false) if tech.id.is_empty() && !in_profile_doc => {
                tech.id = get("id");
                tech.name = get("name");
                tech.version = get("version");
                tech.notes = get("notes");
            }
            ("UMLProfile", false) => in_profile_doc = true,
            ("Documentation", false) if in_profile_doc => {
                let profile = get("name");
                if !profile.is_empty() && !tech.profiles.contains(&profile) {
                    tech.profiles.push(profile);
                }
                in_profile_doc = false;
            }
            ("Stereotype", false) => {
                if let Some(s) = current.take() {
                    tech.stereotypes.push(s);
                }
                current = Some(Stereotype { name: get("name"), metatype: get("metatype"), notes: get("notes"), ..Default::default() });
            }
            ("Stereotype", true) => {
                if let Some(s) = current.take() {
                    tech.stereotypes.push(s);
                }
            }
            ("Apply", false) => {
                if let Some(s) = current.as_mut() {
                    let t = get("type");
                    if !t.is_empty() && !s.applies_to.contains(&t) {
                        s.applies_to.push(t);
                    }
                }
            }
            ("Tag", false) => {
                if let Some(s) = current.as_mut() {
                    let tag = get("name");
                    // Internal tags of the toolbox ("Profile::Stereotype(UML::Class)") are not tagged values.
                    if !tag.is_empty() && !tag.contains("::") {
                        s.tags.push((tag, get("type"), get("values"), get("default"), get("description")));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(s) = current.take() {
        tech.stereotypes.push(s);
    }
    tech.stereotypes.retain(|s| !s.name.is_empty());
    tech
}

/// Folders with technology files: EA's own and the user's.
fn folders(cfg: &crate::config::AppConfig) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(ea_dir) = cfg.ea_path.parent() {
        out.push(ea_dir.join("MDGTechnologies"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        out.push(Path::new(&appdata).join("Sparx Systems").join("EA").join("MDGTechnologies"));
    }
    out
}

/// Technology files: (path, technology id and name read from the start of the file).
fn files(cfg: &crate::config::AppConfig) -> Vec<(PathBuf, String, String)> {
    let mut out = Vec::new();
    for folder in folders(cfg) {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")) {
                continue;
            }
            // The id and name are in the first Documentation tag, near the start.
            let head = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b[..b.len().min(8192)]).to_string()).unwrap_or_default();
            if let Some((_, attrs, _)) = tags(&head).into_iter().find(|(n, _, c)| n == "Documentation" && !c) {
                out.push((path, attrs.get("id").cloned().unwrap_or_default(), attrs.get("name").cloned().unwrap_or_default()));
            }
        }
    }
    out
}

/// Technologies imported into the model: (name, XML) from t_document.
fn stored(cfg: &crate::config::AppConfig) -> Vec<(String, String)> {
    let rows = crate::tools::query_rows(cfg, "select coalesce(docname,'') as c0, coalesce(doctype,'') as c1, coalesce(elementtype,'') as c2, coalesce(strcontent,'') as c3 from t_document where doctype = 'MDG' or elementtype = 'MDG'")
        .unwrap_or_default();
    rows.into_iter().filter(|r| r[3].contains("<MDG.Technology")).map(|r| (r[0].clone(), r[3].clone())).collect()
}

/// Profiles and stereotypes the model uses: profile → (stereotype → number of uses).
fn used(cfg: &crate::config::AppConfig) -> BTreeMap<String, BTreeMap<String, usize>> {
    let rows = crate::tools::query_rows(cfg, "select description as c0 from t_xref where name = 'Stereotypes'").unwrap_or_default();
    let mut out: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for r in rows {
        for part in r[0].split("@STEREO;").skip(1) {
            let Some(fq) = part.split(';').find_map(|kv| kv.strip_prefix("FQName=")) else { continue };
            let (profile, stereotype) = fq.split_once("::").unwrap_or(("(no profile)", fq));
            *out.entry(profile.to_string()).or_default().entry(stereotype.to_string()).or_insert(0) += 1;
        }
    }
    out
}

fn find(cfg: &crate::config::AppConfig, wanted: &str) -> Result<Technology> {
    let w = wanted.trim().to_lowercase();
    for (name, xml) in stored(cfg) {
        let t = parse(&xml, "model");
        if t.id.to_lowercase() == w || t.name.to_lowercase() == w || name.to_lowercase() == w || t.profiles.iter().any(|p| p.to_lowercase() == w) {
            return Ok(t);
        }
    }
    let files = files(cfg);
    let hit = files
        .iter()
        .find(|(_, id, name)| id.to_lowercase() == w || name.to_lowercase() == w)
        .or_else(|| files.iter().find(|(p, id, name)| id.to_lowercase().contains(&w) || name.to_lowercase().contains(&w) || p.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase().contains(&w))));
    if let Some((path, _, _)) = hit {
        let bytes = std::fs::read(path).with_context(|| crate::i18n::fmt("cannot read {0}", &[path.display().to_string()]))?;
        return Ok(parse(&String::from_utf8_lossy(&bytes), &path.display().to_string()));
    }
    // A profile name used in the model (e.g. "BPMN2.0") inside some technology file.
    for (path, _, _) in &files {
        let bytes = std::fs::read(path).unwrap_or_default();
        let t = parse(&String::from_utf8_lossy(&bytes), &path.display().to_string());
        if t.profiles.iter().any(|p| p.to_lowercase() == w) {
            return Ok(t);
        }
    }
    bail!("{}", crate::i18n::fmt("no MDG technology or profile \"{0}\": call mdg_technologies without arguments for the list", &[wanted.to_string()]))
}

fn stereotype_text(s: &Stereotype) -> String {
    let mut out = format!("«{}» metatype {}{}", s.name, if s.metatype.is_empty() { "-" } else { &s.metatype }, if s.applies_to.is_empty() { String::new() } else { format!(", applies to {}", s.applies_to.join(", ")) });
    if !s.notes.trim().is_empty() {
        out.push_str(&format!(" – {}", s.notes.trim().chars().take(160).collect::<String>()));
    }
    for (name, kind, values, default, description) in &s.tags {
        out.push_str(&format!("\n    tag {name}"));
        if !kind.is_empty() {
            out.push_str(&format!(" : {kind}"));
        }
        if !values.is_empty() {
            out.push_str(&format!(" [{values}]"));
        }
        if !default.is_empty() {
            out.push_str(&format!(" = {default}"));
        }
        if !description.trim().is_empty() {
            out.push_str(&format!(" – {}", description.trim().chars().take(120).collect::<String>()));
        }
    }
    out
}

/// The tool: without arguments the list of technologies and the profiles the model uses; with `technology` its
/// stereotypes; with `stereotype` also the tagged values of that one only.
pub fn run(input: &Value, cfg: &crate::config::AppConfig) -> Result<String> {
    let technology = input["technology"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let stereotype = input["stereotype"].as_str().map(str::trim).filter(|s| !s.is_empty());
    let Some(technology) = technology else {
        let mut out = vec!["Profiles used in the model (profile: stereotype × uses):".to_string()];
        let used = used(cfg);
        if used.is_empty() {
            out.push("  (no stereotypes from profiles)".into());
        }
        for (profile, stereotypes) in &used {
            let list: Vec<String> = stereotypes.iter().map(|(s, n)| format!("{s} ×{n}")).collect();
            out.push(format!("  {profile}: {}", list.join(", ")));
        }
        let stored = stored(cfg);
        out.push(String::new());
        out.push(format!("Technologies imported into the model: {}", if stored.is_empty() { "none".to_string() } else { stored.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", ") }));
        out.push(String::new());
        out.push("Technology files of EA and of the user (id – name):".into());
        for (path, id, name) in files(cfg) {
            out.push(format!("  {id} – {name} ({})", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        }
        out.push("\nCall again with technology (an id, a name or a profile name above) for its stereotypes and tagged values.".into());
        return Ok(out.join("\n"));
    };
    let tech = find(cfg, technology)?;
    let mut out = vec![format!("{} ({}) version {} – {} stereotypes; profiles: {}; source: {}", tech.name, tech.id, tech.version, tech.stereotypes.len(), tech.profiles.join(", "), tech.source)];
    if !tech.notes.trim().is_empty() {
        out.push(tech.notes.trim().to_string());
    }
    match stereotype {
        Some(wanted) => {
            let w = wanted.trim_matches(['«', '»']).to_lowercase();
            let hits: Vec<&Stereotype> = tech.stereotypes.iter().filter(|s| s.name.to_lowercase() == w || s.name.to_lowercase().contains(&w)).collect();
            if hits.is_empty() {
                bail!("{}", crate::i18n::fmt("technology {0} has no stereotype \"{1}\"", &[tech.name.clone(), wanted.to_string()]));
            }
            out.extend(hits.iter().take(20).map(|s| stereotype_text(s)));
        }
        None => {
            // All stereotypes, tagged values only counted (the list can be long).
            for s in tech.stereotypes.iter().take(400) {
                out.push(format!("«{}» {}{}{}", s.name, s.metatype, if s.applies_to.is_empty() { String::new() } else { format!(" → {}", s.applies_to.join(", ")) }, if s.tags.is_empty() { String::new() } else { format!(" ({} tags)", s.tags.len()) }));
            }
            out.push("\nCall again with stereotype for its tagged values (types, allowed values, defaults).".into());
        }
    }
    Ok(out.join("\n"))
}

pub fn definition() -> Value {
    json!({
        "name": "mdg_technologies",
        "description": "MDG technologies of Enterprise Architect (profiles of stereotypes with tagged values): EA's own and the user's technology files, technologies imported into the model, and which profiles and stereotypes the model really uses. \
Without arguments: the profiles used in the model and the list of technologies. With technology (an id, a name or a profile name, e.g. ArchiMate3, BPMN2.0, SysML1.5): its stereotypes with metatype and what they apply to. \
With technology and stereotype: that stereotype's tagged values with type, allowed values and default. Use it before creating elements with stereotypes of a technology (the exact stereotype and metatype) and before filling tagged values.",
        "input_schema": {
            "type": "object",
            "properties": {
                "technology": { "type": "string", "description": "Technology id, name or profile name; empty = the list." },
                "stereotype": { "type": "string", "description": "A stereotype of that technology (its tagged values)." }
            }
        }
    })
}

/// Packages under version control: version, configuration, who has them checked out, last save and load.
pub fn version_control_status(input: &Value, cfg: &crate::config::AppConfig) -> Result<String> {
    let filter = match input["package"].as_i64() {
        Some(id) => format!(" and package_id = {id}"),
        None => String::new(),
    };
    let rows = crate::tools::query_rows(
        cfg,
        &format!(
            "select package_id as c0, coalesce(name,'') as c1, coalesce(version,'') as c2, coalesce(packageflags,'') as c3, coalesce(xmlpath,'') as c4, \
             coalesce(cast(lastloaddate as varchar(30)),'') as c5, coalesce(cast(lastsavedate as varchar(30)),'') as c6 from t_package \
             where (packageflags like '%VCCFG=%' or cast(iscontrolled as varchar(5)) in ('1','true')){filter} order by name"
        ),
    )?;
    if rows.is_empty() {
        return Ok("No package of this model is under version control (in EA: Package → Version Control → Add Package to Version Control; the configuration in Settings → Version Control).".into());
    }
    let flag = |flags: &str, key: &str| flags.split(';').find_map(|kv| kv.strip_prefix(key)).unwrap_or("").to_string();
    let mut out = vec![format!("Packages under version control: {}", rows.len())];
    for r in rows {
        let checked_out = flag(&r[3], "CheckedOutTo=");
        out.push(format!(
            "package {} \"{}\": version {}, configuration {}, file {}, {}, last saved {}, last loaded {}",
            r[0],
            r[1],
            if r[2].is_empty() { "-" } else { &r[2] },
            flag(&r[3], "VCCFG="),
            if r[4].is_empty() { "-" } else { &r[4] },
            if checked_out.is_empty() { "checked in (read-only in EA)".to_string() } else { format!("checked out to {checked_out}") },
            if r[6].is_empty() { "-" } else { &r[6] },
            if r[5].is_empty() { "-" } else { &r[5] },
        ));
    }
    Ok(out.join("\n"))
}

pub fn version_control_definition() -> Value {
    json!({
        "name": "version_control_status",
        "description": "Packages of the model under version control (EA's package version control with SVN, Git, TFS or a file share): version, configuration, the file, who has each package checked out, last save and load. \
Read it before checking a package in or out (ea_write op version_control) and when the user asks what is checked out.",
        "input_schema": {
            "type": "object",
            "properties": { "package": { "type": "integer", "description": "Only this package (package_id)." } }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_technology() {
        let xml = r#"<?xml version="1.0" encoding="windows-1252"?>
<MDG.Technology version="1.0"><Documentation id="ACME" name="ACME &amp; Co" version="1.2" notes="Test"/>
<UMLProfiles><UMLProfile profiletype="uml2"><Documentation id="X" name="AcmeProfile" version="1"/>
<Content><Stereotypes>
<Stereotype name="Service" metatype="Service" notes="A service"><Icon>QkAA</Icon><AppliesTo><Apply type="Component"/></AppliesTo>
<TaggedValues><Tag name="owner" type="" values="" default="" description="Who owns it"/><Tag name="tier" type="enumeration" values="gold,silver" default="silver"/>
<Tag name="AcmeProfile::Service(UML::Component)" type="" values="" default="Service"/></TaggedValues></Stereotype>
<Stereotype name="Api" metatype="Interface"><AppliesTo><Apply type="Interface"/></AppliesTo></Stereotype>
</Stereotypes></Content></UMLProfile></UMLProfiles></MDG.Technology>"#;
        let t = parse(xml, "test");
        assert_eq!((t.id.as_str(), t.name.as_str(), t.version.as_str()), ("ACME", "ACME & Co", "1.2"));
        assert_eq!(t.profiles, vec!["AcmeProfile"]);
        assert_eq!(t.stereotypes.len(), 2);
        assert_eq!(t.stereotypes[0].applies_to, vec!["Component"]);
        assert_eq!(t.stereotypes[0].tags.len(), 2, "the toolbox tag is not a tagged value");
        let text = stereotype_text(&t.stereotypes[0]);
        assert!(text.contains("tag tier : enumeration [gold,silver] = silver"), "{text}");
    }

    /// The technologies on this computer and in a test model: `LOONAR_TEST_PROPS=… cargo test mdg_on_model -- --ignored --nocapture`
    #[test]
    #[ignore = "requires EA installed and a test model"]
    fn mdg_on_model() {
        crate::i18n::set(crate::i18n::Lang::En);
        let cfg = crate::config::AppConfig::load(Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        let list = run(&json!({}), &cfg).unwrap();
        println!("{}", list.lines().take(12).collect::<Vec<_>>().join("
"));
        assert!(list.contains("BPMN2.0: "), "the model uses BPMN 2.0");
        let bpmn = run(&json!({ "technology": "BPMN2.0", "stereotype": "Activity" }), &cfg).unwrap();
        println!("{}", bpmn.lines().take(14).collect::<Vec<_>>().join("
"));
        assert!(bpmn.contains("«Activity» metatype Activity") && bpmn.contains("tag "));
        let archimate = run(&json!({ "technology": "ArchiMate3" }), &cfg).unwrap();
        assert!(archimate.contains("«ArchiMate_ApplicationComponent»"));
    }
}
