//! Miro: a diagram of the EA model sent to a Miro board as editable shapes and connectors in a frame named after
//! the diagram. Only from EA to Miro: what people change on the board does not come back to the model.
//!
//! The diagram comes from `diagram_editor::model` (positions as on the EA diagram). Sequence diagrams get lifelines
//! (a header and a dashed line) and messages in their EA order with their numbers. The frame goes to a free spot of
//! the board (`free_spot`), which the user can move on the board thumbnail before sending.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.miro.com/v2";
/// Space between the frame's edge and the diagram, and between a new frame and what is on the board.
const PADDING: f64 = 40.0;
const GAP: f64 = 120.0;
/// At most this many items of a board are read for finding a free spot and drawing the thumbnail.
const MAX_ITEMS: usize = 5000;

/// A rectangle on the board: top-left corner, width, height (Miro board units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    /// The two rectangles overlap, or are closer than `gap` to each other.
    pub fn near(&self, o: &Rect, gap: f64) -> bool {
        self.x < o.right() + gap && o.x < self.right() + gap && self.y < o.bottom() + gap && o.y < self.bottom() + gap
    }
}

pub struct Board {
    pub id: String,
    pub name: String,
    pub link: String,
}

/// What is on a board: top-level items (frames separately, they are drawn differently on the thumbnail).
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub rect: Rect,
    pub frame: bool,
}

/// A shape of the diagram, in frame coordinates (top-left corner of the frame is 0,0).
#[derive(Clone, Debug)]
pub struct Shape {
    pub key: String,
    pub rect: Rect,
    pub content: String,
    pub shape: &'static str,
    pub fill: String,
    pub border: String,
    pub font_size: u32,
    pub top: bool,
    /// An anchor for connectors (sequence diagrams): no fill, no border, no text.
    pub anchor: bool,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub caption: String,
    pub start_cap: &'static str,
    pub end_cap: &'static str,
    pub dashed: bool,
    pub line: &'static str,
}

/// A text without a shape: the name under an event, a gateway or an actor (as EA shows it).
#[derive(Clone, Debug)]
pub struct Label {
    pub rect: Rect,
    pub content: String,
}

/// The diagram as Miro items: the frame size, the shapes (in drawing order), the names under small shapes and the
/// connectors.
#[derive(Clone, Debug)]
pub struct Plan {
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<Shape>,
    pub labels: Vec<Label>,
    pub links: Vec<Link>,
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn short_stereotype(s: &str) -> &str {
    s.rsplit("::").next().unwrap_or(s)
}

/// The value of a tagged value of an element.
fn tag<'a>(el: &'a Value, name: &str) -> &'a str {
    el["tags"].as_array().into_iter().flatten().find(|t| t["name"] == name).and_then(|t| t["value"].as_str()).unwrap_or("")
}

/// BPMN kind as a symbol: a task type, an event definition or a gateway marker ("" when there is none).
fn bpmn_symbol(el: &Value) -> &'static str {
    let st = short_stereotype(el["stereotype"].as_str().unwrap_or(""));
    if st == "Activity" {
        return match tag(el, "taskType") {
            "User" => "👤",
            "Service" => "⚙️",
            "Manual" => "✋",
            "Script" => "📜",
            "Send" => "✉️",
            "Receive" => "📨",
            "BusinessRule" => "📋",
            _ => "",
        };
    }
    if st.ends_with("Event") {
        return match tag(el, "eventDefinition") {
            "Message" => "✉️",
            "Timer" => "⏱️",
            "Signal" => "△",
            "Error" => "⚡",
            "Escalation" => "⇧",
            "Conditional" => "☰",
            "Compensation" => "⏪",
            "Cancel" => "✖",
            "Terminate" => "●",
            "Multiple" => "⬠",
            "ParallelMultiple" => "✚",
            _ => "",
        };
    }
    if st == "Gateway" {
        return match tag(el, "gatewayType") {
            "Parallel" => "✚",
            "Inclusive" => "◯",
            "Complex" => "✱",
            "Event" => "⬠",
            _ if tag(el, "markerVisible") == "true" => "✕",
            _ => "",
        };
    }
    ""
}

