//! API specifications as a clean model (LC-397, /import-api): OpenAPI 3 and Swagger 2, AsyncAPI 2/3 and JSON Schema
//! (JSON or YAML) become a package with a class for each schema (attributes with types, descriptions, required
//! marked), an enumeration for each enum, associations between schemas that refer to each other, an interface per tag
//! with an operation per endpoint (the method and path as its alias, parameters, the response type), and a class
//! diagram. Elements already in the target package (same name) are left as they are, so importing again adds only
//! what is new. One ordinary save (approval card, undo).

use crate::config::AppConfig;
use crate::tools::query_rows;
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub fn definition() -> Value {
    json!({
        "name": "import_api",
        "description": "Imports an API specification attached to the conversation – OpenAPI 3 / Swagger 2, AsyncAPI or JSON Schema, in JSON or \
YAML – as a clean model in a new package under `package`: a class per schema (attributes with types and descriptions), an enumeration per \
enum, associations between schemas that refer to each other, an interface per tag with an operation per endpoint (alias = method and path, \
parameters, response type), AsyncAPI channels as an interface, and a class diagram laid out automatically. Names already in the new package \
are skipped. action preview (default) shows what would be created; apply saves it (one save the user approves, undoable).",
        "input_schema": { "type": "object", "properties": {
            "file": { "type": "string", "description": "File name of the attachment (.json, .yaml, .yml)" },
            "package": { "description": "package_id or exact name of the parent package" },
            "name": { "type": "string", "description": "Name of the new package (default: the API's title and version)" },
            "action": { "type": "string", "enum": ["preview", "apply"] }
        }, "required": ["file", "package"] }
    })
}

// ---------- a small YAML reader (the block style of API specifications) ----------

/// YAML as JSON: block mappings and sequences, plain and quoted scalars, flow [..] and {..} written in JSON style,
/// literal (|) and folded (>) blocks, comments. Anchors, aliases and tags are not supported (an error says so).
pub(crate) fn yaml_to_json(text: &str) -> Result<Value> {
    let lines: Vec<(usize, String)> = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty() && l.trim() != "---" && l.trim() != "...")
        .map(|l| (l.len() - l.trim_start().len(), strip_comment(l).trim_end().to_string()))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();
    if lines.iter().any(|(_, l)| {
        let t = l.trim_start();
        t.contains(": &") || t.contains(": *") || t.starts_with("- *") || t.starts_with("<<:")
    }) {
        bail!("YAML anchors and aliases (&, *, <<) are not supported: save the specification as JSON");
    }
    let mut pos = 0;
    let value = block(&lines, &mut pos, lines.first().map(|l| l.0).unwrap_or(0))?;
    Ok(value)
}

/// A comment (" #") outside quotes is cut off.
fn strip_comment(line: &str) -> &str {
    let (mut single, mut double) = (false, false);
    let bytes = line.as_bytes();
    for (i, c) in line.char_indices() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn block(lines: &[(usize, String)], pos: &mut usize, indent: usize) -> Result<Value> {
    let Some((first_indent, first)) = lines.get(*pos) else { return Ok(Value::Null) };
    if *first_indent < indent {
        return Ok(Value::Null);
    }
    let indent = *first_indent;
    if first.trim_start().starts_with("- ") || first.trim() == "-" {
        let mut list = Vec::new();
        while let Some((i, line)) = lines.get(*pos) {
            if *i != indent || !(line.trim_start().starts_with("- ") || line.trim() == "-") {
                break;
            }
            let rest = line.trim_start()[1..].trim_start().to_string();
            if rest.is_empty() {
                *pos += 1;
                list.push(block(lines, pos, indent + 1)?);
            } else if key_value(&rest).is_some() && !rest.starts_with('{') && !rest.starts_with('"') && !rest.starts_with('\'') {
                // "- key: value" starts a mapping whose other keys are indented to the key's column.
                let column = line.len() - rest.len();
                let mut own: Vec<(usize, String)> = vec![(column, rest)];
                *pos += 1;
                while let Some((i, l)) = lines.get(*pos) {
                    if *i < column {
                        break;
                    }
                    own.push((*i, l.clone()));
                    *pos += 1;
                }
                let mut p = 0;
                list.push(block(&own, &mut p, column)?);
            } else {
                *pos += 1;
                list.push(scalar_or_block(&rest, lines, pos, indent)?);
            }
        }
        return Ok(Value::Array(list));
    }
    let mut map = Map::new();
    while let Some((i, line)) = lines.get(*pos) {
        if *i != indent {
            if *i > indent {
                bail!("YAML: unexpected indentation at \"{}\"", line.trim());
            }
            break;
        }
        let Some((key, rest)) = key_value(line.trim_start()) else { bail!("YAML: expected \"key: value\" at \"{}\"", line.trim()) };
        *pos += 1;
        let value = if rest.is_empty() {
            match lines.get(*pos) {
                Some((next, l)) if *next > indent || (*next == indent && l.trim_start().starts_with("- ")) => block(lines, pos, if *next > indent { indent + 1 } else { indent })?,
                _ => Value::Null,
            }
        } else {
            scalar_or_block(&rest, lines, pos, indent)?
        };
        map.insert(key, value);
    }
    Ok(Value::Object(map))
}

/// "key: value" (the key may be quoted); None when the line is not a mapping entry.
fn key_value(text: &str) -> Option<(String, String)> {
    if let Some(quote) = text.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let end = text[1..].find(quote)? + 1;
        let rest = text[end + 1..].trim_start();
        let rest = rest.strip_prefix(':')?;
        return Some((text[1..end].to_string(), rest.trim().to_string()));
    }
    let mut search = 0;
    while let Some(i) = text[search..].find(':') {
        let at = search + i;
        let after = text[at + 1..].chars().next();
        if after.is_none() || after == Some(' ') || after == Some('\t') {
            let key = text[..at].trim();
            if key.is_empty() || key.starts_with('{') || key.starts_with('[') {
                return None;
            }
            return Some((key.to_string(), text[at + 1..].trim().to_string()));
        }
        search = at + 1;
    }
    None
}

