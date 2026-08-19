# ZeroClaw Synology Chat 插件

> 🌏 **English docs:** [`README.md`](README.md) (click here for the English version)

[ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) 通道插件，用于对接 **Synology Chat（群晖 Chat）** 机器人，实现群晖 Chat 用户与 ZeroClaw AI 智能体之间的双向消息互通。

---

## 下载插件二进制文件

预编译的 `synology_chat.wasm` 已通过 GitHub Releases 发布——**本地不需要装 Rust 工具链**，直接下载即可。

👉 **最新 Release 直链：**
```
https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm
```

或从 Release 页面下载：
https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/tag/v0.1.0

`v0.1.0` 资产信息：

| 项目 | 值 |
|------|----|
| 文件名 | `synology_chat.wasm` |
| 大小 | 349,893 字节（约 342 KB） |
| 编译目标 | `wasm32-wasip2`（WASI Preview 2 / component model）|
| 对应 Git Tag | `v0.1.0` |

---

## 1. 部署——把 `.wasm` 文件放到哪里

### 1.1 必需的目录结构

ZeroClaw 通过扫描 `[plugins] plugins_dir`（默认 **`~/.zeroclaw/plugins/`**）来发现插件。插件目录下必须同时存在 `manifest.toml` 和 manifest 中 `wasm_path` 指定的 wasm 二进制。安装后的插件结构**必须严格如下**：

```
~/.zeroclaw/                              ← ZeroClaw 配置根目录（"config_dir"）
├── zeroclaw.toml                         ← 主配置文件（你要改的那个）
└── plugins/                              ← [plugins] plugins_dir（默认值）
    └── synology-chat/                    ← 每个插件一个子目录（名字随意，推荐和 provides 一致）
        ├── manifest.toml                 ← 从本仓库复制（必需）
        └── synology_chat.wasm            ← 你从 Release 下载的 wasm
                                            （文件名必须和 manifest 里的 wasm_path 完全一致：
                                             `synology_chat.wasm` 是下划线，不是中划线！）
```

### 1.2 一键安装脚本

在运行 ZeroClaw 的机器上执行：

```bash
# (1) 创建插件目录
mkdir -p ~/.zeroclaw/plugins/synology-chat

# (2) 下载 Release 二进制文件
curl -L -o ~/.zeroclaw/plugins/synology-chat/synology_chat.wasm \
  https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm

# (3) 下载本仓库的 manifest.toml
curl -L -o ~/.zeroclaw/plugins/synology-chat/manifest.toml \
  https://raw.githubusercontent.com/anlinxi/zeroclaw-synology-chat-plugin/main/manifest.toml

# (4) 验证文件都齐了
ls -la ~/.zeroclaw/plugins/synology-chat/
# 预期输出：
#   manifest.toml
#   synology_chat.wasm   (~342KB)
```

> **用官方二进制安装包的用户请注意。** 默认发布的 ZeroClaw 二进制**不带 WASM 插件后端**。要加载这个插件你需要自己从源码编译时启用 `plugins-wasm-cranelift` feature：
> ```bash
> cd zeroclaw
> cargo build --release --features plugins-wasm-cranelift
> ```

---

## 2. ZeroClaw 配置——修改 `zeroclaw.toml`

ZeroClaw 主配置文件路径通常是 **`~/.zeroclaw/zeroclaw.toml`**。你需要在其中加入**两个独立的 section**：

1. `[plugins]` → 打开插件系统总开关
2. `[channels.synology-chat.<别名>]` → 给这个 Synology Chat 通道插件填参数

### 2.1 完整可用的配置样例

