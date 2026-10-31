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
            "package": { "description": "package_id or exact name (optional)" }
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
        Err(_) => bail!("since: give a date YYYY-MM-DD or a number of days"),
    }
}

/// A date literal of the model's engine (Oracle compares dates with DATE '…'; the others with the ISO text).
fn date_literal(source: ModelSource, date: chrono::NaiveDate) -> String {
    let text = date.format("%Y-%m-%d").to_string();
    if source == ModelSource::Oracle { format!("date '{text}'") } else { format!("'{text}'") }
}

pub fn run(input: &Value, cfg: &AppConfig) -> Result<String> {
    let since = since_date(input["since"].as_str().map(str::to_string).or_else(|| input["since"].as_i64().map(|d| d.to_string())).as_deref().unwrap_or(""), chrono::Local::now().date_naive())?;
    let from = date_literal(cfg.model_source, since);
    let author = input["author"].as_str().map(str::trim).filter(|a| !a.is_empty());
    let by = author.map(|a| format!("and lower(coalesce(x.author,'')) = lower('{}')", crate::model_db::sql_text(cfg.model_source, a))).unwrap_or_default();
    let (scope, title) = crate::model_usage::package_scope(cfg, &input["package"])?;
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
        "Changes in {title} since {since_text}{}: {} elements and {} diagrams.\n",
        author.map(|a| format!(" by {a}")).unwrap_or_default(),
        objects.len(),
        diagrams.len()
    );
    let day = |stamp: &str| stamp.get(..16).unwrap_or(stamp).replace('T', " ");
    let mut current = String::new();
    for (kind, rows) in [("element", &objects), ("diagram", &diagrams)] {
        for r in rows.iter().take(300) {
            if r[0] != current {
                current = r[0].clone();
                out.push_str(&format!("\nPackage \"{current}\":\n"));
            }
            let new = r[4].get(..10).is_some_and(|c| c >= since_text.as_str());
            out.push_str(&format!(
                "  - {} {kind} {} \"{}\"{} – {}\n",
                if new { "new" } else { "changed" },
                r[1],
                r[2],
                if r[3].is_empty() { String::new() } else { format!(" by {}", r[3]) },
                day(&r[5])
            ));
        }
        current.clear();
    }
    if objects.len() > 300 || diagrams.len() > 300 {
        out.push_str("\n(Only the first 300 of each are listed.)\n");
    }
    out.push_str("\nRelationships have no dates in EA and deleted objects leave no trace: compare with a baseline in EA for those.\n");
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

    /// LC-371: on a model copy: `LOONAR_TEST_PROPS=E:/livetest/undo/undo.properties cargo test what_changed_live -- --ignored --nocapture`
    #[test]
    #[ignore = "requires the model from LOONAR_TEST_PROPS"]
    fn what_changed_live() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("app.properties");
        let report = super::run(&serde_json::json!({ "since": "3650" }), &cfg).expect("what_changed");
        println!("{}", report.lines().take(20).collect::<Vec<_>>().join("\n"));
        assert!(report.starts_with("Changes in the whole model since"));
    }
}
