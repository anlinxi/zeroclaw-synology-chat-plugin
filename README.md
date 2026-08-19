# ZeroClaw Synology Chat Plugin

> 🌏 **中文文档**：[`README.zh.md`](README.zh.md)（请点这里阅读中文版部署 + 配置说明）

A [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) channel plugin that integrates with **Synology Chat** bots, enabling bidirectional messaging between users in Synology Chat and ZeroClaw-powered AI agents.

---

## Download the Plugin Binary

Pre-built `synology_chat.wasm` is published via GitHub Releases — no local Rust toolchain required.

👉 **Direct download link for the latest release:**
```
https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm
```

Or grab it from the release page:
https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/tag/v0.1.0

Asset information for `v0.1.0`:

| Field | Value |
|-------|-------|
| File | `synology_chat.wasm` |
| Size | 349,893 bytes (~342 KB) |
| Target | `wasm32-wasip2` (WASI Preview 2 / component model) |
| SHA reference | 342 KB release asset attached to tag `v0.1.0` |

---

## 1. Deployment — Where to Put the `.wasm` File

### 1.1 Required Directory Layout

ZeroClaw discovers plugins by scanning the directory configured in `[plugins] plugins_dir` (default **`~/.zeroclaw/plugins/`**) for subdirectories that contain both a `manifest.toml` and the wasm binary named in the manifest's `wasm_path`. Your installed plugin must look exactly like this:

```
~/.zeroclaw/                              ← ZeroClaw config root ("config_dir")
├── zeroclaw.toml                         ← Main config file (you edit this)
└── plugins/                              ← [plugins] plugins_dir (the default)
    └── synology-chat/                    ← One subdirectory per plugin
        ├── manifest.toml                 ← Copy from this git repository (required)
        └── synology_chat.wasm            ← Release binary you downloaded
                                            (filename MUST match wasm_path in manifest:
                                             `synology_chat.wasm` with UNDERSCORE, not dash)
```

### 1.2 One-shot Installation Script

Run the following on the machine that hosts ZeroClaw:

```bash
# (1) Create the plugin directory
mkdir -p ~/.zeroclaw/plugins/synology-chat

# (2) Grab the release binary
curl -L -o ~/.zeroclaw/plugins/synology-chat/synology_chat.wasm \
  https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm

# (3) Grab the manifest from the repo
curl -L -o ~/.zeroclaw/plugins/synology-chat/manifest.toml \
  https://raw.githubusercontent.com/anlinxi/zeroclaw-synology-chat-plugin/main/manifest.toml

# (4) Verify
ls -la ~/.zeroclaw/plugins/synology-chat/
# Expected output:
#   manifest.toml
#   synology_chat.wasm   (~342KB)
```

> **Note for binary build users.** The default prebuilt installer ships ZeroClaw without the WASM plugin backend. To load this plugin you need a host binary compiled with the `plugins-wasm-cranelift` feature:
> ```bash
> cd zeroclaw
> cargo build --release --features plugins-wasm-cranelift
> ```

---

## 2. ZeroClaw Configuration — `zeroclaw.toml`

The main config file lives at **`~/.zeroclaw/zeroclaw.toml`**. Two independent sections must be present:

1. `[plugins]` → turns on the plugin loader
2. `[channels.synology-chat.<alias>]` → configures this specific channel plugin

### 2.1 Full Working Example

```toml
# ======================================================================
# SECTION 1: Enable the WASM plugin system (OFF by default!)
# ======================================================================
[plugins]
enabled = true                         # Master switch — must be true.
auto_discover = true                   # Auto-scan plugins_dir on startup.
plugins_dir = "~/.zeroclaw/plugins"    # Default; can be omitted if unchanged.
max_plugins = 50

# Signature policy. For a local plugin you compiled / downloaded yourself,
# leave this at "disabled". Use "strict" only if you start signing manifests.
[plugins.security]
signature_mode = "disabled"            # disabled | permissive | strict

# ======================================================================
# SECTION 2: Synology Chat channel configuration
#
# Section name format:
#   [channels.<provides>.<alias>]
#   <provides> = manifest.toml  -> provides = "synology-chat"
#   <alias>    = your free-form instance name ("default" for single NAS setups)
# ======================================================================
[channels.synology-chat.default]
enabled = true

# ── Required ───────────────────────────────────────────────────────
# Synology Chat bot token (copy from DSM → Chat → Integration → Bot page).
# Used for TWO things:  (a) verify inbound token on the webhook,
#                       (b) build the outbound SYNO.Chat.External URL.
bot_token = "YOUR_SYNOLOGY_CHAT_BOT_TOKEN"

# Base URL of your Synology NAS: scheme + host + port, NO trailing slash.
# If ZeroClaw runs ON the NAS itself you can write http://127.0.0.1:5000
synology_base_url = "https://your-nas.example.com:5001"

# ── Optional (documented defaults shown) ───────────────────────────
# Fallback numeric user id for replies. Only used when the incoming
# message has neither `user_id` nor `channel_id`. Set to 0 to omit the
# `user_ids` array completely in that case.
default_user_id = 0

# Maximum UTF-8 characters per segment. Longer replies are split in a
# line-aware manner; only individual oversized lines are hard-split.
max_segment_length = 500

# Minimum gap between consecutive segments, in milliseconds. Throttling
# avoids triggering Synology's internal rate limiter.
min_send_interval_ms = 500
```