```toml
# ======================================================================
# 第 1 部分：启用 WASM 插件系统（默认是关闭的！必须改成 true）
# ======================================================================
[plugins]
enabled = true                         # 插件系统总开关。不打开的话所有插件都不加载。
auto_discover = true                   # 启动时自动扫描 plugins_dir 下面的插件子目录。
plugins_dir = "~/.zeroclaw/plugins"    # 默认就是这个，不改可以不写。
max_plugins = 50

# 签名策略。对于你自己编译 / 下载的本地插件，保持 disabled 就行。
# 当你开始对 manifest 做签名发布时才切到 strict。
[plugins.security]
signature_mode = "disabled"            # disabled | permissive | strict

# ======================================================================
# 第 2 部分：Synology Chat 通道插件配置
#
# section 名格式：
#   [channels.<provides>.<别名>]
#   <provides> = manifest.toml 里的 provides = "synology-chat"
#   <别名>     = 你自由起的实例名（只接一个 NAS 时写 default 即可）
# ======================================================================
[channels.synology-chat.default]
enabled = true

# ── 必填 ───────────────────────────────────────────────────────
# 群晖 Chat 机器人 token（在 DSM → Chat → 集成 / 机器人页面创建时获得）。
# 有两个用途：  (a) 校验入站 webhook 携带的 token 字段；
#             (b) 拼接出站 SYNO.Chat.External API 调用的 URL。
bot_token = "YOUR_SYNOLOGY_CHAT_BOT_TOKEN"

# 群晖 NAS 的基础地址：协议 + 主机 + 端口，尾部不要带 /。
# 如果 ZeroClaw 就跑在 NAS 本机上，可以直接写 http://127.0.0.1:5000
synology_base_url = "https://your-nas.example.com:5001"

# ── 可选（下面写的就是默认值）──────────────────────────────────
# 回退用户 ID。当入站消息既没带 user_id 也没带 channel_id 时，
# 回复时会用这个作为兜底。填 0 表示完全省略 user_ids 字段。
default_user_id = 0

# 单段最大 UTF-8 字符数。超过的内容会按行优先自动分段；
# 只有单条超长行时才会被硬性字符切分。
max_segment_length = 500

# 相邻两段消息之间的最小间隔（毫秒）。用于避免触发群晖内部限流。
min_send_interval_ms = 500
```

### 2.2 通道配置字段对照表

| 字段                     | 必填    | 默认值         | 说明 |
|--------------------------|---------|----------------|------|
| `enabled`                | 否      | `false`        | 本通道别名的开关。设为 false = 关闭这一个 NAS。 |
| `bot_token`              | **是**  | —              | 群晖 Chat 机器人 token（DSM Chat 集成页面）。 |
| `synology_base_url`      | **是**  | —              | NAS 基础地址，例如 `https://nas.example.com:5001`。 |
| `default_user_id`        | 否      | `0`            | 回复时的兜底数字 user_id；仅当入站上下文中没有任何数字目标时才使用。 |
| `max_segment_length`     | 否      | `500`          | 出站单段最大 UTF-8 字符数，超长会自动分段。 |
| `min_send_interval_ms`   | 否      | `500`          | 相邻分段之间的最小间隔（毫秒）。避开群晖限流。 |

### 2.3 多实例配置（通道别名）

多台 NAS / 多个机器人时，分别用不同的别名。每个别名拥有独立的 webhook 路径。

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

| 别名 | ZeroClaw 上对应的入站 webhook 路径 |
|------|-----------------------------------|
| `home` | `http(s)://<zc-host>:<port>/plugin/synology-chat-home` |
| `work` | `http(s)://<zc-host>:<port>/plugin/synology-chat-work` |
| `default`（特殊） | `http(s)://<zc-host>:<port>/plugin/synology-chat`（**不带 `-default` 后缀**——与 ZeroClaw 内置通道约定保持一致） |

---

## 3. 群晖 DSM 侧——传出 Webhook URL 怎么填

在 DSM Chat → 机器人设置 → **传出 Webhook** 页面，把目标 URL 直接指向 ZeroClaw 的插件 webhook 路由：

```
http(s)://<zeroclaw-host>:<zc-port>/plugin/synology-chat
```

其中：
- `<zeroclaw-host>` → 运行 ZeroClaw 的机器的主机名或 IP（必须能从 NAS 访问到）。
- `<zc-port>` → ZeroClaw 的 HTTP 监听端口（看 `zeroclaw.toml`）。
- 如果你的别名不是 `default`，要把 `-{别名}` 缀到路径最后，比如 `…/plugin/synology-chat-home`。

请确认 NAS 和 ZeroClaw 之间的防火墙、反向代理等放行这个连接。

---

## 4. 安装后验证

```bash
# 通过 CLI 打开总开关（也可以直接改 toml，效果一样）
zeroclaw config set plugins.enabled true
zeroclaw config set plugins.auto_discover true

# 确认插件被发现了
zeroclaw plugin list
# → 你应该看到 "synology-chat  v0.1.0  channel  http_client, config_read"

# 查看加载到的 manifest 详情
zeroclaw plugin info synology-chat
```

