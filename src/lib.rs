//! A ZeroClaw WIT **channel** plugin for Synology Chat.
//!
//! *Inbound*: the host serves `POST` on `/plugin/synology-chat` and hands each
//! request to [`parse_webhook`]. The plugin verifies the `token` form field
//! against `bot_token` before decoding; a bad token returns
//! `Err(WebhookRejection::Unauthorized)` so the host replies `403`.
//!
//! *Outbound*: replies are sent (segmented + throttled) to the Synology Chat
//! External API (`/webapi/entry.cgi?...`) via the host's `wasi:http` (`waki`).
//! The body is `application/x-www-form-urlencoded` with a single
//! `payload=<urlencoded-json>` field matching what the original Python
//! middleware posted.
//!
//! Build:  rustup target add wasm32-wasip2
//!         cargo build --target wasm32-wasip2 --release

pub mod synology_chat;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/unstable",
        world: "channel-plugin",
        features: ["plugins-wit-v0"],
    });

    use std::cell::{Cell, RefCell};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::synology_chat::{
        build_outgoing_form_body, build_outgoing_payload, parse_incoming_form, split_message,
        Inbound, SynologyChatConfig, CHANNEL, WEBHOOK_PATH,
    };
    use exports::zeroclaw::plugin::channel::{
        ApprovalRequest, ApprovalResponse, ChannelCapabilities, Guest as Channel, InboundMessage,
        SendMessage, WebhookRejection,
    };
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;

    const PLUGIN_VERSION: &str = "0.1.0";
    const CONNECT_TIMEOUT_SECS: u64 = 10;

    thread_local! {
        static CONFIG: RefCell<SynologyChatConfig> = RefCell::new(SynologyChatConfig::default());
        /// Monotonic inbound counter for the message id (`synology_<seq>`).
        static SEQ: Cell<u64> = const { Cell::new(0) };
        /// Wall-clock timestamp (ms) of the last successfully dispatched
        /// segment — used for the per-channel `min_send_interval_ms` throttle.
        static LAST_SEND_MS: Cell<u64> = const { Cell::new(0) };
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn sleep_ms(ms: u64) {
        // Use waki's built-in blocking delay via a tiny HEAD-with-timeout trick
        // would be nice, but waki 0.5 doesn't expose `thread::sleep`. The host
        // mediates `poll_message` as our yield point; for very small sleeps
        // (≤500ms typical) a simple busy-spin on the clock is acceptable in
        // this single-threaded wasm component.
        let target = now_ms().saturating_add(ms);
        while now_ms() < target {
            // no std::thread available in wasi-preview2 without wasi-threads.
            // Spin gently — this branch is reached at most once per segment
            // and never for more than ~500ms.
        }
    }

    fn header_get(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }

    fn to_wit(inb: Inbound) -> InboundMessage {
        let seq = SEQ.with(|c| {
            let v = c.get();
            c.set(v.wrapping_add(1));
            v
        });
        InboundMessage {
            id: format!("{CHANNEL}_{seq}"),
            sender: inb.sender,
            reply_target: inb.reply_target,
            content: inb.content,
            channel: CHANNEL.to_string(),
            channel_alias: None,
            timestamp: now_ms() / 1000,
            thread_ts: None,
            interruption_scope_id: None,
            attachments: Vec::new(),
            subject: None,
        }
    }

    fn response_text(resp: waki::Response) -> String {
        resp.body()
            .ok()
            .and_then(|body| String::from_utf8(body).ok())
            .unwrap_or_default()
    }

    struct SynologyChatChannel;

    impl PluginInfo for SynologyChatChannel {
        fn plugin_name() -> String {
            CHANNEL.to_string()
        }
        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Channel for SynologyChatChannel {
        fn name() -> String {
            CHANNEL.to_string()
        }

        fn configure(config: String) -> Result<(), String> {
            let cfg = SynologyChatConfig::from_json(&config);
            CONFIG.with(|c| *c.borrow_mut() = cfg);
            LAST_SEND_MS.with(|c| c.set(0));
            SEQ.with(|c| c.set(0));
            Ok(())
        }

        fn send(message: SendMessage) -> Result<(), String> {
            if !message.attachments.is_empty() {
                return Err(
                    "synology-chat: media attachments are not supported yet".to_string(),
                );
            }
            let (cfg, url) = CONFIG.with(|c| {
                let cfg = c.borrow().clone();
                (cfg.clone(), cfg.outgoing_url())
            });
            let Some(url) = url else {
                // No outbound URL configured → drop silently.
                return Ok(());
            };

            let user_id = cfg.resolve_user_id(&message.recipient);
            let segments = split_message(&message.content, cfg.max_segment_length());
            let min_interval = cfg.min_send_interval_ms();

            for (i, segment) in segments.iter().enumerate() {
                let last = LAST_SEND_MS.with(Cell::get);
                let now = now_ms();
                let elapsed = now.saturating_sub(last);
                if last != 0 && elapsed < min_interval {
                    let wait = min_interval - elapsed;
                    sleep_ms(wait);
                }

                let payload = build_outgoing_payload(segment, user_id);
                let body = build_outgoing_form_body(&payload);

                let resp = waki::Client::new()
                    .post(&url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .header("Content-Length", &body.len().to_string())
                    .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS))
                    .body(body.as_bytes().to_vec())
                    .send()
                    .map_err(|e| {
                        format!(
                            "synology-chat: segment {}/{} send failed: {e}",
                            i + 1,
                            segments.len()
                        )
                    })?;
                LAST_SEND_MS.with(|c| c.set(now_ms()));

                let status = resp.status_code();
                let detail = response_text(resp);
                if !(200..300).contains(&status) {
                    return Err(format!(
                        "synology-chat: segment {}/{} HTTP {status}: {detail}",
                        i + 1,
                        segments.len()
                    ));
                }
                if !detail.is_empty() && !detail.contains("\"success\":true") {
                    // Synology returns `{"success":true, ...}` in the body on
                    // happy path; log-ish surface via Err only when the body
                    // explicitly flags failure.
                    if detail.contains("\"success\":false")
                        || detail.contains("\"error\"")
                    {
                        return Err(format!(
                            "synology-chat: segment {}/{} API error: {detail}",
                            i + 1,
                            segments.len()
                        ));
                    }
                }
            }
            Ok(())
        }

        /// A Synology Chat channel never polls — inbound arrives via `parse_webhook`.
        fn poll_message() -> Option<InboundMessage> {
            None
        }

        fn get_channel_capabilities() -> ChannelCapabilities {
            ChannelCapabilities::HEALTH_CHECK | ChannelCapabilities::WEBHOOK_INGRESS
        }

        fn health_check() -> bool {
            CONFIG.with(|c| c.borrow().is_configured())
        }

        fn webhook_path() -> Option<String> {
            Some(WEBHOOK_PATH.to_string())
        }

        fn parse_webhook(
            headers: Vec<(String, String)>,
            body: Vec<u8>,
        ) -> Result<Vec<InboundMessage>, WebhookRejection> {
            let method = header_get(&headers, "x-webhook-method").unwrap_or_default();
            // A GET is a no-op acknowledgement.
            if method.eq_ignore_ascii_case("GET") {
                return Ok(Vec::new());
            }

            let cfg = CONFIG.with(|c| c.borrow().clone());
            let token_for_verify = if cfg.bot_token.trim().is_empty() {
                None
            } else {
                Some(cfg.bot_token.trim())
            };
            let inbound = parse_incoming_form(&body, token_for_verify).map_err(|e| {
                if e.contains("token mismatch") {
                    WebhookRejection::Unauthorized(e)
                } else {
                    WebhookRejection::BadRequest(e)
                }
            })?;
            match inbound {
                Some(inb) => Ok(vec![to_wit(inb)]),
                None => Ok(Vec::new()),
            }
        }

        // ── capability-gated stubs (documented WIT defaults) ──
        fn self_handle() -> Option<String> {
            None
        }
        fn self_addressed_mention() -> Option<String> {
            None
        }
        fn drop_self_message(_msg: InboundMessage) -> bool {
            false
        }
        fn start_typing(_recipient: String) -> Result<(), String> {
            Ok(())
        }
        fn stop_typing(_recipient: String) -> Result<(), String> {
            Ok(())
        }
        fn supports_draft_updates() -> bool {
            false
        }
        fn send_draft(_message: SendMessage) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn update_draft(_r: String, _m: String, _t: String) -> Result<(), String> {
            Ok(())
        }
        fn update_draft_progress(_r: String, _m: String, _t: String) -> Result<(), String> {
            Ok(())
        }
        fn finalize_draft(_r: String, _m: String, _t: String) -> Result<(), String> {
            Ok(())
        }
        fn cancel_draft(_r: String, _m: String) -> Result<(), String> {
            Ok(())
        }
        fn supports_multi_message_streaming() -> bool {
            false
        }
        fn multi_message_delay_ms() -> u64 {
            800
        }
        fn add_reaction(_c: String, _m: String, _e: String) -> Result<(), String> {
            Ok(())
        }
        fn remove_reaction(_c: String, _m: String, _e: String) -> Result<(), String> {
            Ok(())
        }
        fn pin_message(_c: String, _m: String) -> Result<(), String> {
            Ok(())
        }
        fn unpin_message(_c: String, _m: String) -> Result<(), String> {
            Ok(())
        }
        fn redact_message(_c: String, _m: String, _reason: Option<String>) -> Result<(), String> {
            Ok(())
        }
        fn request_approval(
            _recipient: String,
            _request: ApprovalRequest,
        ) -> Result<Option<ApprovalResponse>, String> {
            Ok(None)
        }
        fn request_choice(
            _question: String,
            _choices: Vec<String>,
            _timeout_secs: u64,
        ) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn supports_free_form_ask() -> bool {
            true
        }
    }

    export!(SynologyChatChannel);
}