/// Small shapes (events, gateways, actors, pseudostates) carry their name below, not inside.
fn name_below(el: &Value) -> bool {
    let kind = el["type"].as_str().unwrap_or("");
    let st = el["stereotype"].as_str().unwrap_or("");
    kind == "Event" || st.ends_with("Event") || kind == "Decision" || st.contains("Gateway") || kind == "Actor" || kind == "StateNode"
}

/// Miro shape, fill and border of an EA element.
fn look(el: &Value) -> (&'static str, &'static str, &'static str) {
    let kind = el["type"].as_str().unwrap_or("");
    let st = el["stereotype"].as_str().unwrap_or("");
    if st.ends_with("IntermediateEvent") {
        return ("circle", "#FFF8E6", "#9A6B00");
    }
    if kind == "Event" || st.ends_with("Event") {
        return if st.contains("End") { ("circle", "#FDECEA", "#B3261E") } else { ("circle", "#EAF7EE", "#1D7A46") };
    }
    if kind == "StateNode" {
        return ("circle", "#1F2A33", "#1F2A33");
    }
    if kind == "Decision" || st.contains("Gateway") {
        return ("rhombus", "#FFF8D6", "#B8A26A");
    }
    match kind {
        "Note" | "Text" => ("rectangle", "#FFFDE7", "#C9B458"),
        "Activity" | "Action" | "State" => ("round_rectangle", "#FFF8D6", "#B8A26A"),
        "UseCase" => ("circle", "#FFF8D6", "#B8A26A"),
        "Package" => ("rectangle", "#F1F5F9", "#55667A"),
        "Actor" => ("rectangle", "#FFFFFF", "#0E4A6B"),
        _ => ("rectangle", "#FFFFFF", "#0E4A6B"),
    }
}

/// Text of a shape: «stereotype», the name, and for classes their attributes and operations.
fn content(el: &Value) -> String {
    let mut out = String::new();
    let symbol = bpmn_symbol(el);
    if name_below(el) {
        return if symbol.is_empty() { String::new() } else { format!("<p>{symbol}</p>") };
    }
    if !symbol.is_empty() {
        out.push_str(&format!("<p>{symbol}</p>"));
    }
    let kind = el["type"].as_str().unwrap_or("");
    let st = el["stereotype"].as_str().unwrap_or("");
    if kind == "Interface" {
        out.push_str("<p>«interface»</p>");
    } else if !st.is_empty() && !matches!(short_stereotype(st), "Activity" | "StartEvent" | "EndEvent" | "IntermediateEvent" | "Gateway" | "Pool" | "Lane") {
        out.push_str(&format!("<p>«{}»</p>", esc(short_stereotype(st))));
    }
    out.push_str(&format!("<p><strong>{}</strong></p>", esc(el["name"].as_str().unwrap_or(""))));
    for list in ["attributes", "operations"] {
        let rows: Vec<String> = el[list].as_array().into_iter().flatten().filter_map(|r| r.as_str()).map(esc).collect();
        if !rows.is_empty() {
            out.push_str(&format!("<p>{}</p>", rows.join("<br>")));
        }
    }
    out
}

