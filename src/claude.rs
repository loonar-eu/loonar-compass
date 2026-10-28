//! Claude Messages API client. The model and effort come from the settings.

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
const COUNT_URL: &str = "https://api.anthropic.com/v1/messages/count_tokens";

/// The prompt's tokens when they cannot be counted (OpenAI, the hub, a gateway): 2.2 characters a token over the text
/// of the request (measured 2026-10-28 on a Polish conversation with the tools: 2.23; the earlier 3 left calls above
/// 100k), an image or document as 1 500.
pub fn estimated_tokens(system: &str, messages: &[Value], tools: Option<&Value>) -> u64 {
    fn walk(v: &Value, chars: &mut u64, files: &mut u64) {
        match v {
            Value::String(t) => *chars += t.chars().count() as u64,
            Value::Array(list) => list.iter().for_each(|x| walk(x, chars, files)),
            Value::Object(map) => {
                if matches!(map.get("type").and_then(Value::as_str), Some("image" | "document")) {
                    *files += 1;
                    return;
                }
                for (key, x) in map {
                    *chars += key.len() as u64;
                    walk(x, chars, files);
                }
            }
            _ => *chars += 4,
        }
    }
    let (mut chars, mut files) = (system.chars().count() as u64, 0);
    messages.iter().for_each(|m| walk(m, &mut chars, &mut files));
    if let Some(t) = tools {
        walk(t, &mut chars, &mut files);
    }
    chars * 5 / 11 + files * 1_500
}
pub const API_VERSION: &str = "2023-06-01";

/// Clearing old tool results on the server (LC-301): the beta, how many of the newest tool uses stay, and the least
/// a clear must remove to be worth the cache it rewrites.
const CONTEXT_MANAGEMENT_BETA: &str = "context-management-2025-06-27";
const TOOL_USES_KEPT: u64 = 6;
const CLEAR_AT_LEAST: u64 = 20_000;

/// The cache of the tools and the system prompt: one hour (a longer duration must come before shorter ones).
static PREFIX_CACHE: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| json!({ "type": "ephemeral", "ttl": "1h" }));

/// Attempts for temporary errors (rate limit, overload, server or network errors).
const ATTEMPTS: u32 = 4;
/// Longest wait between attempts, also when the API asks for a longer one (retry-after).
const MAX_WAIT: Duration = Duration::from_secs(30);

/// Cache breakpoints on the conversation (LC-300): the end of the newest message and of the message before the
/// previous answer. The tools and the system prompt have their own two, so a request has at most the four the API
/// allows. Each round of the assistant's loop sends the whole conversation again (history, tool calls and results such
/// as source files read); with these marks everything sent before is read from the cache at a tenth of the price and
/// only the new part is written. The second mark keeps the previous round's cache entry reachable when one round adds
/// many blocks (the API looks back only a limited number of blocks from a mark). The content itself is not changed.
pub fn mark_conversation_cache(messages: &mut [Value]) {
    // A message as a plain string and as one text block are the same to the model; every message is sent as blocks, so
    // a message reads the same in every round whether it carries a mark or not (the cache matches the request byte for
    // byte up to a mark).
    for m in messages.iter_mut() {
        if let Some(text) = m["content"].as_str().map(str::to_string) {
            if !text.is_empty() {
                m["content"] = json!([{ "type": "text", "text": text }]);
            }
        }
    }
    let n = messages.len();
    for index in [n.checked_sub(1), n.checked_sub(3)].into_iter().flatten() {
        mark_message(&mut messages[index]);
    }
}

fn mark_message(message: &mut Value) {
    let Some(blocks) = message["content"].as_array_mut() else { return };
    // The last block that can carry a mark: not reasoning, not an empty text.
    let target = blocks.iter_mut().rev().find(|b| {
        let kind = b["type"].as_str().unwrap_or("");
        !matches!(kind, "thinking" | "redacted_thinking") && !(kind == "text" && b["text"].as_str().is_none_or(|t| t.trim().is_empty()))
    });
    if let Some(block) = target {
        block["cache_control"] = json!({ "type": "ephemeral" });
    }
}

