//! Integration-style tests for the Synology Chat plugin pure logic.
//!
//! The WASM component shim (`lib.rs` under `target_family = "wasm"`) is *not*
//! compiled here — we exercise the host-testable `synology_chat` module only.

use synology_chat::synology_chat::{
    build_outgoing_form_body, build_outgoing_payload, parse_incoming_form, split_message,
    SynologyChatConfig,
};

#[test]
fn round_trip_inbound_parse_and_outgoing_url() {
    let cfg = SynologyChatConfig::from_json(
        r#"{"bot_token":"abc123","synology_base_url":"https://nas.local:5001","default_user_id":6}"#,
    );
    // Inbound user message.
    let body = b"token=abc123&text=ping+%E6%B5%8B%E8%AF%95&username=tester&user_id=42";
    let inbound = parse_incoming_form(body, Some(&cfg.bot_token))
        .ok()
        .flatten()
        .expect("valid form");
    assert_eq!(inbound.sender, "tester");
    assert_eq!(inbound.reply_target, "42");
    assert_eq!(inbound.content, "ping 测试");
    assert_eq!(inbound.user_id, Some(42));

    // The reply goes back to user 42 (explicit inbound id > default).
    let uid = cfg.resolve_user_id(&inbound.reply_target);
    assert_eq!(uid, Some(42));

    let url = cfg.outgoing_url().expect("outgoing url");
    assert!(url.starts_with("https://nas.local:5001/webapi/entry.cgi?"));
    assert!(url.contains("token=%22abc123%22"));

    // Outgoing body shape: payload=<percent-encoded JSON>
    let payload = build_outgoing_payload("pong 测试", uid);
    let body = build_outgoing_form_body(&payload);
    assert!(body.starts_with("payload="));
    let json_part = &body["payload=".len()..];
    // Round-trip decode: percent-decode → parse JSON.
    let decoded: String = percent_decode_ref(json_part);
    let val: serde_json::Value = serde_json::from_str(&decoded).unwrap();
    assert_eq!(val["text"], "pong 测试");
    assert_eq!(val["user_ids"], serde_json::json!([42]));
}

#[test]
fn long_reply_is_segmented_and_each_segment_respects_max_length() {
    let mut lines = Vec::new();
    for i in 0..20 {
        lines.push(format!("line-{i}: {}", "x".repeat(80)));
    }
    let text = lines.join("\n");
    let segs = split_message(&text, 200);
    assert!(segs.len() > 1, "expected multi-segment split, got {segs:?}");
    for seg in &segs {
        assert!(
            seg.len() <= 200,
            "segment of length {} exceeds 200: {seg:?}",
            seg.len()
        );
    }
    // Reconstructed content must match (per-segment trimming of trailing
    // newlines means joining with '\n' yields the same logical text).
    let rejoined = segs.join("\n");
    assert_eq!(rejoined, text);
}

#[test]
fn token_mismatch_is_rejected() {
    let body = b"token=WRONG&text=hi&username=u";
    let err = parse_incoming_form(body, Some("RIGHT")).unwrap_err();
    assert!(err.contains("token mismatch"));
}

#[test]
fn empty_content_is_noop_not_error() {
    let body = b"token=TOK&text=&username=u";
    assert!(parse_incoming_form(body, Some("TOK")).unwrap().is_none());
    assert!(parse_incoming_form(b"", Some("TOK")).unwrap().is_none());
}

// Minimal percent-decode helper used only inside tests (mirrors the module's
// private `percent_decode` so we can round-trip outgoing bodies).
fn percent_decode_ref(input: &str) -> String {
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