/// Line ends of an EA relationship: (start cap, end cap, dashed).
fn caps(c: &Value) -> (&'static str, &'static str, bool) {
    let st = c["stereotype"].as_str().unwrap_or("");
    let directed = c["props"]["direction"].as_str() == Some("Source -> Destination");
    match c["type"].as_str().unwrap_or("") {
        "Generalization" => ("none", "triangle", false),
        "Realisation" | "Realization" => ("none", "triangle", true),
        "Aggregation" => ("none", if c["subtype"] == "Strong" { "filled_diamond" } else { "diamond" }, false),
        "Composition" => ("none", "filled_diamond", false),
        "Dependency" | "Usage" | "Abstraction" => ("none", "arrow", true),
        "ControlFlow" | "ObjectFlow" | "StateFlow" | "InformationFlow" => ("none", "filled_triangle", false),
        "NoteLink" => ("none", "none", true),
        _ if st.ends_with("SequenceFlow") => ("none", "filled_triangle", false),
        _ if st.ends_with("MessageFlow") => ("oval", "arrow", true),
        _ => ("none", if directed { "arrow" } else { "none" }, false),
    }
}

/// The diagram from `diagram_editor::model` as Miro items.
pub fn plan(model: &Value) -> Plan {
    let title = model["diagram"]["name"].as_str().unwrap_or("").to_string();
    if model["diagram"]["type"] == "Sequence" {
        return sequence_plan(model, title);
    }
    let elements: Vec<&Value> = model["elements"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let min_x = elements.iter().map(|e| num(&e["left"])).fold(f64::INFINITY, f64::min);
    let min_y = elements.iter().map(|e| num(&e["top"])).fold(f64::INFINITY, f64::min);
    let (min_x, min_y) = if min_x.is_finite() { (min_x, min_y) } else { (0.0, 0.0) };
    // Larger shapes first, so frames and subprocesses stay behind their contents (as in the editor).
    let mut order = elements.clone();
    order.sort_by(|a, b| (num(&b["width"]) * num(&b["height"])).total_cmp(&(num(&a["width"]) * num(&a["height"]))));
    let mut shapes = Vec::new();
    let mut labels = Vec::new();
    let (mut width, mut height) = (0.0_f64, 0.0_f64);
    for el in order {
        let (shape, fill, border) = look(el);
        let fill = el["props"]["bgcolor"].as_str().filter(|c| !c.is_empty()).or(el["props"]["color"].as_str().filter(|c| !c.is_empty())).unwrap_or(fill);
        let rect = Rect { x: num(&el["left"]) - min_x + PADDING, y: num(&el["top"]) - min_y + PADDING, w: num(&el["width"]).max(20.0), h: num(&el["height"]).max(20.0) };
        width = width.max(rect.right() + PADDING);
        height = height.max(rect.bottom() + PADDING);
        let container = elements.iter().any(|o| o["parent"] == el["id"]);
        let classifier = matches!(el["type"].as_str(), Some("Class" | "Interface" | "Enumeration" | "DataType" | "Component"));
        let name = el["name"].as_str().unwrap_or("");
        if name_below(el) && !name.is_empty() {
            let label = Rect { x: rect.x + rect.w / 2.0 - 60.0, y: rect.bottom() + 4.0, w: 120.0, h: 24.0 };
            height = height.max(label.bottom() + PADDING);
            labels.push(Label { rect: label, content: esc(name) });
        }
        shapes.push(Shape {
            key: el["id"].to_string(),
            rect,
            content: content(el),
            shape,
            fill: fill.to_string(),
            border: border.to_string(),
            font_size: if rect.w < 60.0 || rect.h < 40.0 { 12 } else if rect.w < 140.0 || rect.h < 70.0 { 12 } else { 14 },
            top: container || classifier,
            anchor: false,
        });
    }
    let links = model["connectors"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let (start_cap, end_cap, dashed) = caps(c);
            Link {
                from: c["source"].to_string(),
                to: c["target"].to_string(),
                caption: c["name"].as_str().unwrap_or("").to_string(),
                start_cap,
                end_cap,
                dashed,
                line: "elbowed",
            }
        })
        .collect();
    Plan { title, width: width.max(200.0), height: height.max(120.0), shapes, labels, links }
}