pub struct ClaudeClient {
    http: reqwest::blocking::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    /// Workspace ID: needed when the API key is not scoped to a workspace (empty = no header).
    workspace_id: String,
    /// Effort level sent as output_config.effort (empty = the model's default).
    effort: String,
    /// Ask for a readable summary of the reasoning (thinking display "summarized").
    show_thinking: bool,
    /// Another address of the Messages API (Azure AI Foundry, a gateway); empty = api.anthropic.com.
    base_url: String,
    /// OAuth2 client credentials (token URL, client id, scope, secret) used instead of the API key.
    oauth: Option<(String, String, String, String)>,
    /// The provider answering: OpenAI (the request is translated) or Claude.
    openai: bool,
    /// OpenAI-compatible endpoint (empty = OpenAI).
    openai_url: String,
    /// Claude through the team hub (the hub keeps the key): its errors are about the hub, not about Anthropic.
    via_hub: bool,
    /// What the calls are for, in the cost log (chat, summary, agent, translation…).
    purpose: &'static str,
    /// Above this many prompt tokens the API itself clears the oldest tool results (context editing), see
    /// `with_tool_clearing`.
    tool_clearing: Option<u64>,
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String, max_tokens: u32, workspace_id: String) -> Result<Self> {
        let http = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects())
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http, api_key, model, max_tokens, workspace_id: workspace_id.trim().to_string(), effort: String::new(), show_thinking: false, base_url: String::new(), oauth: None, openai: false, openai_url: String::new(), via_hub: false, purpose: "chat", tool_clearing: None })
    }

    /// The client of the provider chosen in the settings (Claude or OpenAI), with its key, model and effort.
    pub fn from_config(cfg: &crate::config::AppConfig, max_tokens: u32) -> Result<Self> {
        use crate::config::secrets;
        if cfg.uses_openai() {
            // A local endpoint (Ollama, LM Studio) works without a key.
            let local = !cfg.openai_url.trim().is_empty() && crate::jira::check_base_url(cfg.openai_url.trim()).is_ok() && !cfg.openai_url.trim().to_ascii_lowercase().starts_with("https://");
            let key = if !cfg.ai_oauth_token_url.trim().is_empty() {
                // A company gateway with OAuth2: the token takes the place of the key (fresh for every answer).
                crate::auth::client_credentials(cfg.ai_oauth_token_url.trim(), cfg.ai_oauth_client_id.trim(), &secrets::get(secrets::AI_OAUTH_SECRET)?, cfg.ai_oauth_scope.trim())?
            } else {
                match secrets::get(secrets::OPENAI_API_KEY) {
                    Ok(key) => key,
                    Err(_) if local => String::new(),
                    Err(e) => return Err(e),
                }
            };
            let mut client = Self::new(key, cfg.openai_model.clone(), max_tokens, String::new())?.with_effort(&cfg.claude_effort);
            client.openai = true;
            client.openai_url = cfg.openai_url.trim().to_string();
            return Ok(client);
        }
        // The team policy may route Claude through the hub (the hub keeps the key): the hub token is the key there, and
        // no local key or OAuth2 secret is needed.
        if let Some((hub, token)) = crate::hub::via_hub(cfg, "ai") {
            let mut client = Self::new(token, cfg.claude_model.clone(), max_tokens, String::new())?.with_effort(&cfg.claude_effort).with_thinking(cfg.show_thinking);
            client.base_url = format!("{hub}/ai");
            client.via_hub = true;
            return Ok(client);
        }
        let oauth = if cfg.ai_oauth_token_url.trim().is_empty() {
            None
        } else {
            Some((cfg.ai_oauth_token_url.trim().to_string(), cfg.ai_oauth_client_id.trim().to_string(), cfg.ai_oauth_scope.trim().to_string(), secrets::get(secrets::AI_OAUTH_SECRET)?))
        };
        // With OAuth2 there may be no API key at all.
        let key = match secrets::get(secrets::CLAUDE_API_KEY) {
            Ok(key) => key,
            Err(_) if oauth.is_some() => String::new(),
            Err(e) => return Err(e),
        };
        let mut client = Self::new(key, cfg.claude_model.clone(), max_tokens, cfg.claude_workspace_id.clone())?.with_effort(&cfg.claude_effort).with_thinking(cfg.show_thinking);
        let base = cfg.claude_url.trim().trim_end_matches('/').to_string();
        if !base.is_empty() && crate::net::check_service_url(&base).is_err() {
            anyhow::bail!("{}", crate::i18n::fmt("claude.url {0} must be https:// (http:// only on this computer)", &[base]));
        }
        client.base_url = base;
        client.oauth = oauth;
        Ok(client)
    }

    /// Effort for the requests of this client (the model's default when empty).
    /// The reasoning summary in the answer (thinking blocks with text); the model thinks either way.
    pub fn with_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    /// Long tool loops (LC-301): when the prompt passes `trigger` tokens, the API clears the oldest tool results itself
    /// (the newest stay, and the calls stay visible), so a request does not cross Claude Haiku 5.5's 100k-token price
    /// tier. Done on the server, this is not an edit of the conversation: the model's earlier reasoning stays valid,
    /// which clearing on the client would break. Only towards Anthropic directly (gateways may not know the beta).
    pub fn with_tool_clearing(mut self, trigger: u64) -> Self {
        self.tool_clearing = Some(trigger);
        self
    }

    /// What the calls are for, in the cost log.
    pub fn with_purpose(mut self, purpose: &'static str) -> Self {
        self.purpose = purpose;
        self
    }

    pub fn with_effort(mut self, effort: &str) -> Self {
        self.effort = effort.trim().to_string();
        self
    }

    /// One Messages API call. Returns the whole response (with `content` and `stop_reason`),
    /// because the tool loop must see the `tool_use` blocks. Temporary errors are retried.
    pub fn send_value(&self, system: &str, messages: &[Value], tools: Option<&Value>) -> Result<Value> {
        let started = std::time::Instant::now();
        if self.openai {
            let body = crate::openai::to_openai(&self.model, self.max_tokens, &self.effort, system, messages, tools);
            let value = crate::openai::send(&self.http, &self.openai_url, &self.api_key, &body)?;
            crate::usage::record(&crate::usage::Call::from_usage(&self.model, self.purpose, &value["usage"], 0, started.elapsed().as_millis() as u64));
            return Ok(value);
        }
        let body = self.request_body(system, messages, tools);
        let mut attempt = 1;
        loop {
            match self.send_once(&body) {
                Ok(value) => {
                    // What the call cost: the usage of the answer, the retries before it and the time (LC-300).
                    crate::usage::record(&crate::usage::Call::from_usage(&self.model, self.purpose, &value["usage"], attempt - 1, started.elapsed().as_millis() as u64));
                    return Ok(value);
                }
                Err(Failure::Retry { error, wait }) if attempt < ATTEMPTS => {
                    let backoff = Duration::from_secs(2u64.pow(attempt));
                    std::thread::sleep(wait.unwrap_or(backoff).min(MAX_WAIT));
                    attempt += 1;
                    let _ = error;
                }
                Err(Failure::Retry { error, .. }) | Err(Failure::Final(error)) => return Err(error),
            }
        }
    }

    /// The prompt's size in tokens as the model counts them (LC-306): Anthropic's count_tokens endpoint, free and exact
    /// (Polish text, code and tool schemas take far more tokens than characters / 3 suggest). None through OpenAI, the
    /// hub or a gateway, or when the count fails: then `estimated_tokens` is used.
    pub fn count_tokens(&self, system: &str, messages: &[Value], tools: Option<&Value>) -> Option<u64> {
        if self.openai || self.via_hub || !self.base_url.is_empty() {
            return None;
        }
        let mut body = self.request_body(system, messages, tools);
        if let Some(map) = body.as_object_mut() {
            for key in ["max_tokens", "context_management", "output_config"] {
                map.remove(key);
            }
        }
        let mut request = self.http.post(COUNT_URL).header("anthropic-version", API_VERSION).timeout(Duration::from_secs(30));
        if let Some((token_url, client_id, scope, secret)) = &self.oauth {
            request = request.bearer_auth(crate::auth::client_credentials(token_url, client_id, secret, scope).ok()?);
        } else {
            request = request.header("x-api-key", &self.api_key);
        }
        if !self.workspace_id.is_empty() {
            request = request.header("anthropic-workspace-id", &self.workspace_id);
        }
        let response = request.json(&body).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.json::<Value>().ok()?["input_tokens"].as_u64()
    }

    /// The Messages API request: the system prompt and the tools cached, the conversation with its cache marks.
    fn request_body(&self, system: &str, messages: &[Value], tools: Option<&Value>) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            // The system prompt and the tools are the same in every request: cached by Anthropic, they cost a tenth. Kept
            // for an hour (LC-302): between turns the user often reads, works in EA or waits longer than 5 minutes, and a
            // rewrite of this part costs twelve reads. The conversation marks stay at 5 minutes (written every round).
            "system": [{ "type": "text", "text": system, "cache_control": PREFIX_CACHE.clone() }],
            "messages": messages,
        });
        if let Some(list) = body["messages"].as_array_mut() {
            mark_conversation_cache(list);
        }
        if let Some(t) = tools {
            let mut tools = t.clone();
            if let Some(last) = tools.as_array_mut().and_then(|list| list.last_mut()) {
                last["cache_control"] = PREFIX_CACHE.clone();
            }
            body["tools"] = tools;
        }
        if let (Some(trigger), true) = (self.tool_clearing, tools.is_some() && self.base_url.is_empty() && !self.via_hub && !self.openai) {
            body["context_management"] = json!({ "edits": [{
                "type": "clear_tool_uses_20250919",
                "trigger": { "type": "input_tokens", "value": trigger },
                "keep": { "type": "tool_uses", "value": TOOL_USES_KEPT },
                "clear_at_least": { "type": "input_tokens", "value": CLEAR_AT_LEAST },
                "exclude_tools": ["ask_user", "task_list", "report_gap"]
            }] });
        }
        if !self.effort.is_empty() {
            body["output_config"] = json!({ "effort": self.effort });
        }
        if self.show_thinking {
            body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        }
        body
    }

    fn send_once(&self, body: &Value) -> std::result::Result<Value, Failure> {

        let url = if self.base_url.is_empty() { API_URL.to_string() } else { format!("{}/v1/messages", self.base_url) };
        let mut request = self.http.post(&url).header("anthropic-version", API_VERSION);
        if let Some((token_url, client_id, scope, secret)) = &self.oauth {
            let token = crate::auth::client_credentials(token_url, client_id, secret, scope).map_err(Failure::Final)?;
            request = request.bearer_auth(token);
        } else if self.base_url.is_empty() {
            request = request.header("x-api-key", &self.api_key);
        } else {
            // Azure AI Foundry and gateways take the key as api-key; Anthropic-compatible ones as x-api-key.
            request = request.header("x-api-key", &self.api_key).header("api-key", &self.api_key);
        }
        if !self.workspace_id.is_empty() {
            request = request.header("anthropic-workspace-id", &self.workspace_id);
        }
        if body.get("context_management").is_some() {
            request = request.header("anthropic-beta", CONTEXT_MANAGEMENT_BETA);
        }
        let response = request.json(body).send();
        let response = match response {
            Ok(response) => response,
            // Network problems (no connection, proxy, timeout) are worth another attempt.
            Err(e) => {
                return Err(Failure::Retry {
                    error: anyhow::Error::new(e).context(crate::i18n::tr("connecting to the Claude API")),
                    wait: None,
                })
            }
        };
        let status = response.status().as_u16();
        let wait = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = response.text().unwrap_or_default();
        let parsed: Option<Value> = serde_json::from_str(&text).ok();
        if (200..300).contains(&status) {
            return parsed.ok_or_else(|| Failure::Final(anyhow::anyhow!("{}", crate::i18n::tr("the API response is not JSON"))));
        }
        if self.via_hub {
            let message = parsed.as_ref().and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).map(String::from));
            let error = match (status, message) {
                (401, _) => crate::i18n::tr("Your sign-in to the team hub has expired: sign in again in Settings → Team (Compass Hub).").to_string(),
                (_, Some(message)) => crate::i18n::fmt("The team hub refused the request ({0}): {1}", &[status.to_string(), message]),
                (_, None) => crate::i18n::fmt("The team hub or the server in front of it does not answer properly ({0}). Try again in a moment or tell the administrator of the hub.", &[status.to_string()]),
            };
            let error = anyhow::anyhow!("{error}");
            // Only a hub that is temporarily unavailable is asked again; a refusal (sign-in, limit, no key) is final.
            return Err(if matches!(status, 502..=504) { Failure::Retry { error, wait } } else { Failure::Final(error) });
        }
        // An HTML page instead of JSON usually comes from a proxy or firewall between the computer and the API.
        let Some(value) = parsed else {
            let snippet: String = text.chars().filter(|c| !c.is_control()).take(200).collect();
            let error = anyhow::anyhow!(
                "{}",
                crate::i18n::fmt(
                    "The Claude API returned {0} with a response that is not from the API (a proxy or firewall may be blocking api.anthropic.com): {1}",
                    &[status.to_string(), snippet]
                )
            );
            return Err(if status >= 500 { Failure::Retry { error, wait } } else { Failure::Final(error) });
        };
        let message = value["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
        let error = match status {
            401 => crate::i18n::tr("The Claude API rejected the API key (401). Check the key in Settings → AI model.").to_string(),
            403 => crate::i18n::fmt("The API key has no access to this operation (403): {0}", &[message]),
            404 => crate::i18n::fmt("The Claude API does not know the model {0} (404): {1}", &[self.model.clone(), message]),
            400 if message.contains("workspace") => crate::i18n::fmt("The API key is not assigned to a workspace: enter the Workspace ID from the Anthropic Console in Settings → 1. Claude ({0}).", &[message]),
            413 => crate::i18n::tr("The request is too large for the Claude API (413): the attachments may be too big.").to_string(),
            429 => crate::i18n::tr("The Claude API rate limit was reached (429). Wait a moment and try again.").to_string(),
            500..=599 => crate::i18n::fmt("The Claude API is overloaded or temporarily unavailable ({0}). Try again in a moment.", &[status.to_string()]),
            _ => crate::i18n::fmt("the Claude API returned {0}: {1}", &[status.to_string(), message]),
        };
        let error = anyhow::anyhow!("{error}");
        if status == 408 || status == 429 || status >= 500 {
            Err(Failure::Retry { error, wait })
        } else {
            Err(Failure::Final(error))
        }
    }
}

