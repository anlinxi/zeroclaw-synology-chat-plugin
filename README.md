# ZeroClaw Synology Chat Plugin

A [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) channel plugin that integrates with **Synology Chat** bots, enabling bidirectional messaging between users in Synology Chat and ZeroClaw-powered AI agents.

---

## Features

- **Inbound Messages (Synology Chat → ZeroClaw)**
  - Receives user messages via the Synology Chat bot's outgoing webhook (`application/x-www-form-urlencoded`).
  - Token-based request verification to reject unauthorised calls.
  - Correctly decodes percent-encoded form fields, CJK text, and `+` → space conversion.
  - Preserves `user_id`, `channel_id`, and `username` for accurate reply targeting.

- **Outbound Replies (ZeroClaw → Synology Chat)**
  - Sends replies through the official `SYNO.Chat.External` Chatbot API (`/webapi/entry.cgi`).
  - Encodes the JSON payload as `payload=<urlencoded-json>` exactly as the Synology server expects (token is double-quoted and percent-encoded in the query string).
  - Optionally routes replies to a specific user via the `user_ids` array.
  - Validates the HTTP status **and** the `{"success":true}` flag inside the response body.

- **Robust Message Handling**
  - **Line-aware segmentation**: long replies are split by line first; only lines that individually exceed the limit are hard-split on character boundaries. Behaviour 1:1 with the reference Python middleware.
  - **Send throttling**: configurable minimum interval between consecutive segments to stay under Synology's internal rate limit.
  - Idempotent no-op when outbound is not yet configured (no crashes on partial config).

- **Developer Friendly**
  - All protocol logic lives in the pure-Rust `synology_chat` module and is host-tested (`cargo test` works without a browser/wasm runtime).
  - The wasm component shim (`lib.rs`) contains only IO and trait glue — easy to audit and extend.
  - 17 built-in unit + integration tests cover config, token verification, form parsing, splitting, and round-trip body encoding.

---

## Configuration

Add the following section to your ZeroClaw configuration file (e.g. `zeroclaw.toml`):

```toml
[channels.synology-chat.default]
enabled = true

# Bot token obtained when creating the Synology Chat bot.
# Used for BOTH inbound request verification AND outbound API URL construction.
bot_token = "YOUR_SYNOLOGY_CHAT_BOT_TOKEN"

# Base URL of your Synology NAS (protocol + host + port, NO trailing slash).
# The plugin appends `/webapi/entry.cgi?...` automatically.
synology_base_url = "https://your-nas.example.com:5001"

# (Optional) Fallback user id used when an inbound message carries no numeric
# user_id / channel_id. 0 = do not attach user_ids for such messages.
default_user_id = 0

# (Optional) Maximum characters per outgoing segment. Default: 500.
max_segment_length = 500

# (Optional) Minimum gap between two consecutive segments, in milliseconds.
# Default: 500.
min_send_interval_ms = 500
```

### Configuration Reference

| Field                    | Required | Default       | Description                                                                 |
|--------------------------|----------|---------------|-----------------------------------------------------------------------------|
| `enabled`                | No       | `false`       | Master switch for this channel.                                             |
| `bot_token`              | **Yes**  | —             | Synology Chat bot token (from the DSM Chat integration page).              |
| `synology_base_url`      | **Yes**  | —             | NAS base URL, e.g. `https://nas.example.com:5001`.                         |
| `default_user_id`        | No       | `0`           | Fallback numeric user id for replies. Only used when the inbound context has no numeric target. |
| `max_segment_length`     | No       | `500`         | Maximum UTF-8 character length per segment. Longer content is split.       |
| `min_send_interval_ms`   | No       | `500`         | Minimum interval between segments, in ms. Protects Synology rate limits.   |

### Channel Aliasing

You can create multiple independent Synology Chat channels by using aliases:

```toml
[channels.synology-chat.work]
enabled = true
bot_token = "WORK_BOT_TOKEN"
synology_base_url = "https://work-nas.local:5001"
default_user_id = 12

[channels.synology-chat.home]
enabled = true
bot_token = "HOME_BOT_TOKEN"
synology_base_url = "https://home-nas.local:5001"
default_user_id = 4
```

The webhook paths become `/plugin/synology-chat-work` and `/plugin/synology-chat-home` respectively.

---

## Usage Workflow

### 1. Install Synology Chat on DSM
Open **Package Center** on your Synology NAS, search for **Synology Chat**, and install it.

### 2. Create a Chat Bot
1. Open **Synology Chat** and navigate to **Integration / Bots** (integrations page).
2. Click **Create** → choose **Bot**.
3. Fill in the bot's name, avatar, and description.
4. Copy the generated **bot token** — you will need it for the `bot_token` config key.

### 3. Configure the Outgoing Webhook
In the same bot settings page, locate the **Outgoing Webhook** (传出 Webhook) section and set the target URL to:

```
http(s)://<zeroclaw-host>:<port>/plugin/synology-chat
```

