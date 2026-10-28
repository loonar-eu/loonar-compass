//! What every call to the AI model costs (LC-300): the four token counts of the response's `usage`, the price of the
//! model at the prompt's length, retries and time. Each call goes to a local log (`usage/<day>.jsonl` next to the
//! settings: numbers only, no content), to the day's statistics, and to the totals of the answer being made (shown as
//! its last step). Prices: https://platform.claude.com/docs/en/about-claude/pricing (read 2026-10-28).

use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Above this many prompt tokens (input + cache writes + cache reads) Claude Haiku 5.5 bills the whole request at its
/// long-prompt prices.
pub const HAIKU_LONG_PROMPT: u64 = 100_000;
/// How long the call log is kept.
const KEEP_DAYS: i64 = 90;

/// USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prices {
    pub input: f64,
    pub write_5m: f64,
    pub write_1h: f64,
    pub read: f64,
    pub output: f64,
}

const fn p(input: f64, write_5m: f64, write_1h: f64, read: f64, output: f64) -> Prices {
    Prices { input, write_5m, write_1h, read, output }
}

/// The prices of a Claude model for a prompt of `prompt` tokens; None for a model not in the table (e.g. OpenAI).
pub fn prices(model: &str, prompt: u64) -> Option<Prices> {
    let m = model.trim().to_lowercase();
    let m = m.strip_prefix("anthropic.").unwrap_or(&m);
    Some(match m {
        _ if m.starts_with("claude-haiku-5-5") => {
            if prompt > HAIKU_LONG_PROMPT { p(0.50, 0.625, 1.0, 0.05, 2.50) } else { p(0.10, 0.125, 0.20, 0.01, 0.50) }
        }
        _ if m.starts_with("claude-haiku-4-5") => p(1.0, 1.25, 2.0, 0.10, 5.0),
        _ if m.starts_with("claude-sonnet-5-5") => p(2.0, 2.50, 4.0, 0.10, 10.0),
        _ if m.starts_with("claude-sonnet-5") => p(2.0, 2.50, 4.0, 0.20, 10.0),
        _ if m.starts_with("claude-sonnet-4") => p(3.0, 3.75, 6.0, 0.30, 15.0),
        _ if m.starts_with("claude-opus-5-5") => p(4.0, 5.0, 8.0, 0.20, 20.0),
        _ if m.starts_with("claude-opus-5") || m.starts_with("claude-opus-4") => p(5.0, 6.25, 10.0, 0.50, 25.0),
        _ if m.starts_with("claude-fable-5-1") || m.starts_with("claude-mythos-5-1") => p(10.0, 12.50, 20.0, 0.25, 50.0),
        _ if m.starts_with("claude-fable-5") || m.starts_with("claude-mythos-5") => p(10.0, 12.50, 20.0, 1.0, 50.0),
        _ => return None,
    })
}

/// One call to the model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Call {
    pub model: String,
    /// What the call was for: chat (the assistant's loop), summary, agent, translation, check…
    pub purpose: String,
    pub input: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
    pub retries: u32,
    pub millis: u64,
}

impl Call {
    /// The counts of a Messages API response's `usage` (the cache writes split by duration when the API gives it).
    pub fn from_usage(model: &str, purpose: &str, usage: &Value, retries: u32, millis: u64) -> Self {
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        let written = n(&usage["cache_creation_input_tokens"]);
        let (w5, w1) = match usage["cache_creation"].as_object() {
            Some(split) => (split.get("ephemeral_5m_input_tokens").map(n).unwrap_or(0), split.get("ephemeral_1h_input_tokens").map(n).unwrap_or(0)),
            None => (written, 0),
        };
        // An older answer without the split: everything as 5-minute writes.
        let (w5, w1) = if w5 + w1 == 0 { (written, 0) } else { (w5, w1) };
        Self {
            model: model.to_string(),
            purpose: purpose.to_string(),
            input: n(&usage["input_tokens"]),
            cache_write_5m: w5,
            cache_write_1h: w1,
            cache_read: n(&usage["cache_read_input_tokens"]),
            output: n(&usage["output_tokens"]),
            retries,
            millis,
        }
    }

