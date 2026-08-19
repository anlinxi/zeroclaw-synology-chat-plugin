# ZeroClaw Synology Chat 插件

[ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) 通道插件，用于对接 **Synology Chat（群晖 Chat）** 机器人，实现群晖 Chat 用户与 ZeroClaw AI 智能体之间的双向消息互通。

> 英文版请参见 [README.md](README.md)。

---

## 功能特性

### 入站消息（群晖 Chat → ZeroClaw）
- 通过群晖 Chat 机器人的「传出 Webhook」接收用户消息，格式为 `application/x-www-form-urlencoded`。
- **令牌校验**：验证请求中的 `token` 字段与配置的 `bot_token` 是否匹配，拒绝未授权调用（`403 Forbidden`）。
- 正确解码百分号编码的表单字段、CJK 中文字符和 `+` → 空格转换。
- 保留 `user_id`、`channel_id` 和 `username` 字段，用于准确路由回复目标。

### 出站回复（ZeroClaw → 群晖 Chat）
- 通过群晖官方 `SYNO.Chat.External` Chatbot API（`/webapi/entry.cgi`）发送回复。
- 完全复刻官方报文格式：JSON 以 `payload=<百分号编码>` 形式提交；URL 查询串中 `token="..."` 会被正确双引号包裹并做百分号编码。
- 支持通过 `user_ids` 数组定向发送给指定用户。
- 同时校验 HTTP 状态码 **和** 响应体中的 `{"success":true}` 标记。

### 健壮的消息处理
- **按行优先分段**：超长回复优先按换行切分，只有单行超过上限时才做硬性字符切分，与参考 Python 中转逻辑行为 1:1 对齐。
- **发送限流**：可配置相邻分段之间的最小发送间隔，避免触发群晖内部接口限流。
- 出站未配置时为幂等空操作（不会因为配置不完整而崩溃）。

### 开发者友好
- 所有协议逻辑集中在纯 Rust 模块 `synology_chat` 中，可直接在主机侧测试（`cargo test` 无需 wasm 运行时）。
- WASM 组件入口（`lib.rs`）仅包含 IO 与 trait 胶水代码，易于审计与二次扩展。
- 内置 **17 个单元 + 集成测试**，覆盖：配置解析、令牌验证、表单解码、消息分段、完整请求/响应体往返编码。

---

## 配置说明

在 ZeroClaw 配置文件（例如 `zeroclaw.toml`）中添加以下章节：

```toml
[channels.synology-chat.default]
enabled = true

# 在群晖 Chat 中创建机器人时获取的 token。
# 同时用于：① 入站请求的令牌验证；② 出站 API URL 的自动拼接。
bot_token = "YOUR_SYNOLOGY_CHAT_BOT_TOKEN"

# 群晖 NAS 的基础地址（协议 + 主机 + 端口，末尾不要带 /）。
# 插件会自动拼接 /webapi/entry.cgi?...
synology_base_url = "https://your-nas.example.com:5001"

# （可选）当入站消息没有携带数字 user_id / channel_id 时，
# 作为回退的接收用户 ID。设为 0 表示不附加 user_ids 字段。
default_user_id = 0

# （可选）单段消息最大字符数。默认 500。
max_segment_length = 500

# （可选）两段消息之间最小发送间隔（毫秒）。默认 500。
min_send_interval_ms = 500
```

### 配置参数对照表

| 参数名                   | 必填 | 默认值 | 说明                                                                 |
|--------------------------|------|--------|----------------------------------------------------------------------|
| `enabled`                | 否   | false  | 该通道的总开关。                                                     |
| `bot_token`              | **是** | —    | 群晖 Chat 机器人 Token（DSM Chat 集成页面生成）。                    |
| `synology_base_url`      | **是** | —    | NAS 基础地址，例如 `https://nas.example.com:5001`。                  |
| `default_user_id`        | 否   | 0      | 回退接收用户 ID，仅在入站上下文缺少数字目标时生效。                 |
| `max_segment_length`     | 否   | 500    | 单段最大 UTF-8 字符数，超长内容自动分段。                            |
| `min_send_interval_ms`   | 否   | 500    | 分段发送最小间隔（毫秒），保护群晖接口限流。                         |

### 多实例（通道别名）

可通过别名创建多个独立的 Synology Chat 通道：

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

对应的 webhook 路径分别为：
- `/plugin/synology-chat-work`
- `/plugin/synology-chat-home`

---

## 使用流程

### 步骤 1：在 DSM 安装 Synology Chat
登录群晖 DSM，打开 **套件中心**，搜索 **Synology Chat** 并安装。

### 步骤 2：创建 Chat 机器人
1. 打开 **Synology Chat**，进入 **集成 / 机器人** 页面。
2. 点击 **新建** → 选择 **机器人**。
3. 填写机器人的名称、头像、描述。
4. 复制生成的 **机器人 Token**，这是配置中 `bot_token` 的值。

### 步骤 3：配置传出 Webhook
在同一机器人设置页中，找到 **传出 Webhook（Outgoing Webhook）** 部分，把目标地址设置为：

```
http(s)://<zeroclaw-host>:<port>/plugin/synology-chat
```

- `<zeroclaw-host>`：运行 ZeroClaw 机器的主机名或 IP。
- `<port>`：ZeroClaw 的 HTTP 监听端口（参考 `zeroclaw.toml`）。
- 如果使用通道别名，路径末尾追加 `-{alias}`，例如 `/plugin/synology-chat-work`。