如果 `zeroclaw plugin list` **没有**出现 `synology-chat`，请打开 ZeroClaw 启动日志，查找包含 `skip` / `warning` 的行。最常见的三种原因：

| 启动日志中的警告 | 根因 | 修复 |
|-----------------|------|------|
| *malformed manifest* | `manifest.toml` 不存在，或 TOML 语法错误。 | 重新下载 `manifest.toml`。 |
| *missing wasm_path file* | 插件目录里找不到 `synology_chat.wasm`（注意是下划线！），或 manifest.toml 里 `wasm_path` 和实际文件名对不上。 | 确认目录下同时存在两个文件：`manifest.toml` 和 `synology_chat.wasm`。 |
| *signature policy rejected* | `plugins.security.signature_mode = "strict"` 且 manifest 没有签名。 | 本地使用时改为 `disabled` 或 `permissive`。 |

---

## 5. 集成速查表

| 项目 | 值 / 路径 |
|---|---|
| Release 二进制 `synology_chat.wasm` 直链 | https://github.com/anlinxi/zeroclaw-synology-chat-plugin/releases/download/v0.1.0/synology_chat.wasm |
| 安装目录 | `~/.zeroclaw/plugins/synology-chat/{manifest.toml, synology_chat.wasm}` |
| 默认插件目录 | `~/.zeroclaw/plugins`（即 `[plugins] plugins_dir`） |
| 插件系统总开关 | `[plugins] enabled = true` |
| 自动发现开关 | `[plugins] auto_discover = true` |
| 签名策略（本地推荐） | `[plugins.security] signature_mode = "disabled"` |
| 通道配置 section | `[channels.synology-chat.default]` |
| 通道必填字段 | `bot_token`、`synology_base_url` |
| 入站 webhook 路径（别名=default） | `http(s)://<zc-host>:<port>/plugin/synology-chat` |

配好后重启 ZeroClaw，在群晖 Chat 里给机器人发一条私信，AI 会在同一个会话里回复你。🚀

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
- 内置 13 个单元测试 + 4 个集成测试，覆盖配置解析、令牌验证、表单解析、消息分段和往返报文编码。

---

## 5 步使用流程

### 1. 在 DSM 安装群晖 Chat
登录 DSM → 打开 **套件中心** → 搜索 **Synology Chat** → 安装。

### 2. 创建 Chat 机器人
1. 打开 **Synology Chat** → **集成 / 机器人** 页面。
2. 点**新建** → 选择**机器人**。
3. 填写机器人名字、头像、描述。
4. 复制生成的**机器人 token**——对应 `bot_token` 配置项。

### 3. 配置传出 Webhook
在同一页面的**传出 Webhook**（Outgoing Webhook）区域，把目标地址改成：
```
http(s)://<zeroclaw-host>:<port>/plugin/synology-chat
```
- `<zeroclaw-host>` = 运行 ZeroClaw 的机器 IP 或域名。
- `<port>` = ZeroClaw HTTP 监听端口。
- 如果你用了非 `default` 的别名，请追加 `-{别名}`，例如 `/plugin/synology-chat-work`。

> 请确认 NAS 能访问到这个地址（防火墙 / 反向代理放行）。

### 4. 改 ZeroClaw 配置
把上面 `[channels.synology-chat.*]` 那段写进 `zeroclaw.toml`，把 `bot_token` 和 `synology_base_url` 换成你自己的值。

### 5. 重启并测试
重启 ZeroClaw，在 Synology Chat 里给机器人发一条私信，几秒内应该收到 AI 回复。如果没有，看 ZeroClaw 日志里的 `synology-chat:` 报错。

---

## 协议细节（供参考）

### 入站——群晖 → 插件
群晖 Chat 发出 `POST` 请求，`Content-Type: application/x-www-form-urlencoded`：

| 表单字段     | 是否必带 | 说明 |
|--------------|----------|------|
| `token`      | 始终     | 机器人 token——会和 `bot_token` 做精确比较。 |
| `text`       | 始终     | 用户消息正文（可能是百分号编码）。 |
| `username`   | 始终     | 发送者的显示昵称。 |
| `user_id`    | 可选     | 数字用户 ID，用于路由回复目标。 |
| `channel_id` | 可选     | 数字房间 / 频道 ID；`user_id` 缺失时使用。 |

- `text` 为空 → 会返回 `200 OK`，但不会转发给 AI。
- token 不匹配 → 主机返回 `403 Forbidden`。