enum Failure {
    /// Temporary: worth another attempt (after `wait` when the API said how long).
    Retry { error: anyhow::Error, wait: Option<Duration> },
    Final(anyhow::Error),
}

/// Fails when a response has no usable content: e.g. the model refused (`stop_reason: "refusal"`).
pub fn check_refusal(response: &Value) -> Result<()> {
    if response["stop_reason"].as_str() == Some("refusal") {
        bail!("{}", crate::i18n::tr("The model declined to answer this request. Rephrase it or ask about something else."));
    }
    Ok(())
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn marks(v: &Value) -> usize {
        match v {
            Value::Object(o) => usize::from(o.contains_key("cache_control")) + o.values().map(marks).sum::<usize>(),
            Value::Array(a) => a.iter().map(marks).sum(),
            _ => 0,
        }
    }

    fn client() -> ClaudeClient {
        ClaudeClient::new("k".into(), "claude-haiku-5-5".into(), 1000, String::new()).unwrap()
    }

    #[test]
    fn a_tool_loop_round_has_four_marks_and_the_content_unchanged() {
        let messages = vec![
            json!({"role": "user", "content": "List the controllers"}),
            json!({"role": "assistant", "content": [{"type": "thinking", "thinking": "", "signature": "s"}, {"type": "text", "text": "Reading."}, {"type": "tool_use", "id": "t1", "name": "code_read", "input": {"path": "C:/a.java"}}]}),
            json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "class A {}"}]}),
            json!({"role": "assistant", "content": [{"type": "tool_use", "id": "t2", "name": "code_read", "input": {"path": "C:/b.java"}}]}),
            json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t2", "content": "class B {}"}, {"type": "text", "text": ""}]}),
        ];
        let tools = json!([{"name": "a", "input_schema": {}}, {"name": "b", "input_schema": {}}]);
        let body = client().request_body("system", &messages, Some(&tools));
        assert_eq!(marks(&body), 4, "tools, system and two in the conversation: the API allows four");
        // The tools and the system prompt for an hour, the conversation for 5 minutes (longer ones first, as the API needs).
        assert_eq!(body["tools"][1]["cache_control"]["ttl"], "1h");
        assert_eq!(body["system"][0]["cache_control"]["ttl"], "1h");
        assert!(body["messages"][4]["content"][0]["cache_control"]["ttl"].is_null());
        let sent = body["messages"].as_array().unwrap();
        // The newest message: its last block that can carry a mark (the empty text cannot).
        assert!(sent[4]["content"][0]["cache_control"].is_object() && sent[4]["content"][1]["cache_control"].is_null());
        // The message before the previous answer.
        assert!(sent[2]["content"][0]["cache_control"].is_object());
        // A string becomes one text block everywhere, so it reads the same with and without a mark.
        assert_eq!(sent[0]["content"], json!([{"type": "text", "text": "List the controllers"}]));
        // Reasoning blocks are never marked; the caller's messages are not changed.
        assert!(sent[1]["content"][0]["cache_control"].is_null());
        assert_eq!(marks(&json!(messages)), 0);
        let without_marks = |v: &Value| { let mut v = v.clone(); strip(&mut v); v };
        assert_eq!(without_marks(&sent[2]), messages[2]);
    }

    fn strip(v: &mut Value) {
        match v {
            Value::Object(o) => { o.remove("cache_control"); o.values_mut().for_each(strip); }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }

    /// Real API: a new prefix (system + tools) is written for an hour, the conversation for 5 minutes.
    #[test]
    #[ignore = "requires an API key (LOONAR_TEST_PROPS)"]
    fn prefix_is_written_for_an_hour() {
        let cfg = crate::config::AppConfig::load(std::path::Path::new(&std::env::var("LOONAR_TEST_PROPS").expect("LOONAR_TEST_PROPS"))).expect("settings");
        let client = ClaudeClient::from_config(&cfg, 64).unwrap();
        let system = format!("Test {}. {}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), "You answer with one word. ".repeat(400));
        let tools = json!([{"name": "noop", "description": "Does nothing.", "input_schema": {"type": "object", "properties": {}}}]);
        let r = client.send_value(&system, &[json!({"role": "user", "content": "Say ok."})], Some(&tools)).unwrap();
        println!("USAGE {}", r["usage"]);
        assert!(r["usage"]["cache_creation"]["ephemeral_1h_input_tokens"].as_u64().unwrap_or(0) > 0, "the prefix is written for an hour");
    }

    #[test]
    fn tool_clearing_only_towards_anthropic_with_tools() {
        let tools = json!([{"name": "a", "input_schema": {}}]);
        let m = [json!({"role": "user", "content": "x"})];
        let on = client().with_tool_clearing(85_000).request_body("s", &m, Some(&tools));
        let edit = &on["context_management"]["edits"][0];
        assert_eq!((edit["type"].as_str(), edit["trigger"]["value"].as_u64()), (Some("clear_tool_uses_20250919"), Some(85_000)));
        assert!(edit["exclude_tools"].as_array().unwrap().contains(&json!("ask_user")));
        assert!(client().with_tool_clearing(85_000).request_body("s", &m, None).get("context_management").is_none(), "no tools, nothing to clear");
        assert!(client().request_body("s", &m, Some(&tools)).get("context_management").is_none());
    }

    #[test]
    fn short_requests_stay_within_the_limit() {
        let one = client().request_body("s", &[json!({"role": "user", "content": "Summarise it."})], None);
        assert_eq!(marks(&one), 2);
        let empty = client().request_body("s", &[json!({"role": "user", "content": ""})], None);
        assert_eq!(marks(&empty), 1, "an empty message gets no mark");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn refusal_is_reported() {
        assert!(super::check_refusal(&serde_json::json!({ "stop_reason": "refusal", "content": [] })).is_err());
        assert!(super::check_refusal(&serde_json::json!({ "stop_reason": "end_turn", "content": [] })).is_ok());
    }
}

#[cfg(test)]
mod model_probe {
    /// The reasoning summary comes back, and a tool round trip with thinking blocks passed back works (real API).
    /// `cargo test reasoning_summary -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn reasoning_summary() {
        use serde_json::json;
        let key = crate::config::secrets::get(crate::config::secrets::CLAUDE_API_KEY).unwrap();
        let client = super::ClaudeClient::new(key, crate::config::CLAUDE_MODEL.to_string(), 4000, String::new()).unwrap().with_effort("high").with_thinking(true);
        let tools = json!([{ "name": "count_classes", "description": "Counts the classes in the model.", "input_schema": { "type": "object", "properties": {} } }]);
        let mut messages = vec![json!({ "role": "user", "content": "Think carefully: a model has classes with inheritance cycles A->B->C->A and an interface I realised by A and C. Which modelling problems are there and in which order would you fix them? First count the classes with the tool." })];
        let first = client.send_value("You are an assistant for a UML model.", &messages, Some(&tools)).unwrap();
        let thinking: Vec<&str> = first["content"].as_array().unwrap().iter().filter(|b| b["type"] == "thinking").filter_map(|b| b["thinking"].as_str()).collect();
        println!("stop {} thinking blocks {}: {:?}", first["stop_reason"], thinking.len(), thinking.iter().map(|t| t.chars().take(120).collect::<String>()).collect::<Vec<_>>());
        assert_eq!(first["stop_reason"], "tool_use");
        let id = first["content"].as_array().unwrap().iter().find(|b| b["type"] == "tool_use").unwrap()["id"].clone();
        messages.push(json!({ "role": "assistant", "content": first["content"].clone() }));
        messages.push(json!({ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": "7 classes" }] }));
        let second = client.send_value("You are an assistant for a UML model.", &messages, Some(&tools)).unwrap();
        let text: String = second["content"].as_array().unwrap().iter().filter_map(|b| b["text"].as_str()).collect();
        println!("answer: {text}");
        assert!(text.contains('7'));
    }

    /// Every offered model answers with an effort level (real API, key from the Credential Manager).
    #[test]
    #[ignore]
    fn models_accept_effort() {
        let key = crate::config::secrets::get(crate::config::secrets::CLAUDE_API_KEY).expect("key");
        for model in crate::config::CLAUDE_MODELS {
            let client = super::ClaudeClient::new(key.clone(), model.to_string(), 64, String::new()).unwrap().with_effort("low");
            let tools = serde_json::json!([{"name":"ping","description":"Answers pong.","input_schema":{"type":"object","properties":{}}}]);
            let result = client.send_value("Answer in one word.", &[serde_json::json!({"role":"user","content":"Say hi"})], Some(&tools));
            println!("{model}: {}", match &result { Ok(v) => format!("ok stop={} ", v["stop_reason"]), Err(e) => format!("ERROR {e:#}") });
        }
    }
}