> 请确保 NAS 可以访问到该 URL（防火墙 / 反代规则放行）。

### 步骤 4：修改 ZeroClaw 配置
将上面的 `[channels.synology-chat.*]` 片段写入 `zeroclaw.toml`，替换 `bot_token` 和 `synology_base_url` 为你自己的实际值。

### 步骤 5：重启并测试
重启 ZeroClaw。在 Synology Chat 中打开与该机器人的对话框，发送一条消息，AI 应当在数秒内回复。如无响应，请查看 ZeroClaw 日志中 `synology-chat:` 开头的错误信息。

---

## 协议细节（供参考）

### 入站：群晖 → 插件
Synology Chat 以 `POST` 方式提交，`Content-Type: application/x-www-form-urlencoded`，包含以下表单字段：

| 表单字段     | 出现情况 | 说明                                                  |
|--------------|----------|-------------------------------------------------------|
| `token`      | 必有     | 机器人 Token，与 `bot_token` 比对做校验。              |
| `text`       | 必有     | 用户发送的消息内容（可能被百分号编码）。              |
| `username`   | 必有     | 发送者显示名。                                        |
| `user_id`    | 可选     | 数字用户 ID，用于路由回复目标。                       |
| `channel_id` | 可选     | 数字房间/频道 ID，当 `user_id` 不存在时作为回退。     |

- 空 `text` → 返回 `200 OK` 但不会转发给 AI 智能体。
- Token 不匹配 → 主机返回 `403 Forbidden`。

### 出站：插件 → 群晖
插件向如下 URL 发起 POST：

```
<synology_base_url>/webapi/entry.cgi
    ?api=SYNO.Chat.External
    &method=chatbot
    &version=2
    &token=%22<bot_token>%22
```

请求体（Content-Type: `application/x-www-form-urlencoded`）：

```
payload=<百分号编码的 JSON>
```

其中 JSON 结构为：

```json
{
  "text": "回复的文本片段",
  "user_ids": [ <用户数字 ID> ]   // 当目标未知时省略该字段
}
```

成功条件：返回 **HTTP 2xx** 且响应体中包含 `"success":true`。

---

## 开发指南

### 项目结构

```
synology-chat-plugin/
├── manifest.toml             # ZeroClaw 插件清单（权限、能力声明等）
├── Cargo.toml                # Rust 工程配置
├── README.md                 # 英文文档
├── README.zh.md              # 中文文档（本文件）
├── .gitignore
├── src/
│   ├── lib.rs                # WASM 组件入口 — Channel trait 实现、HTTP IO
│   └── synology_chat.rs      # 纯协议逻辑 — 可主机侧测试 + 单元测试
└── tests/
    └── synology_chat.rs      # 集成测试（完整往返、分段、拒绝逻辑等）
```

### 环境要求

* Rust stable 工具链（推荐 ≥ 1.80）
* 构建插件组件需要 `wasm32-wasip2` 目标
* （可选）[cargo-component](https://github.com/bytecodealliance/cargo-component) 用于高级组件签名

```bash
rustup target add wasm32-wasip2
```

### 构建

由于 `lib.rs` 中 WIT 路径写的是相对路径 `../../wit/unstable`，请把整个目录放到 `zeroclaw-plugins/plugins/synology-chat/` 下，再执行：

```bash
cd zeroclaw-plugins/plugins/synology-chat
cargo build --target wasm32-wasip2 --release
```

构建产物位置：

```
target/wasm32-wasip2/release/synology-chat.wasm
```

### 测试

逻辑 crate 完全可以在主机侧测试，不需要 wasm 运行时：

```bash
cd synology-chat-plugin
cargo test
```

预期输出：

```
running 13 tests  (单元测试)
test result: ok. 13 passed; 0 failed

running 4 tests   (集成测试)
test result: ok. 4 passed; 0 failed
```

### 本地联调技巧

1. **用 curl 模拟入站 webhook**：
   ```bash
   curl -X POST http://localhost:<zc-port>/plugin/synology-chat \
        -d "token=YOUR_TOKEN&text=你好&username=测试用户&user_id=42"
   ```
   Token 匹配时应返回 `200 OK`，并且消息出现在 ZeroClaw 的对话中。

2. **模拟群晖 API 端**：可用 `nc -l -p 5001` 或本地 httpbin 等工具，先验证 `payload=` 报文体的形状再切到真实 NAS，便于调试编码细节。

---

## 常见问题排查

| 现象                         | 可能的原因 / 解决办法                                                                                         |
|------------------------------|--------------------------------------------------------------------------------------------------------------|
| Chat 中看不到 AI 回复        | ① 确认 ZeroClaw 主机能访问到 `synology_base_url`。② 确认 bot token 大小写完全一致。③ 查看 ZeroClaw 日志中 `synology-chat:` 报错。 |
| 入站返回 HTTP 403            | 入站请求的 token 与 `bot_token` 不匹配。检查是否有多余的尾部空格。                                           |
| 回复被截断为一段             | 调大 `max_segment_length`（Synology Chat 本身限制更大，500 是保守默认值）。                                   |
| 日志中出现限流相关错误       | 把 `min_send_interval_ms` 调到 750 或 1000。                                                                 |
| 插件加载失败                 | 确认 `.wasm` 是以 `--target wasm32-wasip2 --release` 构建的，并拷贝到了 `zeroclaw.toml` 中注册的插件目录。    |

---

## 开源协议

与上游 `zeroclaw-plugins` 仓库保持一致：**MIT OR Apache-2.0** 双许可。