### 出站——插件 → 群晖
插件 POST 到：

```
<synology_base_url>/webapi/entry.cgi
    ?api=SYNO.Chat.External
    &method=chatbot
    &version=2
    &token=%22<bot_token>%22
```

请求体（Content-Type: `application/x-www-form-urlencoded`）：

```
payload=<做过百分号编码的 JSON>
```

其中 JSON 结构是：

```json
{
  "text": "回复正文分段",
  "user_ids": [ <数字用户 ID> ]   // 目标未知时整个字段省略
}
```

成功判定要求同时满足：`HTTP 2xx` **并且** 响应体中包含 `"success":true`。

---

## 开发指南

### 项目结构

```
synology-chat-plugin/
├── manifest.toml             # ZeroClaw 插件清单（权限、能力声明）
├── Cargo.toml                # Rust crate 配置
├── README.md                 # 英文文档
├── README.zh.md              # 中文文档（本文件）
├── .gitignore
├── src/
│   ├── lib.rs                # WASM 组件胶水层（Channel trait 实现 + HTTP IO）
│   └── synology_chat.rs      # 纯协议逻辑（主机侧可测试 + 单元测试）
└── tests/
    └── synology_chat.rs      # 集成测试（往返、分段、拒绝等）
```

### 环境要求

* Rust stable 工具链（推荐 ≥ 1.80）
* `wasm32-wasip2` target（构建插件 component 必需）
* 可选：[cargo-component](https://github.com/bytecodealliance/cargo-component) 用于高级组件签名

```bash
rustup target add wasm32-wasip2
```

### 构建

由于 `lib.rs` 中的 WIT 路径引用 `../../wit/unstable`，必须把本项目目录放在 **`zeroclaw-plugins/plugins/synology-chat/`** 下，再运行：

```bash
cd zeroclaw-plugins/plugins/synology-chat
cargo build --target wasm32-wasip2 --release
```

编译产物位于：

```
target/wasm32-wasip2/release/synology_chat.wasm
```

### 测试

所有协议逻辑都可以在主机侧直接跑，不需要 wasm 运行时：

```bash
cd synology-chat-plugin
cargo test
```

预期输出：

```
running 13 tests  (unit)
test result: ok. 13 passed; 0 failed

running 4 tests   (integration)
test result: ok. 4 passed; 0 failed
```

### 本地联调小技巧

1. **用 curl 模拟入站 webhook：**
   ```bash
   curl -X POST http://localhost:<zc-port>/plugin/synology-chat \
        -d "token=YOUR_TOKEN&text=你好&username=tester&user_id=42"
   ```
   token 对得上就会返回 `200 OK`，并且这条消息会出现在 ZeroClaw 活动会话里。

2. **模拟群晖 API 端**：在正式指向 NAS 之前，可以用 `nc -l -p 5001` 或本地 httpbin 先验证 `payload=` 报文体的格式。

---

## 常见问题排查

| 现象 | 可能原因 / 修复 |
|------|----------------|
| Chat 里看不到 AI 回复 | 1. 检查 ZeroClaw 主机是否能访问到 `synology_base_url`。2. 检查机器人 token 是否一致（大小写敏感）。3. 查看 ZeroClaw 日志中 `synology-chat:` 打头的错误。 |
| 入站 webhook 返回 HTTP 403 | 收到的 token 与 `bot_token` 不匹配，检查是否有尾随空格。 |
| 回复总是被截断成一段 | 调大 `max_segment_length`（群晖 Chat 本身的限制更高；500 是一个保守默认值）。 |
| 日志中出现限流报错 | 把 `min_send_interval_ms` 调到 750 或 1000。 |
| 插件加载失败 | 确认 `.wasm` 是用 `--target wasm32-wasip2 --release` 编译的，并已拷贝到 `zeroclaw.toml` 声明的插件目录。 |
| `zeroclaw plugin list` 空的 | 确认 `[plugins] enabled = true` 与 `[plugins] auto_discover = true` 都开了；看启动日志中有无 `skip` 警告。 |
| `plugins.security.signature_mode=strict` 拒绝插件 | 未签名的本地插件请改用 `disabled`；或发布前对 manifest 签名。 |

---

## 开源协议

MIT OR Apache-2.0 双许可，与上游 `zeroclaw-plugins` 仓库一致。