/// Sequence diagram: a header for each lifeline with a dashed line down, and the messages as connectors between
/// anchors on the lifelines, from top to bottom in the order of EA (the message's position, else its number).
fn sequence_plan(model: &Value, title: String) -> Plan {
    const HEADER: f64 = 50.0;
    const STEP: f64 = 50.0;
    let lifelines: Vec<&Value> = model["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| !matches!(e["type"].as_str(), Some("InteractionFragment" | "Note" | "Text" | "Boundary")))
        .collect();
    let min_x = lifelines.iter().map(|e| num(&e["left"])).fold(f64::INFINITY, f64::min);
    let min_x = if min_x.is_finite() { min_x } else { 0.0 };
    let mut messages: Vec<&Value> = model["connectors"].as_array().into_iter().flatten().filter(|c| c["type"] == "Sequence").collect();
    messages.sort_by(|a, b| {
        let key = |c: &Value| (num(&c["props"]["y"]), num(&c["props"]["seq"]));
        let (ya, sa) = key(a);
        let (yb, sb) = key(b);
        if ya > 0.0 && yb > 0.0 { ya.total_cmp(&yb).then(sa.total_cmp(&sb)) } else { sa.total_cmp(&sb) }
    });
    let mut shapes = Vec::new();
    let mut centers = std::collections::HashMap::new();
    let top = PADDING;
    let first = top + HEADER + 40.0;
    let bottom = first + STEP * messages.len().max(1) as f64 + 20.0;
    let mut width: f64 = 0.0;
    for el in &lifelines {
        let w = num(&el["width"]).clamp(90.0, 220.0);
        let x = num(&el["left"]) - min_x + PADDING;
        let (shape, fill, border) = if el["type"] == "Actor" { ("rectangle", "#FFFFFF", "#0E4A6B") } else { ("rectangle", "#E6F0F6", "#0E4A6B") };
        let key = el["id"].to_string();
        shapes.push(Shape {
            key: format!("h{key}"),
            rect: Rect { x, y: top, w, h: HEADER },
            content: format!("<p><strong>{}</strong></p>", esc(el["name"].as_str().unwrap_or(""))),
            shape,
            fill: fill.into(),
            border: border.into(),
            font_size: 14,
            top: false,
            anchor: false,
        });
        let cx = x + w / 2.0;
        shapes.push(anchor(format!("b{key}"), cx, bottom));
        centers.insert(key, cx);
        width = width.max(x + w + PADDING);
    }
    let mut links: Vec<Link> = lifelines
        .iter()
        .map(|el| {
            let key = el["id"].to_string();
            Link { from: format!("h{key}"), to: format!("b{key}"), caption: String::new(), start_cap: "none", end_cap: "none", dashed: true, line: "straight" }
        })
        .collect();
    for (i, m) in messages.iter().enumerate() {
        let (from, to) = (m["source"].to_string(), m["target"].to_string());
        let (Some(&x1), Some(&x2)) = (centers.get(&from), centers.get(&to)) else { continue };
        let y = first + STEP * i as f64;
        let (a, b) = (format!("m{i}a"), format!("m{i}b"));
        shapes.push(anchor(a.clone(), x1, y));
        // A message to itself goes a little down on the same lifeline.
        shapes.push(anchor(b.clone(), x2, if from == to { y + STEP * 0.6 } else { y }));
        let reply = m["props"]["return"] == true;
        let asynchronous = m["props"]["sort"].as_str() == Some("Asynchronous");
        let number = m["props"]["seq"].as_i64().filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or_else(|| (i + 1).to_string());
        let name = m["name"].as_str().unwrap_or("");
        links.push(Link {
            from: a,
            to: b,
            caption: format!("{number}: {name}"),
            start_cap: "none",
            end_cap: if reply || asynchronous { "arrow" } else { "filled_triangle" },
            dashed: reply,
            line: if from == to { "curved" } else { "straight" },
        });
    }
    Plan { title, width: width.max(200.0), height: bottom + PADDING, shapes, labels: vec![], links }
}

fn anchor(key: String, cx: f64, cy: f64) -> Shape {
    Shape {
        key,
        rect: Rect { x: cx - 5.0, y: cy - 5.0, w: 10.0, h: 10.0 },
        content: String::new(),
        shape: "circle",
        fill: "#FFFFFF".into(),
        border: "#FFFFFF".into(),
        font_size: 10,
        top: false,
        anchor: true,
    }
}

/// A free spot for a frame of this size: next to what is already on the board, not over it, as close as possible
/// to the middle of the existing content. An empty board: around 0,0.
pub fn free_spot(items: &[Item], w: f64, h: f64) -> Rect {
    if items.is_empty() {
        return Rect { x: -w / 2.0, y: -h / 2.0, w, h };
    }
    let rects: Vec<Rect> = items.iter().map(|i| i.rect).collect();
    let left = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
    let top = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
    let right = rects.iter().map(|r| r.right()).fold(f64::NEG_INFINITY, f64::max);
    let bottom = rects.iter().map(|r| r.bottom()).fold(f64::NEG_INFINITY, f64::max);
    let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
    // Candidates: to the right of and below every item, and right of / below the whole content.
    let mut candidates = vec![(right + GAP, top), (left, bottom + GAP)];
    for r in &rects {
        candidates.push((r.right() + GAP, r.y));
        candidates.push((r.x, r.bottom() + GAP));
        candidates.push((r.x - GAP - w, r.y));
        candidates.push((r.x, r.y - GAP - h));
    }
    let free = |x: f64, y: f64| {
        let spot = Rect { x, y, w, h };
        rects.iter().all(|r| !spot.near(r, GAP / 2.0))
    };
    let distance = |&(x, y): &(f64, f64)| ((x + w / 2.0 - cx).powi(2) + (y + h / 2.0 - cy).powi(2)).sqrt();
    let best = candidates.into_iter().filter(|&(x, y)| free(x, y)).min_by(|a, b| distance(a).total_cmp(&distance(b)));
    let (x, y) = best.unwrap_or((right + GAP, top));
    Rect { x, y, w, h }
}

/// Connection to the Miro REST API with an access token (from a Miro app installed in the user's team).
pub struct Client {
    token: String,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(token: &str) -> Result<Self> {
        let token = token.trim().to_string();
        if token.is_empty() {
            bail!("{}", crate::i18n::tr("no Miro access token in the settings"));
        }
        let http = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects())
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .context(crate::i18n::tr("HTTP client"))?;
        Ok(Self { token, http })
    }

    fn call(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value> {
        let response = request.bearer_auth(&self.token).header("Accept", "application/json").send().context(crate::i18n::tr("connection to Miro"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if status.as_u16() == 401 {
            bail!("{}", crate::i18n::tr("Miro rejected the access token (check it in the settings)"));
        }
        if status.as_u16() == 403 {
            bail!("{}", crate::i18n::tr("Miro refused access: the token needs the boards:read and boards:write permissions, and the board must allow editing"));
        }
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or(text);
            bail!("{}", crate::i18n::fmt("Miro answered {0}: {1}", &[status.as_u16().to_string(), message.chars().take(300).collect()]));
        }
        Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).context("Miro")? })
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        self.call(self.http.get(format!("{API}{path}")).query(query))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.call(self.http.post(format!("{API}{path}")).json(body))
    }

    /// Settings test: the token works. Returns the number of boards it can see.
    pub fn check(&self) -> Result<u64> {
        let page = self.get("/boards", &[("limit", "1".into())])?;
        Ok(page["total"].as_u64().unwrap_or(0))
    }

    /// Boards the token can see, the most recently changed first (with the name filter, when given).
    pub fn boards(&self, query: &str) -> Result<Vec<Board>> {
        let mut params = vec![("limit", "50".to_string()), ("sort", "last_modified".to_string())];
        if !query.trim().is_empty() {
            params.push(("query", query.trim().to_string()));
        }
        let page = self.get("/boards", &params)?;
        Ok(page["data"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| Board {
                id: b["id"].as_str().unwrap_or("").to_string(),
                name: b["name"].as_str().unwrap_or("").to_string(),
                link: b["viewLink"].as_str().unwrap_or("").to_string(),
            })
            .filter(|b| !b.id.is_empty())
            .collect())
    }

    /// Top-level items of a board as rectangles (Miro gives centres; these are top-left corners).
    pub fn items(&self, board: &str) -> Result<Vec<Item>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut params = vec![("limit", "50".to_string())];
            if let Some(c) = &cursor {
                params.push(("cursor", c.clone()));
            }
            let page = self.get(&format!("/boards/{}/items", urlencode(board)), &params)?;
            for item in page["data"].as_array().into_iter().flatten() {
                if !item["parent"].is_null() || item["type"] == "connector" {
                    continue;
                }
                let (w, h) = (num(&item["geometry"]["width"]), num(&item["geometry"]["height"]));
                let (x, y) = (num(&item["position"]["x"]), num(&item["position"]["y"]));
                if w <= 0.0 && h <= 0.0 {
                    continue;
                }
                out.push(Item { rect: Rect { x: x - w / 2.0, y: y - h / 2.0, w, h: if h > 0.0 { h } else { w } }, frame: item["type"] == "frame" });
            }
            cursor = page["cursor"].as_str().filter(|c| !c.is_empty()).map(str::to_string);
            if cursor.is_none() || out.len() >= MAX_ITEMS {
                break;
            }
        }
        Ok(out)
    }

    /// Sends the diagram: the frame at `at` (its top-left corner on the board), then the shapes, then the
    /// connectors. Returns the link that opens the board at the frame.
    pub fn send(&self, board: &Board, plan: &Plan, at: Rect, progress: &dyn Fn(usize, usize)) -> Result<String> {
        let path = format!("/boards/{}", urlencode(&board.id));
        let total = 1 + plan.shapes.len() + plan.labels.len() + plan.links.len();
        let mut done = 0;
        let frame = self.post(
            &format!("{path}/frames"),
            &json!({
                "data": { "title": plan.title, "format": "custom", "type": "freeform" },
                "style": { "fillColor": "#ffffff" },
                "position": { "x": at.x + plan.width / 2.0, "y": at.y + plan.height / 2.0, "origin": "center" },
                "geometry": { "width": plan.width, "height": plan.height },
            }),
        )?;
        let frame_id = frame["id"].as_str().ok_or_else(|| anyhow!("{}", crate::i18n::tr("Miro did not create the frame")))?.to_string();
        done += 1;
        progress(done, total);
        let mut ids = std::collections::HashMap::new();
        for s in &plan.shapes {
            let style = if s.anchor {
                json!({ "fillOpacity": "0.0", "borderOpacity": "0.0", "borderWidth": "1.0" })
            } else {
                json!({
                    "fillColor": s.fill, "borderColor": s.border, "borderWidth": "1.0", "fontSize": s.font_size.to_string(),
                    "textAlign": "center", "textAlignVertical": if s.top { "top" } else { "middle" }, "color": "#1a1a1a",
                })
            };
            let created = self.post(
                &format!("{path}/shapes"),
                &json!({
                    "data": { "content": s.content, "shape": s.shape },
                    "style": style,
                    // With a parent, the position is relative to the parent's top-left corner.
                    "position": { "x": s.rect.x + s.rect.w / 2.0, "y": s.rect.y + s.rect.h / 2.0, "origin": "center" },
                    "geometry": { "width": s.rect.w, "height": s.rect.h },
                    "parent": { "id": frame_id },
                }),
            )?;
            if let Some(id) = created["id"].as_str() {
                ids.insert(s.key.clone(), id.to_string());
            }
            done += 1;
            progress(done, total);
        }
        for l in &plan.labels {
            self.post(
                &format!("{path}/texts"),
                &json!({
                    "data": { "content": l.content },
                    "style": { "fontSize": "12", "textAlign": "center", "color": "#2A3A47" },
                    "position": { "x": l.rect.x + l.rect.w / 2.0, "y": l.rect.y + l.rect.h / 2.0, "origin": "center" },
                    "geometry": { "width": l.rect.w },
                    "parent": { "id": frame_id },
                }),
            )?;
            done += 1;
            progress(done, total);
        }
        for l in &plan.links {
            let (Some(from), Some(to)) = (ids.get(&l.from), ids.get(&l.to)) else { continue };
            let mut body = json!({
                "startItem": { "id": from, "snapTo": "auto" },
                "endItem": { "id": to, "snapTo": "auto" },
                "shape": l.line,
                "style": {
                    "strokeColor": "#3B4B59", "strokeWidth": "1.5", "strokeStyle": if l.dashed { "dashed" } else { "normal" },
                    "startStrokeCap": l.start_cap, "endStrokeCap": l.end_cap, "fontSize": "12",
                },
            });
            if !l.caption.is_empty() {
                body["captions"] = json!([{ "content": esc(&l.caption), "position": "50%" }]);
            }
            self.post(&format!("{path}/connectors"), &body)?;
            done += 1;
            progress(done, total);
        }
        Ok(if board.link.is_empty() { String::new() } else { format!("{}?moveToWidget={frame_id}", board.link) })
    }
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_diagram() -> Value {
        json!({
            "diagram": { "id": 8, "name": "Domain model", "type": "Logical" },
            "elements": [
                { "id": 1, "name": "Customer", "type": "Class", "stereotype": "", "parent": 0, "left": 100, "top": 80, "width": 160, "height": 90,
                  "attributes": ["- name: String"], "operations": ["+ register()"], "props": { "bgcolor": "#FFE599", "color": "" } },
                { "id": 2, "name": "Order", "type": "Class", "stereotype": "entity", "parent": 0, "left": 400, "top": 80, "width": 160, "height": 90,
                  "attributes": [], "operations": [], "props": {} },
            ],
            "connectors": [
                { "id": 5, "type": "Association", "stereotype": "", "name": "places", "source": 1, "target": 2, "props": { "direction": "Source -> Destination" } },
                { "id": 6, "type": "Generalization", "stereotype": "", "name": "", "source": 2, "target": 1, "props": {} },
            ],
        })
    }

    #[test]
    fn a_class_diagram_becomes_shapes_and_connectors() {
        let p = plan(&class_diagram());
        assert_eq!(p.title, "Domain model");
        assert_eq!(p.shapes.len(), 2);
        let customer = p.shapes.iter().find(|s| s.key == "1").unwrap();
        assert_eq!((customer.rect.x, customer.rect.y), (PADDING, PADDING), "the diagram starts at the frame's padding");
        assert_eq!(customer.fill, "#FFE599", "the colour on the diagram is kept");
        assert!(customer.content.contains("<strong>Customer</strong>") && customer.content.contains("- name: String"));
        let order = p.shapes.iter().find(|s| s.key == "2").unwrap();
        assert!(order.content.starts_with("<p>«entity»</p>"));
        assert_eq!(p.width, 400.0 - 100.0 + 160.0 + 2.0 * PADDING);
        assert_eq!(p.links[0].end_cap, "arrow");
        assert_eq!(p.links[1].end_cap, "triangle");
    }

    #[test]
    fn bpmn_kinds_and_names_of_small_shapes() {
        let model = json!({
            "diagram": { "id": 3, "name": "Order process", "type": "BPMN2.0::Business Process" },
            "elements": [
                { "id": 1, "name": "Order placed", "type": "Event", "stereotype": "StartEvent", "left": 10, "top": 50, "width": 30, "height": 30,
                  "tags": [{ "name": "eventDefinition", "value": "Message" }] },
                { "id": 2, "name": "Check", "type": "Activity", "stereotype": "Activity", "left": 80, "top": 40, "width": 110, "height": 60,
                  "tags": [{ "name": "taskType", "value": "User" }] },
                { "id": 3, "name": "OK?", "type": "Decision", "stereotype": "Gateway", "left": 230, "top": 50, "width": 40, "height": 40,
                  "tags": [{ "name": "gatewayType", "value": "Parallel" }] },
            ],
            "connectors": [],
        });
        let p = plan(&model);
        let shape = |k: &str| p.shapes.iter().find(|s| s.key == k).unwrap();
        assert_eq!(shape("1").content, "<p>✉️</p>", "an event shows its kind, its name goes below");
        assert!(shape("2").content.starts_with("<p>👤</p>") && shape("2").content.contains("Check"));
        assert_eq!(shape("3").content, "<p>✚</p>");
        let mut names: Vec<&str> = p.labels.iter().map(|l| l.content.as_str()).collect();
        names.sort();
        assert_eq!(names, ["OK?", "Order placed"], "names of the event and the gateway, not of the task");
        assert!(p.labels.iter().all(|l| l.rect.y > PADDING + 30.0), "names are below the shapes");
    }

    #[test]
    fn a_sequence_diagram_keeps_the_order_of_messages() {
        let model = json!({
            "diagram": { "id": 9, "name": "Checkout", "type": "Sequence" },
            "elements": [
                { "id": 1, "name": "Client", "type": "Actor", "left": 50, "top": 50, "width": 100, "height": 600 },
                { "id": 2, "name": "Shop", "type": "Sequence", "left": 300, "top": 50, "width": 120, "height": 600 },
            ],
            "connectors": [
                { "id": 11, "type": "Sequence", "name": "confirm()", "source": 2, "target": 1, "props": { "seq": 2, "y": 0 } },
                { "id": 10, "type": "Sequence", "name": "order()", "source": 1, "target": 2, "props": { "seq": 1, "y": 0 } },
                { "id": 12, "type": "Sequence", "name": "ok", "source": 2, "target": 1, "props": { "seq": 3, "y": 0, "return": true } },
                { "id": 13, "type": "Sequence", "name": "check()", "source": 2, "target": 2, "props": { "seq": 4, "y": 0 } },
            ],
        });
        let p = plan(&model);
        let messages: Vec<&Link> = p.links.iter().filter(|l| !l.caption.is_empty()).collect();
        let captions: Vec<&str> = messages.iter().map(|l| l.caption.as_str()).collect();
        assert_eq!(captions, ["1: order()", "2: confirm()", "3: ok", "4: check()"]);
        assert!(messages[2].dashed, "a return message is dashed");
        assert_eq!(messages[3].line, "curved", "a message to itself");
        // Two lifelines: a header, a dashed line to the bottom anchor.
        assert_eq!(p.links.iter().filter(|l| l.caption.is_empty() && l.dashed).count(), 2);
        let y = |k: &str| p.shapes.iter().find(|s| s.key == k).unwrap().rect.y;
        assert!(y("m0a") < y("m1a") && y("m1a") < y("m2a"), "messages go down in their order");
    }

    #[test]
    fn a_free_spot_does_not_cover_what_is_on_the_board() {
        assert_eq!(free_spot(&[], 400.0, 200.0), Rect { x: -200.0, y: -100.0, w: 400.0, h: 200.0 });
        let items = [
            Item { rect: Rect { x: 0.0, y: 0.0, w: 1000.0, h: 600.0 }, frame: true },
            Item { rect: Rect { x: 1200.0, y: 0.0, w: 300.0, h: 300.0 }, frame: false },
        ];
        let spot = free_spot(&items, 500.0, 400.0);
        for i in &items {
            assert!(!spot.near(&i.rect, 1.0), "{spot:?} overlaps {:?}", i.rect);
        }
        // Close to the content, not far away.
        assert!(spot.x < 2500.0 && spot.y < 2000.0, "{spot:?}");
    }
}