fn scalar_or_block(text: &str, lines: &[(usize, String)], pos: &mut usize, indent: usize) -> Result<Value> {
    let t = text.trim();
    if t.starts_with('|') || t.starts_with('>') {
        let folded = t.starts_with('>');
        let mut parts = Vec::new();
        while let Some((i, l)) = lines.get(*pos) {
            if *i <= indent {
                break;
            }
            parts.push(l.trim().to_string());
            *pos += 1;
        }
        return Ok(Value::String(parts.join(if folded { " " } else { "\n" })));
    }
    if t.starts_with('[') || t.starts_with('{') {
        // Flow style: JSON, or with plain words (made into strings).
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            return Ok(v);
        }
        return Ok(flow(t));
    }
    Ok(scalar(t))
}

/// A simple flow collection with plain words: [a, b] or {a: 1, b: x}.
fn flow(text: &str) -> Value {
    let inner = &text[1..text.len().saturating_sub(1)];
    let parts: Vec<&str> = inner.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
    if text.starts_with('[') {
        Value::Array(parts.into_iter().map(scalar).collect())
    } else {
        Value::Object(parts.into_iter().filter_map(|p| key_value(p).or_else(|| p.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))).map(|(k, v)| (k, scalar(&v))).collect())
    }
}

fn scalar(text: &str) -> Value {
    let t = text.trim();
    if t.len() >= 2 && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\''))) {
        if t.starts_with('"') {
            return serde_json::from_str::<Value>(t).unwrap_or_else(|_| Value::String(t[1..t.len() - 1].to_string()));
        }
        return Value::String(t[1..t.len() - 1].replace("''", "'"));
    }
    match t {
        "~" | "null" | "Null" | "NULL" | "" => Value::Null,
        "true" | "True" | "TRUE" => Value::Bool(true),
        "false" | "False" | "FALSE" => Value::Bool(false),
        _ => {
            if let Ok(n) = t.parse::<i64>() {
                return json!(n);
            }
            if t.chars().any(|c| c.is_ascii_digit()) && !t.contains(':') {
                if let Ok(f) = t.parse::<f64>() {
                    return json!(f);
                }
            }
            Value::String(t.to_string())
        }
    }
}

// ---------- the specification as a model ----------

/// The name a $ref points to: "#/components/schemas/Order" → "Order".
fn ref_name(r: &str) -> String {
    r.rsplit('/').next().unwrap_or(r).to_string()
}

