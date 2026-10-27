//! Claude Messages API client. The model and effort come from the settings.

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2023-06-01";

/// Attempts for temporary errors (rate limit, overload, server or network errors).
const ATTEMPTS: u32 = 4;
/// Longest wait between attempts, also when the API asks for a longer one (retry-after).
const MAX_WAIT: Duration = Duration::from_secs(30);

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
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String, max_tokens: u32, workspace_id: String) -> Result<Self> {
        let http = reqwest::blocking::Client::builder().redirect(crate::net::same_origin_redirects())
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http, api_key, model, max_tokens, workspace_id: workspace_id.trim().to_string(), effort: String::new(), show_thinking: false, base_url: String::new(), oauth: None, openai: false, openai_url: String::new(), via_hub: false })
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

    pub fn with_effort(mut self, effort: &str) -> Self {
        self.effort = effort.trim().to_string();
        self
    }

    /// One Messages API call. Returns the whole response (with `content` and `stop_reason`),
    /// because the tool loop must see the `tool_use` blocks. Temporary errors are retried.
    pub fn send_value(&self, system: &str, messages: &[Value], tools: Option<&Value>) -> Result<Value> {
        if self.openai {
            let body = crate::openai::to_openai(&self.model, self.max_tokens, &self.effort, system, messages, tools);
            return crate::openai::send(&self.http, &self.openai_url, &self.api_key, &body);
        }
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            // The system prompt and the tools are the same in every request: cached by Anthropic, they cost a tenth.
            "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
            "messages": messages,
        });
        if let Some(t) = tools {
            let mut tools = t.clone();
            if let Some(last) = tools.as_array_mut().and_then(|list| list.last_mut()) {
                last["cache_control"] = json!({ "type": "ephemeral" });
            }
            body["tools"] = tools;
        }
        if !self.effort.is_empty() {
            body["output_config"] = json!({ "effort": self.effort });
        }
        if self.show_thinking {
            body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        }
        let mut attempt = 1;
        loop {
            match self.send_once(&body) {
                Ok(value) => return Ok(value),
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