    /// All input tokens of the request: what decides Haiku 5.5's price tier.
    pub fn prompt(&self) -> u64 {
        self.input + self.cache_write_5m + self.cache_write_1h + self.cache_read
    }

    pub fn long_prompt(&self) -> bool {
        self.model.to_lowercase().contains("haiku-5-5") && self.prompt() > HAIKU_LONG_PROMPT
    }

    /// The cost in USD at the model's price for this prompt length (None for a model without a price).
    pub fn usd(&self) -> Option<f64> {
        let p = prices(&self.model, self.prompt())?;
        Some(
            (self.input as f64 * p.input
                + self.cache_write_5m as f64 * p.write_5m
                + self.cache_write_1h as f64 * p.write_1h
                + self.cache_read as f64 * p.read
                + self.output as f64 * p.output)
                / 1_000_000.0,
        )
    }

    /// The share of the prompt read from the cache.
    pub fn cache_share(&self) -> f64 {
        if self.prompt() == 0 { 0.0 } else { self.cache_read as f64 / self.prompt() as f64 }
    }
}

/// The totals of several calls (an answer, a task).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    pub calls: u32,
    pub input: u64,
    pub cache_write: u64,
    /// The part of `cache_write` kept for an hour.
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
    pub long_prompts: u32,
    pub largest_prompt: u64,
    pub retries: u32,
    pub millis: u64,
    pub usd: f64,
    /// Some call had no price (the USD total is then too low).
    pub unpriced: bool,
}

impl Totals {
    pub fn add(&mut self, c: &Call) {
        self.calls += 1;
        self.input += c.input;
        self.cache_write += c.cache_write_5m + c.cache_write_1h;
        self.cache_write_1h += c.cache_write_1h;
        self.cache_read += c.cache_read;
        self.output += c.output;
        self.long_prompts += u32::from(c.long_prompt());
        self.largest_prompt = self.largest_prompt.max(c.prompt());
        self.retries += c.retries;
        self.millis += c.millis;
        match c.usd() {
            Some(usd) => self.usd += usd,
            None => self.unpriced = true,
        }
    }

    pub fn cache_share(&self) -> f64 {
        let prompt = self.input + self.cache_write + self.cache_read;
        if prompt == 0 { 0.0 } else { self.cache_read as f64 / prompt as f64 }
    }

    /// One line for the steps of an answer, e.g. "7 calls · 412k in (91% from cache) · 3.1k out · $0.0123".
    pub fn line(&self) -> String {
        let k = |n: u64| if n >= 10_000 { format!("{}k", n / 1000) } else { format!("{:.1}k", n as f64 / 1000.0) };
        let mut parts = vec![
            crate::i18n::fmt("{0} calls to the model", &[self.calls.to_string()]),
            crate::i18n::fmt("{0} input ({1}% from the cache, {2} written to it)", &[k(self.input + self.cache_write + self.cache_read), format!("{:.0}", self.cache_share() * 100.0), k(self.cache_write)]),
            crate::i18n::fmt("{0} output", &[k(self.output)]),
        ];
        if self.long_prompts > 0 {
            parts.push(crate::i18n::fmt("{0} over 100k tokens", &[self.long_prompts.to_string()]));
        }
        if self.retries > 0 {
            parts.push(crate::i18n::fmt("{0} retries", &[self.retries.to_string()]));
        }
        parts.push(if self.unpriced && self.usd == 0.0 { crate::i18n::tr("cost unknown").to_string() } else { format!("${:.4}", self.usd) });
        parts.join(" · ")
    }
}

static DIR: OnceLock<PathBuf> = OnceLock::new();

thread_local! {
    /// The calls of the answer being made in this thread (each answer has its own worker thread).
    static ANSWER: RefCell<Totals> = RefCell::new(Totals::default());
}

/// The settings folder: the call log goes to its `usage` subfolder.
pub fn init(settings_dir: &std::path::Path) {
    let dir = settings_dir.join("usage");
    let _ = DIR.set(dir.clone());
    prune(&dir);
}

/// A new answer starts in this thread: its totals start from zero.
pub fn begin_answer() {
    ANSWER.with(|a| *a.borrow_mut() = Totals::default());
}

