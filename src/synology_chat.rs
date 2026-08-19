//! Pure Synology Chat channel logic — no wasm, no HTTP, no host deps.
//!
//! This is the `rlib` half of the plugin. It owns everything I/O-free and
//! therefore host-testable with a plain `cargo test`:
//!
//!   * parsing the plugin's `[channels.synology-chat.<alias>]` config section,
//!   * verifying the incoming `token` form field against the configured bot token,
//!   * decoding the Synology Chat outgoing-webhook form body into an inbound message,
//!   * splitting long replies into segments (line-aware, max length),
//!   * building the `payload=...` form body and the full Synology External API URL.

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{json, Value};

pub const CHANNEL: &str = "synology-chat";
/// The URL path segment the host mounts this channel's webhook under
/// (`/plugin/synology-chat`).
pub const WEBHOOK_PATH: &str = "synology-chat";
pub const DEFAULT_MAX_SEGMENT_LENGTH: usize = 500;
pub const DEFAULT_MIN_SEND_INTERVAL_MS: u64 = 500;

/// The plugin's config section, mirroring `[channels.synology-chat.<alias>]`
/// snake_case keys.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SynologyChatConfig {
    /// Host-side enable gate.
    #[serde(default)]
    pub enabled: bool,
    /// Synology Chat bot token — used both for inbound token verification
    /// and for building the outbound External API URL.
    #[serde(default)]
    pub bot_token: String,
    /// Synology NAS base URL, e.g. `https://nas.example.com:5001` (no trailing
    /// slash). The plugin appends the External API path + token-encoded query.
    #[serde(default)]
    pub synology_base_url: String,
    /// Default `user_ids[0]` when the inbound context does not carry a numeric
    /// user id. 0 (default) means "do not include user_ids unless the caller
    /// explicitly passes one".
    #[serde(default)]
    pub default_user_id: i64,
    /// Maximum characters per outgoing segment; longer replies are split
    /// line-by-line. Defaults to [`DEFAULT_MAX_SEGMENT_LENGTH`].
    #[serde(default)]
    pub max_segment_length: Option<usize>,
    /// Minimum interval between two consecutive segment sends, in milliseconds.
    /// Defaults to [`DEFAULT_MIN_SEND_INTERVAL_MS`].
    #[serde(default)]
    pub min_send_interval_ms: Option<u64>,
}

impl SynologyChatConfig {
    /// Parse the JSON config string the host hands to `configure`. An empty or
    /// malformed string yields defaults (inert rather than a hard failure).
    pub fn from_json(config_json: &str) -> Self {
        serde_json::from_str(config_json).unwrap_or_default()
    }

    pub fn is_configured(&self) -> bool {
        !self.bot_token.trim().is_empty() && !self.synology_base_url.trim().is_empty()
    }

    pub fn max_segment_length(&self) -> usize {
        self.max_segment_length
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_SEGMENT_LENGTH)
    }

    pub fn min_send_interval_ms(&self) -> u64 {
        self.min_send_interval_ms
            .unwrap_or(DEFAULT_MIN_SEND_INTERVAL_MS)
    }

    /// Resolve the numeric user id for an outbound reply: prefer the explicit
    /// reply_target when it parses as a positive integer, else fall back to
    /// `default_user_id`. Returns `None` when neither is a positive id.
    pub fn resolve_user_id(&self, reply_target: &str) -> Option<i64> {
        if let Ok(id) = reply_target.parse::<i64>() {
            if id > 0 {
                return Some(id);
            }
        }
        if self.default_user_id > 0 {
            Some(self.default_user_id)
        } else {
            None
        }
    }

    /// Build the full Synology External API URL for sending messages. The
    /// resulting URL looks like:
    /// ```text
    /// <base>/webapi/entry.cgi?api=SYNO.Chat.External&method=chatbot&version=2&token="%22...%22"
    /// ```
    /// (The token is double-quoted, then the quotes + token are percent-encoded
    /// so the server sees `token="the-token"`.)
    pub fn outgoing_url(&self) -> Option<String> {
        if !self.is_configured() {
            return None;
        }
        let base = self.synology_base_url.trim().trim_end_matches('/');
        // `token="bot_token"` on the query string: the quotes are part of the
        // value the Synology Chat server expects, so we percent-encode them.
        let token_value = format!("\"{}\"", self.bot_token.trim());
        let encoded_token = utf8_percent_encode(&token_value, NON_ALPHANUMERIC).to_string();
        Some(format!(
            "{base}/webapi/entry.cgi?api=SYNO.Chat.External&method=chatbot&version=2&token={encoded_token}"
        ))
    }
}

/// An inbound message after parsing the Synology Chat outgoing-webhook form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    pub sender: String,
    /// reply_target: the numeric `user_id` (as a string) from the incoming
    /// form so replies can go back to the same user. Falls back to the sender
    /// username if the form lacks a user_id.
    pub reply_target: String,
    pub content: String,
    pub user_id: Option<i64>,
}