- `<zeroclaw-host>` = hostname or IP of the machine running ZeroClaw.
- `<port>` = ZeroClaw's HTTP listen port (see `zeroclaw.toml`).
- If you use a channel alias, append `-{alias}` to the path, e.g. `/plugin/synology-chat-work`.

> Ensure your NAS can reach this URL (firewall / reverse proxy rules).

### 4. Update ZeroClaw Config
Paste the `[channels.synology-chat.*]` section (shown above) into `zeroclaw.toml`, replacing `bot_token` and `synology_base_url` with your values.

### 5. Restart & Test
Restart ZeroClaw. Open a Synology Chat direct message with your bot and send a message. The AI should reply shortly. Check the ZeroClaw logs if you do not see a reply.

---

## Protocol Details (for reference)

### Inbound — Synology → Plugin
Synology Chat sends `POST` requests with `Content-Type: application/x-www-form-urlencoded`:

| Form Field   | Presence   | Description                                            |
|--------------|------------|--------------------------------------------------------|
| `token`      | Always     | The bot token — verified against `bot_token`.          |
| `text`       | Always     | User message text (may be percent-encoded).            |
| `username`   | Always     | Display name of the sender.                            |
| `user_id`    | Optional   | Numeric user id used for reply-target routing.         |
| `channel_id` | Optional   | Numeric room/channel id — used when `user_id` absent.  |

- Empty `text` → acknowledged as `200 OK` but not forwarded to the agent.
- Token mismatch → host returns `403 Forbidden`.

### Outbound — Plugin → Synology
The plugin POSTs to:

```
<synology_base_url>/webapi/entry.cgi
    ?api=SYNO.Chat.External
    &method=chatbot
    &version=2
    &token=%22<bot_token>%22
```

Body (Content-Type: `application/x-www-form-urlencoded`):

```
payload=<percent-encoded JSON>
```

where the JSON payload shape is:

```json
{
  "text": "reply text segment",
  "user_ids": [ <numeric user id> ]   // omitted when target is unknown
}
```

Success is confirmed by `HTTP 2xx` **and** a response body containing `"success":true`.

---

## Development

### Project Layout

```
synology-chat-plugin/
├── manifest.toml             # ZeroClaw plugin manifest (permissions, capabilities)
├── Cargo.toml                # Rust crate config
├── README.md                 # English docs (this file)
├── README.zh.md              # 中文文档
├── .gitignore
├── src/
│   ├── lib.rs                # WASM component shim — Channel trait impl, HTTP IO
│   └── synology_chat.rs      # Pure protocol logic — host testable + unit tests
└── tests/
    └── synology_chat.rs      # Integration tests (round-trip, split, rejection)
```

### Prerequisites

* Rust stable toolchain (≥ 1.80 recommended)
* `wasm32-wasip2` target for building the plugin component
* Optional: [cargo-component](https://github.com/bytecodealliance/cargo-component) for advanced component signing

```bash
rustup target add wasm32-wasip2
```

### Build

Place this directory under `zeroclaw-plugins/plugins/synology-chat/` (because the WIT path in `lib.rs` points to `../../wit/unstable`), then run:

```bash
cd zeroclaw-plugins/plugins/synology-chat
cargo build --target wasm32-wasip2 --release
```

The compiled artifact is:

```
target/wasm32-wasip2/release/synology-chat.wasm
```

### Test

The logic crate is fully host-testable — no wasm runtime required:

```bash
cd synology-chat-plugin
cargo test
```

Expected output summary:

```
running 13 tests  (unit)
test result: ok. 13 passed; 0 failed

running 4 tests   (integration)
test result: ok. 4 passed; 0 failed
```

### Local Verification Tips

1. **Simulate an inbound webhook with curl**:
   ```bash
   curl -X POST http://localhost:<zc-port>/plugin/synology-chat \
        -d "token=YOUR_TOKEN&text=hello&username=tester&user_id=42"
   ```
   If the token matches, `200 OK` + the message appears in ZeroClaw's active chat.

2. **Simulate the Synology API side** with `nc -l -p 5001` or a local httpbin to verify the `payload=` body shape if you want to debug the encoding before pointing to the real NAS.

---

## Troubleshooting

| Symptom | Likely Cause / Fix |
|---------|---------------------|
| No reply in Chat | 1. Check `synology_base_url` is reachable from the ZeroClaw host. 2. Check the bot token matches (case-sensitive). 3. Look at ZeroClaw logs for `synology-chat:` errors. |
| HTTP 403 for inbound | Incoming token does not match `bot_token`. Verify no trailing whitespace. |
| Replies truncated to one segment | Increase `max_segment_length` (Synology Chat's own limit is higher; 500 is a safe default). |
| Rate-limit errors in logs | Increase `min_send_interval_ms` to 750 or 1000. |
| Plugin fails to load | Ensure the `.wasm` was built with `--target wasm32-wasip2 --release` and copied to the plugins directory registered in `zeroclaw.toml`. |

---

## License

MIT OR Apache-2.0, same as the upstream `zeroclaw-plugins` repository.
