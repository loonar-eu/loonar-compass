//! What changed in the model (LC-371, /what-changed): elements, diagrams and packages created or changed since a date
//! (or in the last days), optionally by one author and in one package, grouped by package with who and when. EA keeps
//! the creation and modification dates of these objects; relationships have none, and deletions leave no trace – a
//! comparison with a baseline shows those (EA: Package → Manage Baselines → Compare). Reads only.

use crate::config::{AppConfig, ModelSource};
use crate::tools::query_rows;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({
        "name": "what_changed",
        "description": "Elements, diagrams and packages created or changed since a date or in the last days, optionally by one author and in \
one package (with child packages), grouped by package with author and time – for \"what changed this week\", a change log for a review or \
release notes, and /what-changed. Reads only. Relationships have no dates in EA and deletions leave no trace: say so, and for those offer a \
comparison with a baseline in EA. Summarise the result in business language when asked.",
        "input_schema": { "type": "object", "properties": {
            "since": { "type": "string", "description": "YYYY-MM-DD, or a number of days back such as \"7\" (default 7)" },
            "author": { "type": "string", "description": "Only changes by this author (EA's author field, optional)" },
            "package": { "description": "package_id or exact name (optional; required with baseline)" },
            "baseline": { "type": "string", "description": "Compare the package with this baseline of it (its version, or \"latest\") instead of dates: \
added, deleted and changed elements, relationships and diagrams, without EA's noise (dates, line ends, whitespace)" }
        } }
    })
}

/// The first day of the period: a date YYYY-MM-DD as given, or today minus a number of days.
pub(crate) fn since_date(given: &str, today: chrono::NaiveDate) -> Result<chrono::NaiveDate> {
    let given = given.trim();
    if given.is_empty() {
        return Ok(today - chrono::Duration::days(7));
    }
    if let Ok(days) = given.trim_end_matches(['d', 'D']).trim().parse::<i64>() {
        if (0..=3650).contains(&days) {
            return Ok(today - chrono::Duration::days(days));
        }
    }
    match chrono::NaiveDate::parse_from_str(given, "%Y-%m-%d") {
        Ok(date) => Ok(date),
        Err(_) => bail!("{}", crate::i18n::tr("since: give a date YYYY-MM-DD or a number of days")),
    }
}

/// A date and time "YYYY-MM-DD HH:MM:SS" as a literal of the model's engine (Oracle: TIMESTAMP '…').
pub(crate) fn date_text(source: ModelSource, stamp: &str) -> String {
    let stamp: String = stamp.chars().filter(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | ' ')).collect();
    if source == ModelSource::Oracle { format!("timestamp '{stamp}'") } else { format!("'{stamp}'") }
}

/// A date literal of the model's engine (Oracle compares dates with DATE '…'; the others with the ISO text).
fn date_literal(source: ModelSource, date: chrono::NaiveDate) -> String {
    let text = date.format("%Y-%m-%d").to_string();
    if source == ModelSource::Oracle { format!("date '{text}'") } else { format!("'{text}'") }
}

fn xml_attr(tag: &str, name: &str) -> String {
    let key = format!(" {name}=\"");
    tag.find(&key)
        .and_then(|at| tag[at + key.len()..].split('"').next())
        .unwrap_or("")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&#xD;", "\r")
        .replace("&#xA;", "\n")
        .replace("&amp;", "&")
}