/// Parse a Synology Chat outgoing-webhook form body (`x-www-form-urlencoded`).
/// Returns `Ok(None)` when this is a plain GET / acknowledgement, `Err` when
/// the token does not match or required fields are missing.
pub fn parse_incoming_form(
    body: &[u8],
    configured_token: Option<&str>,
) -> Result<Option<Inbound>, String> {
    if body.is_empty() {
        return Ok(None);
    }
    let body_str = std::str::from_utf8(body)
        .map_err(|_| "synology-chat: incoming body is not valid UTF-8".to_string())?;
    let parsed = form_urlencoded_decode(body_str);

    let form_token = parsed.get("token").cloned().unwrap_or_default();
    if let Some(expected) = configured_token {
        if !expected.trim().is_empty() && form_token.trim() != expected.trim() {
            return Err("synology-chat: token mismatch".to_string());
        }
    }

    let text = parsed.get("text").cloned().unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }

    let username = parsed
        .get("username")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "user".to_string());
    let user_id_str = parsed.get("user_id").cloned().filter(|s| !s.is_empty());
    let user_id = user_id_str.as_deref().and_then(|s| s.parse::<i64>().ok());
    let reply_target = user_id_str
        .clone()
        .or_else(|| parsed.get("channel_id").cloned().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| username.clone());

    Ok(Some(Inbound {
        sender: username,
        reply_target,
        content: text,
        user_id,
    }))
}