/// The type of a property in words EA shows: string, integer, Order, Order[], string (date-time)…
fn type_of(schema: &Value) -> (String, Option<String>) {
    if let Some(r) = schema["$ref"].as_str() {
        return (ref_name(r), Some(ref_name(r)));
    }
    for combined in ["allOf", "oneOf", "anyOf"] {
        if let Some(first) = schema[combined].as_array().and_then(|a| a.iter().find(|s| s["$ref"].is_string()).or(a.first())) {
            return type_of(first);
        }
    }
    match schema["type"].as_str() {
        Some("array") => {
            let (inner, target) = type_of(&schema["items"]);
            (format!("{inner}[]"), target)
        }
        Some(t) => (schema["format"].as_str().map(|f| format!("{t} ({f})")).unwrap_or_else(|| t.to_string()), None),
        None if schema["properties"].is_object() => ("object".into(), None),
        None => ("".into(), None),
    }
}

/// What the specification describes: (title, schemas, endpoints by interface, kind).
struct Spec {
    title: String,
    kind: &'static str,
    schemas: BTreeMap<String, Value>,
    /// interface name → operations {name, alias, notes, parameters, returns}
    interfaces: BTreeMap<String, Vec<Value>>,
}

fn read_spec(v: &Value) -> Spec {
    let title = format!("{} {}", v["info"]["title"].as_str().or(v["title"].as_str()).unwrap_or("API"), v["info"]["version"].as_str().unwrap_or("")).trim().to_string();
    let kind = if v["openapi"].is_string() {
        "OpenAPI"
    } else if v["swagger"].is_string() {
        "Swagger"
    } else if v["asyncapi"].is_string() {
        "AsyncAPI"
    } else {
        "JSON Schema"
    };
    let mut schemas = BTreeMap::new();
    for place in [&v["components"]["schemas"], &v["definitions"], &v["$defs"]] {
        for (name, s) in place.as_object().into_iter().flatten() {
            schemas.insert(name.clone(), s.clone());
        }
    }
    // A JSON Schema document that is itself one object.
    if kind == "JSON Schema" && v["properties"].is_object() {
        schemas.insert(v["title"].as_str().unwrap_or("Root").to_string(), v.clone());
    }
    let mut interfaces: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (path, item) in v["paths"].as_object().into_iter().flatten() {
        for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let op = &item[method];
            if !op.is_object() {
                continue;
            }
            let tag = op["tags"][0].as_str().unwrap_or("API").to_string();
            let name = op["operationId"].as_str().map(String::from).unwrap_or_else(|| {
                let words: String = path.split('/').filter(|p| !p.is_empty() && !p.starts_with('{')).map(|p| {
                    let mut c = p.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                }).collect();
                format!("{method}{words}")
            });
            let mut parameters: Vec<Value> = op["parameters"].as_array().into_iter().chain(item["parameters"].as_array()).flatten()
                .filter(|p| p["name"].is_string())
                .map(|p| json!({ "name": p["name"], "type": type_of(if p["schema"].is_object() { &p["schema"] } else { p }).0 }))
                .collect();
            if let Some(body) = op["requestBody"]["content"].as_object().and_then(|c| c.values().next()) {
                parameters.push(json!({ "name": "body", "type": type_of(&body["schema"]).0 }));
            }
            let ok = op["responses"].as_object().and_then(|r| r.iter().find(|(code, _)| code.starts_with('2')).map(|(_, r)| r.clone())).unwrap_or(Value::Null);
            let returns = ok["content"].as_object().and_then(|c| c.values().next()).map(|m| type_of(&m["schema"]).0).or_else(|| ok["schema"].is_object().then(|| type_of(&ok["schema"]).0)).unwrap_or_default();
            let notes = [op["summary"].as_str(), op["description"].as_str()].into_iter().flatten().collect::<Vec<_>>().join("\n\n");
            interfaces.entry(tag).or_default().push(json!({ "name": name, "alias": format!("{} {path}", method.to_uppercase()), "notes": notes, "parameters": parameters, "returns": returns }));
        }
    }
    // AsyncAPI: channels with their messages.
    for (channel, item) in v["channels"].as_object().into_iter().flatten() {
        for action in ["publish", "subscribe"] {
            let op = &item[action];
            if !op.is_object() {
                continue;
            }
            let payload = type_of(&op["message"]["payload"]).0;
            let payload = if payload.is_empty() { op["message"]["$ref"].as_str().map(ref_name).unwrap_or_default() } else { payload };
            interfaces.entry("Channels".into()).or_default().push(json!({ "name": op["operationId"].as_str().map(String::from).unwrap_or_else(|| format!("{action} {channel}")),
                "alias": format!("{} {channel}", action.to_uppercase()), "notes": op["summary"].as_str().unwrap_or(""), "parameters": [{ "name": "message", "type": payload }], "returns": "" }));
        }
    }
    for (name, m) in v["components"]["messages"].as_object().into_iter().flatten() {
        if m["payload"]["properties"].is_object() && !schemas.contains_key(name) {
            schemas.insert(name.clone(), m["payload"].clone());
        }
    }
    Spec { title, kind, schemas, interfaces }
}