/// The totals of the answer made in this thread so far.
pub fn answer_totals() -> Totals {
    ANSWER.with(|a| a.borrow().clone())
}

/// Records one call: the answer's totals, the day's statistics and the local log.
pub fn record(call: &Call) {
    ANSWER.with(|a| a.borrow_mut().add(call));
    crate::telemetry::usage(call);
    if let Some(dir) = DIR.get() {
        let _ = append(dir, call);
    }
}

fn append(dir: &std::path::Path, c: &Call) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let now = chrono::Local::now();
    let line = json!({
        "time": now.to_rfc3339(),
        "model": c.model,
        "purpose": c.purpose,
        "input": c.input,
        "cache_write_5m": c.cache_write_5m,
        "cache_write_1h": c.cache_write_1h,
        "cache_read": c.cache_read,
        "output": c.output,
        "prompt": c.prompt(),
        "long_prompt": c.long_prompt(),
        "usd": c.usd(),
        "retries": c.retries,
        "ms": c.millis,
    });
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{}.jsonl", now.format("%Y-%m-%d"))))?;
    writeln!(file, "{line}")
}

fn prune(dir: &std::path::Path) {
    let oldest = (chrono::Local::now() - chrono::Duration::days(KEEP_DAYS)).format("%Y-%m-%d").to_string();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jsonl") && name.as_str() < oldest.as_str() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn haiku(input: u64, w5: u64, read: u64, output: u64) -> Call {
        Call { model: "claude-haiku-5-5".into(), purpose: "chat".into(), input, cache_write_5m: w5, cache_read: read, output, ..Default::default() }
    }

    #[test]
    fn haiku_prices_by_prompt_length() {
        // 50k prompt: 10k input, 10k written, 30k read, 2k output at the short-prompt prices.
        let short = haiku(10_000, 10_000, 30_000, 2_000);
        let expected = (10_000.0 * 0.10 + 10_000.0 * 0.125 + 30_000.0 * 0.01 + 2_000.0 * 0.50) / 1e6;
        assert!((short.usd().unwrap() - expected).abs() < 1e-12);
        assert!(!short.long_prompt());
        // The same counts over 100k: the whole request at the long-prompt prices (cache reads count to the length).
        let long = haiku(10_000, 10_000, 90_000, 2_000);
        assert!(long.long_prompt());
        let expected = (10_000.0 * 0.50 + 10_000.0 * 0.625 + 90_000.0 * 0.05 + 2_000.0 * 2.50) / 1e6;
        assert!((long.usd().unwrap() - expected).abs() < 1e-12);
        // Exactly 100k is still the short price.
        assert!(!haiku(100_000, 0, 0, 0).long_prompt());
    }

    #[test]
    fn usage_with_and_without_the_cache_split() {
        let u = json!({"input_tokens": 12, "cache_creation_input_tokens": 300, "cache_read_input_tokens": 5000, "output_tokens": 40,
                       "cache_creation": {"ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 200}});
        let c = Call::from_usage("claude-haiku-5-5", "chat", &u, 1, 900);
        assert_eq!((c.input, c.cache_write_5m, c.cache_write_1h, c.cache_read, c.output, c.retries), (12, 100, 200, 5000, 40, 1));
        assert_eq!(c.prompt(), 5312);
        let old = Call::from_usage("claude-haiku-5-5", "chat", &json!({"input_tokens": 1, "cache_creation_input_tokens": 300, "output_tokens": 2}), 0, 0);
        assert_eq!((old.cache_write_5m, old.cache_write_1h), (300, 0));
    }

    #[test]
    fn totals_and_unknown_models() {
        let mut t = Totals::default();
        t.add(&haiku(1_000, 0, 9_000, 100));
        t.add(&Call { model: "gpt-5-mini".into(), input: 10, output: 10, ..Default::default() });
        assert_eq!((t.calls, t.cache_read), (2, 9_000));
        assert!(t.unpriced && t.usd > 0.0);
        assert!((t.cache_share() - 9_000.0 / 10_010.0).abs() < 1e-9);
        assert!(prices("claude-opus-5-5", 0).is_some() && prices("anthropic.claude-sonnet-5-5", 0).is_some() && prices("gpt-5", 0).is_none());
    }
}