/// Minimal application/x-www-form-urlencoded decoder (handles the two fields
/// Synology actually sends: `token`, `text`, `username`, `user_id`, `channel_id`).
fn form_urlencoded_decode(input: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in input.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some(kv) => kv,
            None => (pair, ""),
        };
        let key = percent_decode(k);
        let value = percent_decode(v);
        out.insert(key, value);
    }
    out
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Split a long message into line-aware segments of at most `max_length`
/// characters each, preserving line breaks where possible. This matches the
/// original Python `split_message` behaviour.
pub fn split_message(text: &str, max_length: usize) -> Vec<String> {
    if max_length == 0 {
        return if text.is_empty() { Vec::new() } else { vec![text.to_string()] };
    }
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.split('\n') {
        // If adding the new line (+ '\n') would overflow, flush the current
        // segment first (unless it's already empty).
        if !current.is_empty() && current.len() + line.len() + 1 > max_length {
            segments.push(current.trim_end().to_string());
            current = String::new();
        }
        // If a single line exceeds max_length, we hard-split it char-by-char
        // (Synology only guarantees the max; lines can't overflow it).
        if line.len() > max_length {
            if !current.is_empty() {
                segments.push(current.trim_end().to_string());
                current = String::new();
            }
            let mut remaining = line;
            while !remaining.is_empty() {
                let split_at = remaining
                    .char_indices()
                    .nth(max_length)
                    .map(|(i, _)| i)
                    .unwrap_or(remaining.len());
                let (chunk, rest) = remaining.split_at(split_at);
                segments.push(chunk.to_string());
                remaining = rest;
            }
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    let tail = current.trim_end().to_string();
    if !tail.is_empty() {
        segments.push(tail);
    }
    segments
}

/// Build the outgoing payload JSON as Synology Chat External API expects:
/// ```json
/// {"text": "<segment>", "user_ids": [<user_id>]}
/// ```
/// `user_ids` is omitted when `user_id` is `None` (e.g. broadcast / channel-only).
pub fn build_outgoing_payload(text: &str, user_id: Option<i64>) -> Value {
    let mut obj = json!({ "text": text });
    if let Some(uid) = user_id {
        obj["user_ids"] = json!([uid]);
    }
    obj
}

/// Build the full `application/x-www-form-urlencoded` POST body:
/// ```text
/// payload=%7B%22text%22%3A...%7D
/// ```
/// The JSON payload is serialised with `ensure_ascii=false` so CJK text stays
/// compact (Synology accepts UTF-8).
pub fn build_outgoing_form_body(payload: &Value) -> String {
    let json_str = serde_json::to_string(payload).unwrap_or_default();
    format!(
        "payload={}",
        utf8_percent_encode(&json_str, NON_ALPHANUMERIC)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Config ────────────────────────────────────────────────────────────

    #[test]
    fn config_parses_all_fields_and_sensible_defaults() {
        let cfg = SynologyChatConfig::from_json(
            r#"{"enabled":true,"bot_token":" TOK ","synology_base_url":" https://x:5001/ ","default_user_id":6,"max_segment_length":300,"min_send_interval_ms":250}"#,
        );
        assert!(cfg.enabled);
        assert!(cfg.is_configured());
        assert_eq!(cfg.max_segment_length(), 300);
        assert_eq!(cfg.min_send_interval_ms(), 250);
        assert_eq!(cfg.resolve_user_id(""), Some(6));
        let url = cfg.outgoing_url().unwrap();
        assert!(url.starts_with("https://x:5001/webapi/entry.cgi?"));
        assert!(url.contains("token=%22TOK%22"));
    }

    #[test]
    fn config_missing_fields_makes_it_inert() {
        let empty = SynologyChatConfig::from_json("{}");
        assert!(!empty.is_configured());
        assert_eq!(empty.max_segment_length(), DEFAULT_MAX_SEGMENT_LENGTH);
        assert_eq!(empty.min_send_interval_ms(), DEFAULT_MIN_SEND_INTERVAL_MS);
        assert!(empty.outgoing_url().is_none());
        // bad JSON → defaults, not a panic
        let bad = SynologyChatConfig::from_json("not json");
        assert!(!bad.is_configured());
    }

    #[test]
    fn resolve_user_id_prefers_reply_target_over_default() {
        let cfg = SynologyChatConfig::from_json(r#"{"default_user_id":10,"bot_token":"t","synology_base_url":"http://x"}"#);
        assert_eq!(cfg.resolve_user_id("6"), Some(6));
        assert_eq!(cfg.resolve_user_id("not-a-number"), Some(10));
        assert_eq!(cfg.resolve_user_id("0"), Some(10));
        let cfg2 = SynologyChatConfig::from_json(r#"{"bot_token":"t","synology_base_url":"http://x"}"#);
        assert_eq!(cfg2.resolve_user_id("not-a-number"), None);
        assert_eq!(cfg2.resolve_user_id("42"), Some(42));
    }

    // ── Incoming form parsing ─────────────────────────────────────────────

    #[test]
    fn parse_incoming_form_happy_path_with_user_id() {
        let body = b"token=TOK&text=hello+world&username=alice&user_id=6&channel_id=123";
        let inbound = parse_incoming_form(body, Some("TOK"))
            .ok()
            .flatten()
            .unwrap();
        assert_eq!(inbound.sender, "alice");
        assert_eq!(inbound.reply_target, "6"); // user_id wins
        assert_eq!(inbound.content, "hello world");
        assert_eq!(inbound.user_id, Some(6));
    }

    #[test]
    fn parse_incoming_form_falls_back_to_username_when_no_ids() {
        let body = b"token=TOK&text=%E4%BD%A0%E5%A5%BD&username=bob";
        let inbound = parse_incoming_form(body, Some("TOK"))
            .ok()
            .flatten()
            .unwrap();
        assert_eq!(inbound.content, "你好");
        assert_eq!(inbound.reply_target, "bob");
    }

    #[test]
    fn parse_incoming_form_rejects_bad_token() {
        let body = b"token=WRONG&text=hi&username=u";
        let err = parse_incoming_form(body, Some("CORRECT")).unwrap_err();
        assert!(err.contains("token mismatch"));
    }

    #[test]
    fn parse_incoming_form_accepts_any_token_when_unconfigured() {
        let body = b"token=ANYTHING&text=hi&username=u";
        assert!(parse_incoming_form(body, None).unwrap().is_some());
    }

    #[test]
    fn parse_incoming_form_ignores_empty() {
        assert!(parse_incoming_form(b"", Some("t")).unwrap().is_none());
        assert!(parse_incoming_form(b"token=t&text=&username=u", Some("t"))
            .unwrap()
            .is_none());
    }

    // ── Message splitting ─────────────────────────────────────────────────

    #[test]
    fn split_message_short_and_empty() {
        assert_eq!(split_message("", 10), Vec::<String>::new());
        assert_eq!(split_message("hi", 10), vec!["hi".to_string()]);
    }

    #[test]
    fn split_message_respects_line_boundaries() {
        let text = "line one\nline two\nline three";
        let segs = split_message(text, 15);
        // "line one\nline two" = 18 chars > 15, so they must be separate or
        // flush-first, each individually <= 15.
        for seg in &segs {
            assert!(
                seg.len() <= 15,
                "segment '{}' has length {} > 15",
                seg,
                seg.len()
            );
        }
        // Rejoined text should be equivalent (allowing trimming of trailing
        // newlines per segment, i.e. line breaks preserved as '\n' between
        // content lines, not at segment edges).
        let rejoined = segs.join("\n");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn split_message_hard_splits_oversized_line() {
        let long = "a".repeat(120);
        let segs = split_message(&long, 50);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].len(), 50);
        assert_eq!(segs[1].len(), 50);
        assert_eq!(segs[2].len(), 20);
    }

    // ── Outgoing body building ────────────────────────────────────────────

    #[test]
    fn build_outgoing_payload_with_and_without_user() {
        let with = build_outgoing_payload("hi", Some(6));
        assert_eq!(with["text"], "hi");
        assert_eq!(with["user_ids"], json!([6]));

        let without = build_outgoing_payload("hi", None);
        assert_eq!(without["text"], "hi");
        assert!(without.get("user_ids").is_none());
    }

    #[test]
    fn build_outgoing_form_body_percent_encodes_payload() {
        let payload = json!({"text":"hello & world = ?","user_ids":[6]});
        let body = build_outgoing_form_body(&payload);
        assert!(body.starts_with("payload="));
        let encoded = body.trim_start_matches("payload=");
        // Must not contain raw JSON separators in the encoded half.
        assert!(!encoded.contains('{'));
        assert!(!encoded.contains('"'));
        assert!(!encoded.contains(' '));
        // Decoding the percent part yields valid JSON with the expected text.
        let decoded = percent_decode(encoded);
        let v: Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(v["text"], "hello & world = ?");
    }
}