/// The same text apart from line ends and whitespace (EA's comparison reports those as changes).
fn same_text(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

/// EA's comparison log (DoBaselineCompare) as a readable list without its noise (LC-371): items only in the model
/// (added), only in the baseline (deleted), changed – with the properties that really changed (not DateModified,
/// DateCreated, nor text that differs only in line ends or whitespace; EA's internal "Scope" is shown as Visibility).
/// EA nests the items (LC-440): a relationship sits in the "Links" of its element, the places of elements on a diagram
/// in its "Visual Elements"; a value only on one side ("Model only" / "Baseline only", e.g. notes written after the
/// baseline) is a change; the kind of an item comes from its "Type" property, as EA leaves the type attribute empty.
pub(crate) fn baseline_diff(xml: &str) -> (Vec<String>, Vec<String>, Vec<String>, usize) {
    let (mut added, mut deleted, mut changed, mut noise) = (Vec::new(), Vec::new(), Vec::new(), 0usize);
    // Open items: (name, type attribute, label).
    let mut stack: Vec<(String, String, String)> = Vec::new();
    // Elements moved or resized on each diagram, reported as one line per diagram.
    let mut layout: Vec<(String, usize)> = Vec::new();
    let mut rest = xml;
    loop {
        let open = rest.find("<CompareItem ");
        let close = rest.find("</CompareItem>");
        match (open, close) {
            (Some(o), c) if c.is_none_or(|c| o < c) => {
                let segment = &rest[o + "<CompareItem ".len()..];
                let head = format!(" {}", segment.split('>').next().unwrap_or(""));
                let own_end = segment.find("<CompareItem ").into_iter().chain(segment.find("</CompareItem>")).min().unwrap_or(segment.len());
                let body = &segment[..own_end];
                let (name, kind, status) = (xml_attr(&head, "name"), xml_attr(&head, "type"), xml_attr(&head, "status"));
                // Changed properties: (name, baseline, model).
                let mut props: Vec<(String, String, String)> = Vec::new();
                let mut item_type = String::new();
                let mut element = false;
                for p in body.split("<Property ").skip(1) {
                    let p = format!(" {}", p.split("/>").next().unwrap_or(""));
                    let prop = xml_attr(&p, "name");
                    let (model, base) = (xml_attr(&p, "model"), xml_attr(&p, "baseline"));
                    if prop == "Type" {
                        item_type = if model.is_empty() { base.clone() } else { model.clone() };
                    }
                    element |= prop == "Abstract";
                    if !matches!(xml_attr(&p, "status").as_str(), "Changed" | "Model only" | "Baseline only") {
                        continue;
                    }
                    let duid_only = |t: &str| t.split(';').filter(|x| !x.trim().is_empty()).all(|x| x.trim_start().starts_with("DUID="));
                    if matches!(prop.as_str(), "DateModified" | "DateCreated" | "Modified" | "Created") || same_text(&model, &base) || (prop == "Style" && duid_only(&model) && duid_only(&base)) || prop == "Geometry" {
                        // Geometry: the bends of a line, a layout change of the diagram, not of the model.
                        continue;
                    }
                    props.push((if prop == "Scope" { "Visibility".to_string() } else { prop }, base, model));
                }
                let parent = stack.last().cloned();
                let owner = stack.iter().rev().nth(1).map(|s| s.0.clone()).unwrap_or_default();
                let label = match (kind.as_str(), parent.as_ref().map(|p| p.1.as_str())) {
                    ("DGM", _) => format!("\"{name}\" {} \"{owner}\"", "on diagram"),
                    (_, Some("Links")) => format!("{} \"{name}\" ({} \"{owner}\")", "relationship", "of"),
                    (_, Some(container)) if !container.is_empty() && container != "DGM" && !kind.is_empty() => format!("{container} \"{name}\""),
                    (_, Some(container)) if !container.is_empty() && container != "DGM" => format!("{container} \"{name}\" ({} \"{owner}\")", "of"),
                    // An item EA types itself (an attribute, an operation) sits right in its element: its "Type" property
                    // is its data type, not a diagram type.
                    (_, Some(_)) if matches!(kind.as_str(), "Attribute" | "Operation" | "Parameter" | "Constraint" | "Requirement" | "Scenario" | "Tagged Value") => format!("{kind} \"{name}\" ({} \"{}\")", "of", parent.as_ref().map(|p| p.0.as_str()).unwrap_or_default()),
                    _ if element && !item_type.is_empty() => format!("{item_type} \"{name}\""),
                    _ if !item_type.is_empty() => format!("{} \"{name}\" ({item_type})", "diagram"),
                    _ if !kind.is_empty() => format!("{kind} \"{name}\""),
                    _ => format!("\"{name}\""),
                };
                let container = matches!(kind.as_str(), "Links" | "Visual Elements") || (kind == name && !kind.is_empty() && kind != "DGM");
                if !container {
                    match status.as_str() {
                        "Model only" => added.push(label.clone()),
                        "Baseline only" => deleted.push(label.clone()),
                        "Changed" | "Identical" if !props.is_empty() || status == "Changed" => {
                            let geometry = |p: &str| matches!(p, "Left" | "Top" | "Right" | "Bottom");
                            if kind == "DGM" && props.iter().any(|p| geometry(&p.0)) {
                                match layout.iter_mut().find(|l| l.0 == owner) {
                                    Some(l) => l.1 += 1,
                                    None => layout.push((owner.clone(), 1)),
                                }
                            }
                            let shown: Vec<String> = props
                                .iter()
                                .filter(|p| !(kind == "DGM" && geometry(&p.0)))
                                .map(|(prop, base, model)| match settings_diff(base, model) {
                                    // "A=1;B=2;" settings: only the ones that differ.
                                    Some(diff) => format!("{prop}: {diff}"),
                                    None => {
                                        let short = |t: &str| if t.chars().count() > 80 { format!("{}…", t.chars().take(80).collect::<String>()) } else { t.to_string() };
                                        format!("{prop}: \"{}\" → \"{}\"", short(base), short(model))
                                    }
                                })
                                .collect();
                            if !shown.is_empty() {
                                changed.push(format!("{label}: {}", shown.join("; ")));
                            } else if status == "Changed" && !(kind == "DGM" && props.iter().any(|p| geometry(&p.0))) {
                                // A change of the item's children only (they are listed themselves) or of EA's noise.
                                noise += usize::from(props.is_empty() && !body_has_children(segment, own_end));
                            }
                        }
                        _ => {}
                    }
                }
                stack.push((name, kind, label));
                rest = &segment[own_end..];
            }
            (_, Some(c)) => {
                stack.pop();
                rest = &rest[c + "</CompareItem>".len()..];
            }
            _ => break,
        }
    }
    for (diagram, count) in layout {
        changed.push(format!("diagram \"{diagram}\": layout changed (elements moved or resized: {count})"));
    }
    (added, deleted, changed, noise)
}

/// The settings that differ between two "Key=value;Key=value;" texts, or None when they are not such texts.
fn settings_diff(base: &str, model: &str) -> Option<String> {
    let pairs = |t: &str| -> Option<Vec<(String, String)>> {
        let parts: Vec<&str> = t.split(';').map(str::trim).filter(|p| !p.is_empty()).collect();
        (parts.len() > 1 && parts.iter().all(|p| p.contains('='))).then(|| parts.iter().filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect())
    };
    let (a, b) = (pairs(base)?, pairs(model)?);
    let get = |list: &[(String, String)], key: &str| list.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut keys: Vec<&String> = a.iter().chain(b.iter()).map(|(k, _)| k).collect();
    keys.dedup();
    let mut seen = std::collections::HashSet::new();
    let diffs: Vec<String> = keys
        .into_iter()
        .filter(|k| seen.insert(k.to_string()) && k.as_str() != "DUID")
        .filter_map(|k| {
            let (x, y) = (get(&a, k), get(&b, k));
            (x != y).then(|| format!("{k} \"{x}\" → \"{y}\""))
        })
        .take(10)
        .collect();
    Some(diffs.join(", "))
}

/// Whether an item has items of its own right after its properties (its change is theirs).
fn body_has_children(segment: &str, own_end: usize) -> bool {
    segment[own_end..].starts_with("<CompareItem ")
}

/// The comparison of a package with a baseline, through EA.
fn against_baseline(input: &Value, cfg: &AppConfig) -> Result<String> {
    let Some(package) = input["package"].as_i64().or_else(|| {
        input["package"].as_str().and_then(|n| query_rows(cfg, &format!("select package_id from t_package where name = '{}'", crate::model_db::sql_text(cfg.model_source, n))).ok()?.first()?.first()?.parse().ok())
    }) else {
        bail!("{}", crate::i18n::tr("a comparison with a baseline needs the package (id or exact name)"))
    };
    let wanted = input["baseline"].as_str().unwrap_or("").trim();
    let wanted = if wanted.eq_ignore_ascii_case("latest") { "" } else { wanted };
    let file = std::env::temp_dir().join(format!("loonar-compare-{}.xml", uuid::Uuid::new_v4().simple()));
    crate::ea::run_batch(cfg, &[json!({ "op": "baseline_compare", "package": package, "baseline": wanted, "file": file.display().to_string() })], &crate::ea::BatchOptions::default())?;
    let xml = std::fs::read_to_string(&file)?;
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(format!("{}.baselines.xml", file.display()));
    let (added, deleted, changed, noise) = baseline_diff(&xml);
    let mut out = format!(
        "{}\n",
        crate::i18n::fmt(
            "Package {0} compared with its baseline {1}: {2} added, {3} deleted, {4} changed ({5} changes that were only EA's noise left out).",
            &[
                package.to_string(),
                if wanted.is_empty() { crate::i18n::tr("(the newest)").to_string() } else { wanted.to_string() },
                added.len().to_string(),
                deleted.len().to_string(),
                changed.len().to_string(),
                noise.to_string(),
            ]
        )
    );
    for (title, list) in [("Added", &added), ("Deleted", &deleted), ("Changed", &changed)] {
        if !list.is_empty() {
            out.push_str(&format!("\n{}:\n{}\n", crate::i18n::tr(title), list.iter().take(200).map(|l| format!("  - {l}")).collect::<Vec<_>>().join("\n")));
        }
    }
    Ok(out)
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    if input["baseline"].as_str().is_some_and(|b| !b.trim().is_empty()) {
        return against_baseline(input, cfg);
    }
    let since = since_date(input["since"].as_str().map(str::to_string).or_else(|| input["since"].as_i64().map(|d| d.to_string())).as_deref().unwrap_or(""), chrono::Local::now().date_naive())?;
    let from = date_literal(cfg.model_source, since);
    let author = input["author"].as_str().map(str::trim).filter(|a| !a.is_empty());
    let by = author.map(|a| format!("and lower(coalesce(x.author,'')) = lower('{}')", crate::model_db::sql_text(cfg.model_source, a))).unwrap_or_default();
    let (scope, title) = crate::model_usage::package_scope_shown(cfg, &input["package"])?;
    let scope_of = |alias: &str| scope.replace("o.package_id", &format!("{alias}.package_id"));
    let objects = query_rows(cfg, &format!(
        "select p.name, x.object_type, x.name, coalesce(x.author,''), x.createddate, x.modifieddate from t_object x join t_package p on p.package_id = x.package_id \
         where x.modifieddate >= {from} and x.object_type not in ('Package', 'Text', 'Boundary') {by} {} order by p.name, x.modifieddate desc",
        scope_of("x")
    ))?;
    let diagrams = query_rows(cfg, &format!(
        "select p.name, x.diagram_type, x.name, coalesce(x.author,''), x.createddate, x.modifieddate from t_diagram x join t_package p on p.package_id = x.package_id \
         where x.modifieddate >= {from} {by} {} order by p.name, x.modifieddate desc",
        scope_of("x")
    ))?;
    let since_text = since.format("%Y-%m-%d").to_string();
    let mut out = format!(
        "{}\n",
        match author {
            Some(a) => crate::i18n::fmt("Changes in {0} since {1} by {2}: {3} elements and {4} diagrams.", &[title.clone(), since_text.clone(), a.to_string(), objects.len().to_string(), diagrams.len().to_string()]),
            None => crate::i18n::fmt("Changes in {0} since {1}: {2} elements and {3} diagrams.", &[title.clone(), since_text.clone(), objects.len().to_string(), diagrams.len().to_string()]),
        }
    );
    let day = |stamp: &str| stamp.get(..16).unwrap_or(stamp).replace('T', " ");
    let mut current = String::new();
    for (kind, rows) in [("element", &objects), ("diagram", &diagrams)] {
        for r in rows.iter().take(300) {
            if r[0] != current {
                current = r[0].clone();
                out.push_str(&format!("\n{}\n", crate::i18n::fmt("Package \"{0}\":", &[current.clone()])));
            }
            let new = r[4].get(..10).is_some_and(|c| c >= since_text.as_str());
            let what = match (new, kind) {
                (true, "element") => crate::i18n::tr("new element"),
                (false, "element") => crate::i18n::tr("changed element"),
                (true, _) => crate::i18n::tr("new diagram"),
                (false, _) => crate::i18n::tr("changed diagram"),
            };
            out.push_str(&format!(
                "  - {what} {} \"{}\"{} – {}\n",
                r[1],
                r[2],
                if r[3].is_empty() { String::new() } else { format!(" {}", crate::i18n::fmt("by {0}", &[r[3].clone()])) },
                day(&r[5])
            ));
        }
        current.clear();
    }
    if objects.len() > 300 || diagrams.len() > 300 {
        out.push_str(&format!("\n{}\n", crate::i18n::tr("(Only the first 300 of each are listed.)")));
    }
    out.push_str(&format!("\n{}\n", crate::i18n::tr("Relationships have no dates in EA and deleted objects leave no trace: compare with a baseline in EA for those.")));
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn periods() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        assert_eq!(super::since_date("", today).unwrap().to_string(), "2026-10-24");
        assert_eq!(super::since_date("30", today).unwrap().to_string(), "2026-10-01");
        assert_eq!(super::since_date("14d", today).unwrap().to_string(), "2026-10-17");
        assert_eq!(super::since_date("2026-09-01", today).unwrap().to_string(), "2026-09-01");
        assert!(super::since_date("last week", today).is_err());
    }

    /// LC-371: EA's comparison log without its noise.
    #[test]
    fn baseline_log() {
        let xml = r#"<EA.CompareLog><ComparePackage name="P"><CompareResults><CompareItem name="P" type="" guid="{1}" status="Identical"><Properties/>
<CompareItem name="Payment" type="Class" guid="{2}" status="Model only"><Properties/></CompareItem>
<CompareItem name="Invoice" type="Class" guid="{3}" status="Baseline only"><Properties/></CompareItem>
<CompareItem name="Customer Order" type="Class" guid="{4}" status="Changed"><Properties><Property name="Name" model="Customer Order" baseline="Order" status="Changed"/><Property name="DateModified" model="2" baseline="1" status="Changed"/><Property name="Scope" model="Private" baseline="Public" status="Changed"/></Properties></CompareItem>
<CompareItem name="Shipment" type="Class" guid="{5}" status="Changed"><Properties><Property name="Notes" model="a&#xD;&#xA;b" baseline="a&#xA;b " status="Changed"/></Properties></CompareItem>
</CompareItem></CompareResults></ComparePackage></EA.CompareLog>"#;
        let (added, deleted, changed, noise) = super::baseline_diff(xml);
        assert_eq!(added, vec!["Class \"Payment\""]);
        assert_eq!(deleted, vec!["Class \"Invoice\""]);
        assert_eq!(changed, vec!["Class \"Customer Order\": Name: \"Order\" → \"Customer Order\"; Visibility: \"Public\" → \"Private\""]);
        assert_eq!(noise, 1, "notes that differ only in line ends are noise");
    }

    /// LC-440: EA's real log nests the items and leaves the type empty; notes written after the baseline are "Model only".
    #[test]
    fn baseline_log_as_ea_writes_it() {
        let xml = r#"<EA.CompareLog><ComparePackage name="P"><CompareResults><CompareItem name="P" type="" guid="{1}" status="Identical"><Properties><Property name="Abstract" model="false" baseline="false" status="Identical"/></Properties>
<CompareItem name="Faktura" type="" guid="{2}" status="Changed"><Properties><Property name="Abstract" model="false" baseline="false" status="Identical"/><Property name="DateModified" model="2" baseline="1" status="Changed"/><Property name="Notes" model="Faktura VAT." status="Model only"/><Property name="Type" model="Class" baseline="Class" status="Identical"/></Properties><CompareItem name="nabywca" type="Attribute" guid="{7}" status="Model only"><Properties><Property name="Type" model="Klient" status="Model only"/></Properties></CompareItem></CompareItem>
<CompareItem name="Serwis" type="" guid="{3}" status="Changed"><Properties><Property name="Abstract" model="false" baseline="false" status="Identical"/><Property name="Type" model="Class" baseline="Class" status="Identical"/></Properties><CompareItem name="Links" type="Links" guid="{3}Links" status="Identical"><CompareItem name="Sequence (zapisz())" type="" guid="{4}" status="Changed"><Properties><Property name="Type" model="Sequence" baseline="Sequence" status="Identical"/><Property name="Name" model="zapisz()" baseline="zapisz()" status="Identical"/><Property name="Notes" model="nowa" baseline="" status="Changed"/></Properties></CompareItem></CompareItem></CompareItem>
<CompareItem name="BB3 Sekwencja" type="" guid="{5}" status="Changed"><Properties><Property name="Type" model="Sequence" baseline="Sequence" status="Identical"/></Properties><CompareItem name="Visual Elements" type="Visual Elements" guid="{5}Visual Elements" status="Identical"><CompareItem name="Serwis" type="DGM" guid="DGM-{3}{5}" status="Changed"><Properties><Property name="Top" model="20" baseline="50" status="Changed"/><Property name="Style" baseline="DUID=47E60C04;" status="Baseline only"/></Properties></CompareItem></CompareItem></CompareItem>
<CompareItem name="Rabat" type="" guid="{6}" status="Model only"><Properties><Property name="Abstract" model="false" status="Model only"/><Property name="Type" model="Class" status="Model only"/></Properties></CompareItem>
</CompareItem></CompareResults></ComparePackage></EA.CompareLog>"#;
        let (added, deleted, changed, _) = super::baseline_diff(xml);
        assert_eq!(added, vec!["Attribute \"nabywca\" (of \"Faktura\")", "Class \"Rabat\""], "LC-440: an attribute, not a diagram");
        assert!(deleted.is_empty());
        assert_eq!(changed, vec![
            "Class \"Faktura\": Notes: \"\" → \"Faktura VAT.\"".to_string(),
            "relationship \"Sequence (zapisz())\" (of \"Serwis\"): Notes: \"\" → \"nowa\"".to_string(),
            "diagram \"BB3 Sekwencja\": layout changed (elements moved or resized: 1)".to_string(),
        ]);
    }

    /// LC-371: on a model copy: `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test what_changed_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn what_changed_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let report = super::run(&serde_json::json!({ "since": "3650" }), &cfg).expect("what_changed");
        println!("{}", report.lines().take(20).collect::<Vec<_>>().join("\n"));
        assert!(report.starts_with("Changes in the whole model since"));
    }

    /// LC-440: EA's raw comparison of a package with a baseline, on a model copy (LOONAR_TEST_PACKAGE, LOONAR_TEST_BASELINE).
    #[test]
    #[ignore = "requires EA and the model from LOONAR_TEST_PROPS"]
    fn baseline_compare_raw_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let package: i64 = std::env::var("LOONAR_TEST_PACKAGE").expect("LOONAR_TEST_PACKAGE").parse().unwrap();
        let file = std::env::temp_dir().join("lc440-compare.xml");
        crate::ea::run_batch(&cfg, &[serde_json::json!({ "op": "baseline_compare", "package": package, "baseline": std::env::var("LOONAR_TEST_BASELINE").unwrap_or_default(), "file": file.display().to_string() })], &crate::ea::BatchOptions::default()).expect("compare");
        crate::ea::shutdown();
        let (added, deleted, changed, noise) = super::baseline_diff(&std::fs::read_to_string(&file).unwrap());
        println!("added {added:#?}
deleted {deleted:#?}
changed {changed:#?}
noise {noise}");
    }
}