fn plan(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let file = std::path::Path::new(input["file"].as_str().unwrap_or("")).file_name().context("file")?.to_string_lossy().to_string();
    let text = std::fs::read_to_string(crate::history::attachment_path(&cfg.attachments_dir, &file)).with_context(|| format!("there is no attachment {file}"))?;
    let doc: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => yaml_to_json(&text).with_context(|| format!("{file} is neither JSON nor YAML that Compass can read"))?,
    };
    let spec = read_spec(&doc);
    if spec.schemas.is_empty() && spec.interfaces.is_empty() {
        bail!("{file}: no schemas, paths or channels found – is it an OpenAPI, AsyncAPI or JSON Schema document?");
    }
    let (scope, _) = crate::model_usage::package_scope(cfg, &input["package"])?;
    let parent: i64 = scope.split('(').nth(1).and_then(|s| s.split([',', ')']).next()).and_then(|p| p.parse().ok()).context("package")?;
    let name = input["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| spec.title.clone());
    // An earlier import of the same API: its package is used again and names already in it are skipped.
    let quoted = crate::model_db::sql_text(cfg.model_source, &name);
    let existing_package: Option<i64> = query_rows(cfg, &format!("select package_id from t_package where parent_id = {parent} and name = '{quoted}'"))?.first().and_then(|r| r[0].parse().ok());
    let existing: Vec<String> = match existing_package {
        Some(p) => query_rows(cfg, &format!("select name from t_object where package_id = {p}"))?.into_iter().map(|r| r[0].to_lowercase()).collect(),
        None => Vec::new(),
    };
    let mut ops = Vec::new();
    let package = match existing_package {
        Some(p) => json!(p),
        None => {
            ops.push(json!({ "op": "create_package", "key": "pkg", "parent": parent, "name": name }));
            json!("#pkg")
        }
    };
    let diagram_name = format!("{name} – {}", spec.kind);
    // The diagram of an earlier import takes the new elements; otherwise a new one.
    let existing_diagram: Option<i64> = match existing_package {
        Some(p) => query_rows(cfg, &format!("select diagram_id from t_diagram where package_id = {p} and name = '{}'", crate::model_db::sql_text(cfg.model_source, &diagram_name)))?.first().and_then(|r| r[0].parse().ok()),
        None => None,
    };
    let diagram = match existing_diagram {
        Some(d) => json!(d),
        None => {
            ops.push(json!({ "op": "create_diagram", "key": "dia", "package": package, "name": diagram_name, "type": "Logical" }));
            json!("#dia")
        }
    };
    let key = |n: &str| format!("e_{}", n.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let (mut classes, mut enums, mut skipped, mut links) = (0, 0, Vec::new(), 0);
    let mut placed = 0;
    let mut place = |ops: &mut Vec<Value>, element: Value, placed: &mut i64| {
        ops.push(json!({ "op": "place", "diagram": diagram.clone(), "element": element, "left": 40 + (*placed % 5) * 220, "top": 40 + (*placed / 5) * 200, "width": 170, "height": 110 }));
        *placed += 1;
    };
    for (schema_name, s) in &spec.schemas {
        if existing.contains(&schema_name.to_lowercase()) {
            skipped.push(schema_name.clone());
            continue;
        }
        let k = key(schema_name);
        let notes = s["description"].as_str().unwrap_or("").to_string();
        if let Some(values) = s["enum"].as_array() {
            ops.push(json!({ "op": "create_element", "key": k, "package": package, "name": schema_name, "type": "Enumeration", "notes": notes }));
            for value in values {
                let text = value.as_str().map(String::from).unwrap_or_else(|| value.to_string());
                ops.push(json!({ "op": "create_attribute", "element": format!("#{k}"), "name": text }));
            }
            enums += 1;
        } else {
            ops.push(json!({ "op": "create_element", "key": k, "package": package, "name": schema_name, "type": "Class", "notes": notes }));
            let required: Vec<&str> = s["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let mut properties: Vec<(String, Value)> = s["properties"].as_object().map(|p| p.iter().map(|(a, b)| (a.clone(), b.clone())).collect()).unwrap_or_default();
            // allOf: the properties of the parts written inline.
            for part in s["allOf"].as_array().into_iter().flatten() {
                properties.extend(part["properties"].as_object().into_iter().flatten().map(|(a, b)| (a.clone(), b.clone())));
            }
            for (prop, p) in &properties {
                let (t, _) = type_of(p);
                let mut note = p["description"].as_str().unwrap_or("").to_string();
                if required.contains(&prop.as_str()) {
                    note = if note.is_empty() { "required".into() } else { format!("required. {note}") };
                }
                ops.push(json!({ "op": "create_attribute", "element": format!("#{k}"), "name": prop, "type": t, "notes": note }));
            }
            classes += 1;
        }
        place(&mut ops, json!(format!("#{k}")), &mut placed);
    }
    // Associations between schemas (to new ones in this import).
    for (schema_name, s) in &spec.schemas {
        if skipped.contains(schema_name) || s["enum"].is_array() {
            continue;
        }
        let mut props: Vec<(String, Value)> = s["properties"].as_object().map(|p| p.iter().map(|(a, b)| (a.clone(), b.clone())).collect()).unwrap_or_default();
        for part in s["allOf"].as_array().into_iter().flatten() {
            props.extend(part["properties"].as_object().into_iter().flatten().map(|(a, b)| (a.clone(), b.clone())));
        }
        for (prop, p) in props {
            if let (_, Some(target)) = type_of(&p) {
                if spec.schemas.contains_key(&target) && !skipped.contains(&target) && target != *schema_name {
                    ops.push(json!({ "op": "create_connector", "source": format!("#{}", key(schema_name)), "target": format!("#{}", key(&target)), "type": "Association", "name": prop }));
                    links += 1;
                }
            }
        }
        // allOf with a $ref: the schema extends the referenced one.
        for part in s["allOf"].as_array().into_iter().flatten() {
            if let Some(r) = part["$ref"].as_str() {
                let target = ref_name(r);
                if spec.schemas.contains_key(&target) && !skipped.contains(&target) {
                    ops.push(json!({ "op": "create_connector", "source": format!("#{}", key(schema_name)), "target": format!("#{}", key(&target)), "type": "Generalization" }));
                    links += 1;
                }
            }
        }
    }
    let (mut operations, mut interfaces) = (0, 0);
    for (interface, list) in &spec.interfaces {
        if existing.contains(&interface.to_lowercase()) {
            skipped.push(interface.clone());
            continue;
        }
        let k = key(&format!("if_{interface}"));
        ops.push(json!({ "op": "create_element", "key": k, "package": package, "name": interface, "type": "Interface" }));
        for o in list {
            ops.push(json!({ "op": "create_operation", "element": format!("#{k}"), "name": o["name"], "alias": o["alias"], "notes": o["notes"], "return_type": o["returns"], "parameters": o["parameters"] }));
            operations += 1;
        }
        interfaces += 1;
        place(&mut ops, json!(format!("#{k}")), &mut placed);
    }
    if placed == 0 {
        // Nothing new: nothing is saved.
        ops.clear();
    } else if existing_diagram.is_none() {
        ops.push(json!({ "op": "layout_diagram", "diagram": "#dia", "direction": "down", "spacing": 40 }));
    }
    let text = format!(
        "{} \"{}\" → package \"{name}\"{}: {classes} class(es), {enums} enumeration(s), {links} relationship(s), {} interface(s) with {operations} operation(s), diagram \"{diagram_name}\".{}",
        spec.kind,
        spec.title,
        if existing_package.is_some() { " (already there: only what is new)" } else { "" },
        interfaces,
        if skipped.is_empty() { String::new() } else { format!(" Already in the package, left as they are: {}.", skipped.join(", ")) }
    );
    Ok((ops, text))
}

/// preview: the plan in words; apply: the operations (saved by the caller).
pub fn run(input: &Value, cfg: &AppConfig) -> Result<(Vec<Value>, String)> {
    let (ops, text) = plan(input, cfg)?;
    if input["action"].as_str() == Some("apply") {
        Ok((ops, text))
    } else {
        Ok((Vec::new(), format!("{text}\n{} operation(s) – nothing saved yet; run with action apply to save.", ops.len())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PETSTORE: &str = r##"openapi: 3.0.1
info:
  title: Shop API   # a comment
  version: "1.2"
paths:
  /orders/{id}:
    get:
      tags: [Orders]
      operationId: getOrder
      summary: One order
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Order'
components:
  schemas:
    Order:
      type: object
      description: |
        An order of a customer.
        Paid or not.
      required: [id]
      properties:
        id:
          type: string
        status:
          $ref: "#/components/schemas/Status"
        lines:
          type: array
          items:
            $ref: '#/components/schemas/Line'
    Line:
      type: object
      properties:
        qty: {type: integer}
    Status:
      type: string
      enum:
        - new
        - paid
"##;

    #[test]
    fn yaml_of_an_api() {
        let v = yaml_to_json(PETSTORE).unwrap();
        assert_eq!(v["info"]["title"], "Shop API");
        assert_eq!(v["info"]["version"], "1.2");
        assert_eq!(v["paths"]["/orders/{id}"]["get"]["parameters"][0]["name"], "id");
        assert_eq!(v["paths"]["/orders/{id}"]["get"]["parameters"][0]["schema"]["type"], "string");
        assert_eq!(v["components"]["schemas"]["Order"]["required"][0], "id");
        assert_eq!(v["components"]["schemas"]["Status"]["enum"][1], "paid");
        assert_eq!(v["components"]["schemas"]["Line"]["properties"]["qty"]["type"], "integer");
        assert!(v["components"]["schemas"]["Order"]["description"].as_str().unwrap().contains("Paid or not."));
        assert!(yaml_to_json("a: &x 1\nb: *x").is_err());
        let spec = read_spec(&v);
        assert_eq!(spec.kind, "OpenAPI");
        assert_eq!(spec.schemas.len(), 3);
        let get = &spec.interfaces["Orders"][0];
        assert_eq!((get["name"].as_str(), get["alias"].as_str(), get["returns"].as_str()), (Some("getOrder"), Some("GET /orders/{id}"), Some("Order")));
        assert_eq!(type_of(&v["components"]["schemas"]["Order"]["properties"]["lines"]), ("Line[]".to_string(), Some("Line".to_string())));
    }

    /// `LOONAR_TEST_PROPS=… cargo test import_api_live -- --ignored --nocapture`: the API imported into EA, and again (nothing new).
    #[test]
    #[ignore = "requires EA and a copy of a test model"]
    fn import_api_live() {
        let mut cfg = AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").unwrap())).unwrap();
        cfg.attachments_dir = std::env::temp_dir().join(format!("loonar-api-{}", std::process::id()));
        std::fs::create_dir_all(&cfg.attachments_dir).unwrap();
        std::fs::write(cfg.attachments_dir.join("shop.yaml"), PETSTORE).unwrap();
        let root: i64 = query_rows(&cfg, "select package_id from t_package where parent_id = 0 order by package_id").unwrap()[0][0].parse().unwrap();
        let name = format!("ZZ_api_{}", chrono::Local::now().format("%H%M%S"));
        let input = json!({ "file": "shop.yaml", "package": root, "name": name, "action": "apply" });
        let (ops, text) = run(&input, &cfg).unwrap();
        println!("{text}");
        let done = crate::ea::run_batch(&cfg, &ops, &crate::ea::BatchOptions { rollback_key: Some("pkg".into()), keep_locks: false }).expect("import");
        let pkg = done["created"].as_array().unwrap().iter().find(|c| c["key"] == "pkg").unwrap()["id"].as_i64().unwrap();
        let q = |sql: &str| query_rows(&cfg, sql).unwrap();
        let names: Vec<String> = q(&format!("select name from t_object where package_id = {pkg} order by name")).into_iter().map(|r| r[0].clone()).collect();
        let alias = q(&format!("select o.style from t_operation o join t_object e on e.object_id = o.object_id where e.package_id = {pkg} and o.name = 'getOrder'"));
        let links = q(&format!("select c.connector_type from t_connector c join t_object e on e.object_id = c.start_object_id where e.package_id = {pkg}"));
        let (_, again) = run(&json!({ "file": "shop.yaml", "package": root, "name": name }), &cfg).unwrap();
        println!("{again}");
        crate::ea::run_batch(&cfg, &[json!({"op":"delete","package":pkg})], &crate::ea::BatchOptions::default()).unwrap();
        crate::ea::shutdown();
        let _ = std::fs::remove_dir_all(&cfg.attachments_dir);
        assert_eq!(names, ["Line", "Order", "Orders", "Status"], "{names:?}");
        assert_eq!(alias.first().map(|r| r[0].as_str()), Some("GET /orders/{id}"));
        assert_eq!(links.len(), 2, "{links:?}");
        assert!(again.contains("0 class(es)") && again.contains("left as they are"), "{again}");
    }
}
