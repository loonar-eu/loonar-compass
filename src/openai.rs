//! OpenAI Chat Completions as a second model provider. The rest of the application speaks the Claude Messages
//! format; a request is translated to OpenAI's format here and the answer back (text and tool calls), so the tool
//! loop, approvals and history work the same with both providers.

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::time::Duration;

pub const API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Effort of the settings as OpenAI's `reasoning_effort` (which has no `xhigh`/`max`).
fn reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" | "xhigh" | "max" => Some("high"),
        _ => None,
    }
}

/// Text of a tool_result's content (a string or a list of blocks).
fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// One Claude content block as an OpenAI user content part (None for blocks OpenAI has no place for).
fn user_part(block: &Value) -> Option<Value> {
    match block["type"].as_str()? {
        "text" => Some(json!({ "type": "text", "text": block["text"] })),
        "image" => {
            let source = &block["source"];
            let url = format!("data:{};base64,{}", source["media_type"].as_str().unwrap_or("image/png"), source["data"].as_str().unwrap_or(""));
            Some(json!({ "type": "image_url", "image_url": { "url": url } }))
        }
        "document" => {
            let source = &block["source"];
            match source["type"].as_str() {
                Some("base64") => Some(json!({
                    "type": "file",
                    "file": {
                        "filename": block["title"].as_str().unwrap_or("document.pdf"),
                        "file_data": format!("data:{};base64,{}", source["media_type"].as_str().unwrap_or("application/pdf"), source["data"].as_str().unwrap_or("")),
                    }
                })),
                Some("text") => Some(json!({ "type": "text", "text": source["data"] })),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The Claude request (system + messages + tools) in OpenAI's Chat Completions format.
pub fn to_openai(model: &str, max_tokens: u32, effort: &str, system: &str, messages: &[Value], tools: Option<&Value>) -> Value {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        let content = &message["content"];
        if let Some(text) = content.as_str() {
            out.push(json!({ "role": role, "content": text }));
            continue;
        }
        let blocks = content.as_array().cloned().unwrap_or_default();
        if role == "assistant" {
            let text: Vec<&str> = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
            let calls: Vec<Value> = blocks
                .iter()
                .filter(|b| b["type"] == "tool_use")
                .map(|b| json!({ "id": b["id"], "type": "function", "function": { "name": b["name"], "arguments": b["input"].to_string() } }))
                .collect();
            let mut entry = Map::new();
            entry.insert("role".into(), json!("assistant"));
            entry.insert("content".into(), if text.is_empty() { Value::Null } else { json!(text.join("\n")) });
            if !calls.is_empty() {
                entry.insert("tool_calls".into(), json!(calls));
            }
            out.push(Value::Object(entry));
            continue;
        }
        // A user turn: tool results first (OpenAI wants them right after the call), then the rest of the content.
        let mut parts = Vec::new();
        for block in &blocks {
            if block["type"] == "tool_result" {
                let mut text = result_text(&block["content"]);
                if block["is_error"].as_bool() == Some(true) {
                    text = format!("ERROR: {text}");
                }
                out.push(json!({ "role": "tool", "tool_call_id": block["tool_use_id"], "content": text }));
                // Pictures returned by a tool (e.g. a diagram image) go to the model as a user message.
                if let Some(inner) = block["content"].as_array() {
                    parts.extend(inner.iter().filter(|b| b["type"] == "image").filter_map(user_part));
                }
            } else if let Some(part) = user_part(block) {
                parts.push(part);
            }
        }
        if !parts.is_empty() {
            out.push(json!({ "role": "user", "content": parts }));
        }
    }
    let mut body = json!({ "model": model, "messages": out, "max_completion_tokens": max_tokens });
    if let Some(level) = reasoning_effort(effort) {
        body["reasoning_effort"] = json!(level);
    }
    if let Some(list) = tools.and_then(Value::as_array) {
        let functions: Vec<Value> = list
            .iter()
            .map(|t| json!({ "type": "function", "function": { "name": t["name"], "description": t["description"], "parameters": t["input_schema"] } }))
            .collect();
        body["tools"] = json!(functions);
    }
    body
}

/// OpenAI's answer in the Claude response format (`content` with text and tool_use blocks, `stop_reason`, `usage`).
pub fn from_openai(response: &Value) -> Result<Value> {
    let choice = response["choices"].get(0).context("OpenAI returned no answer")?;
    let message = &choice["message"];
    let mut content = Vec::new();
    if let Some(text) = message["content"].as_str().filter(|t| !t.trim().is_empty()) {
        content.push(json!({ "type": "text", "text": text }));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
        let input: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
        content.push(json!({ "type": "tool_use", "id": call["id"], "name": call["function"]["name"], "input": input }));
    }
    if let Some(refusal) = message["refusal"].as_str().filter(|r| !r.is_empty()) {
        return Ok(json!({ "content": [{ "type": "text", "text": refusal }], "stop_reason": "refusal", "model": response["model"], "usage": {} }));
    }
    let stop = match choice["finish_reason"].as_str() {
        Some("tool_calls") => "tool_use",
        Some("length") => "max_tokens",
        _ if content.iter().any(|b| b["type"] == "tool_use") => "tool_use",
        _ => "end_turn",
    };
    Ok(json!({
        "content": content,
        "stop_reason": stop,
        "model": response["model"],
        "usage": { "input_tokens": response["usage"]["prompt_tokens"], "output_tokens": response["usage"]["completion_tokens"] },
    }))
}

/// One request to OpenAI, retried on temporary errors.
pub fn send(http: &reqwest::blocking::Client, key: &str, body: &Value) -> Result<Value> {
    let mut attempt = 1;
    loop {
        let response = http.post(API_URL).bearer_auth(key).json(body).send();
        let response = match response {
            Ok(r) => r,
            Err(e) if attempt < 4 => {
                let _ = e;
                std::thread::sleep(Duration::from_secs(2u64.pow(attempt)));
                attempt += 1;
                continue;
            }
            Err(e) => return Err(anyhow::Error::new(e).context(crate::i18n::tr("connecting to the OpenAI API"))),
        };
        let status = response.status().as_u16();
        let text = response.text().unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return from_openai(&value);
        }
        if (status == 429 || status >= 500) && attempt < 4 {
            std::thread::sleep(Duration::from_secs(2u64.pow(attempt)));
            attempt += 1;
            continue;
        }
        let message = value["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| text.chars().take(200).collect());
        match status {
            401 => bail!("{}", crate::i18n::tr("The OpenAI API rejected the API key (401). Check the OpenAI key in Settings → AI model.")),
            404 => bail!("{}", crate::i18n::fmt("The OpenAI API does not know the model {0} (404): {1}", &[body["model"].as_str().unwrap_or("").to_string(), message])),
            _ => bail!("{}", crate::i18n::fmt("the OpenAI API returned {0}: {1}", &[status.to_string(), message])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_loop_translates_both_ways() {
        let messages = vec![
            json!({ "role": "user", "content": [{ "type": "text", "text": "List packages" }, { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AAA" } }] }),
            json!({ "role": "assistant", "content": [{ "type": "thinking", "thinking": "" }, { "type": "text", "text": "Checking." }, { "type": "tool_use", "id": "t1", "name": "db_query", "input": { "sql": "select 1" } }] }),
            json!({ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": "1 row" }] }),
        ];
        let tools = json!([{ "name": "db_query", "description": "Query", "input_schema": { "type": "object", "properties": { "sql": { "type": "string" } } } }]);
        let body = to_openai("gpt-x", 1000, "max", "System.", &messages, Some(&tools));
        let out = body["messages"].as_array().unwrap();
        assert_eq!(out[0]["role"], "system");
        assert_eq!(out[1]["content"][1]["type"], "image_url");
        assert_eq!(out[2]["tool_calls"][0]["function"]["name"], "db_query");
        assert_eq!(out[2]["tool_calls"][0]["function"]["arguments"], "{\"sql\":\"select 1\"}");
        assert_eq!(out[3]["role"], "tool");
        assert_eq!(out[3]["tool_call_id"], "t1");
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["tools"][0]["function"]["parameters"]["type"], "object");

        let answer = json!({ "model": "gpt-x", "choices": [{ "finish_reason": "tool_calls", "message": { "content": null,
            "tool_calls": [{ "id": "c2", "type": "function", "function": { "name": "ea_write", "arguments": "{\"ops\":[]}" } }] } }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 } });
        let back = from_openai(&answer).unwrap();
        assert_eq!(back["stop_reason"], "tool_use");
        assert_eq!(back["content"][0]["type"], "tool_use");
        assert_eq!(back["content"][0]["input"]["ops"], json!([]));
        assert_eq!(back["usage"]["output_tokens"], 5);
    }
}