### 2.2 Channel Configuration Reference

| Field                    | Required | Default       | Description                                                                 |
|--------------------------|----------|---------------|-----------------------------------------------------------------------------|
| `enabled`                | No       | `false`       | Master switch for this channel alias.                                       |
| `bot_token`              | **Yes**  | —             | Synology Chat bot token (DSM Chat integration page).                       |
| `synology_base_url`      | **Yes**  | —             | NAS base URL. E.g. `https://nas.example.com:5001`.                          |
| `default_user_id`        | No       | `0`           | Fallback numeric user id for replies. Only used when inbound context has no numeric target. |
| `max_segment_length`     | No       | `500`         | Max UTF-8 chars per outgoing segment.                                       |
| `min_send_interval_ms`   | No       | `500`         | Min interval between segments, in ms. Guards Synology rate limits.         |

### 2.3 Multiple Instances (Channel Aliasing)

Use a different alias per NAS / bot. Each alias gets its own webhook path.

```toml
[channels.synology-chat.home]
enabled = true
bot_token = "HOME_BOT_TOKEN"
synology_base_url = "https://home-nas.local:5001"
default_user_id = 4

[channels.synology-chat.work]
enabled = true
bot_token = "WORK_BOT_TOKEN"
synology_base_url = "https://work-nas.company.com:5001"
default_user_id = 12
```

| Alias | Incoming webhook path on ZeroClaw |
|-------|-----------------------------------|
| `home` | `http(s)://<zc-host>:<port>/plugin/synology-chat-home` |
| `work` | `http(s)://<zc-host>:<port>/plugin/synology-chat-work` |
| `default` (special) | `http(s)://<zc-host>:<port>/plugin/synology-chat` (no `-default` suffix — matches built-in channel conventions) |

---

## 3. Synology DSM Side — Outgoing Webhook URL

In DSM Chat → Bot Settings → **Outgoing Webhook**, point the URL directly at ZeroClaw's plugin webhook route:

```
http(s)://<zeroclaw-host>:<zc-port>/plugin/synology-chat
```

Where:
- `<zeroclaw-host>` → hostname or IP of the machine running ZeroClaw (must be reachable from the NAS).
- `<zc-port>` → ZeroClaw's HTTP listen port (see `zeroclaw.toml`).
- If your alias is not `default`, append `-{alias}` e.g. `…/plugin/synology-chat-home`.

Make sure firewalls and any reverse proxy between the NAS and ZeroClaw allow this connection.

---

## 4. Post-Install Verification

```bash
# Toggle the master switches via CLI (or edit the toml, same result)
zeroclaw config set plugins.enabled true
zeroclaw config set plugins.auto_discover true

# Confirm the plugin is discovered
zeroclaw plugin list
# → you should see "synology-chat  v0.1.0  channel  http_client, config_read"

# Inspect the loaded manifest
zeroclaw plugin info synology-chat
```

If the plugin does **not** appear in `zeroclaw plugin list`, open the ZeroClaw startup log and look for `skip` / `warning` lines. The three most common reasons:

| Startup warning | Root cause | Fix |
|-----------------|------------|-----|
| *malformed manifest* | `manifest.toml` missing or has invalid TOML syntax. | Re-download `manifest.toml`. |
| *missing wasm_path file* | Directory has no `synology_chat.wasm` (note the underscore), or `manifest.toml` `wasm_path` does not match the real filename. | Verify both files exist side-by-side: `manifest.toml` + `synology_chat.wasm`. |
| *signature policy rejected* | `plugins.security.signature_mode = "strict"` and the manifest is unsigned. | Switch to `disabled` or `permissive` for your local install. |

---

## 5. Quick Integration Reference Card

| Item | Value / Path |
|---|---|
| Release binary (`synology_chat.wasm`) | https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm |
| Install directory | `~/.zeroclaw/plugins/synology-chat/{manifest.toml, synology_chat.wasm}` |
| Default plugins directory | `~/.zeroclaw/plugins` (`[plugins] plugins_dir`) |
| Plugin system switch | `[plugins] enabled = true` |
| Auto-discovery switch | `[plugins] auto_discover = true` |
| Signature policy recommendation (local) | `[plugins.security] signature_mode = "disabled"` |
| Channel config section | `[channels.synology-chat.default]` |
| Required channel keys | `bot_token`, `synology_base_url` |
| Inbound webhook path (alias=default) | `http(s)://<zc-host>:<port>/plugin/synology-chat` |

Once everything is in place, restart ZeroClaw, open a DM with your bot inside Synology Chat, and send it a message — the AI will reply in the same thread. 🚀

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
target/wasm32-wasip2/release/synology_chat.wasm
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
| `zeroclaw plugin list` shows nothing | Confirm `[plugins] enabled = true` AND `[plugins] auto_discover = true`; check startup log for `skip` warnings. |
| `plugins.security.signature_mode=strict` rejects the plugin | Switch to `disabled` for unsigned local plugins, or sign the manifest before publishing. |

---

## License

MIT OR Apache-2.0, same as the upstream `zeroclaw-plugins` repository.
