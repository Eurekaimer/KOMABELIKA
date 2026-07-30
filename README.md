# Komari Call

仓库：`KOMABELIKA` · crate / 命令：`komari-call`

一个生活在终端里的小鞠知花风格聊天伙伴。当前 `v0.1.0` 已提供流式聊天 TUI、DeepSeek 官方 API、离线 Mock Provider、结构化 Persona，以及 SQLite 会话保存和恢复。Codex、OpenCode Zen 与长期记忆仍在后续阶段，本文不会把它们写成已完成功能。

> 小鞠不需要替我工作。 <del>写代码、修 Linux、整理项目和管理日程，已经有太多 Agent 抢着做了。</del>
> 她只需要陪我聊天就可以了。

## 当前界面

<!-- TODO: 接入真实 Provider 后补充终端截图。此处不放置版权图片。 -->

```text
┌ Komari Call  会话：晚上好  Provider：mock  模型：komari-mock ┐
│ 对话                                                        │
│ 你                                                          │
│ 晚上好                                                      │
│ 小鞠                                                        │
│ 唔，我听见了……                                             │
├ 输入 ───────────────────────────────────────────────────────┤
│                                                             │
└ Enter 发送 · Esc 取消 · Ctrl+C 保存并退出 ──────────────────┘
```

## 安装与运行

当前支持 Linux。SQLite 由 `rusqlite` 的 `bundled` 功能构建，不要求系统预装 SQLite；从源码构建需要 Rust stable 和 C/C++ 链接器。

直接安装全局命令：

```bash
cargo install --git https://github.com/Eurekaimer/KOMABELIKA.git --locked
```

也可以 clone 后运行：

```bash
git clone https://github.com/Eurekaimer/KOMABELIKA.git
cd KOMABELIKA
cargo run --release
```

未配置 DeepSeek 时默认进入离线 Mock Provider。要使用 DeepSeek：

```bash
komari-call login deepseek
komari-call config --provider deepseek --model deepseek-v4-flash
komari-call doctor
komari-call
```

`login` 会隐藏终端输入并把 Key 存入 Linux 系统 Keyring，不写入 TOML。也可仅对当前命令传入 Key：

```bash
komari-call chat --provider deepseek --model deepseek-v4-flash --api-key "$DEEPSEEK_API_KEY"
```

查看 API 当前返回的模型：

```bash
komari-call models --provider deepseek
```

Nix 用户也可以直接运行仓库 flake：

```bash
nix run github:Eurekaimer/KOMABELIKA
```

数据默认位于操作系统的用户数据目录；开发时可覆盖配置和数据路径：

```bash
KOMARI_CALL_CONFIG="$PWD/config.toml" \
KOMARI_CALL_DATA_DIR="$PWD/.local-data" \
cargo run --release
```

### 键位

- `Enter`：发送
- `Shift+Enter`：换行（部分终端无法区分该组合键）
- `Esc`：取消生成；已有片段以 `interrupted` 保存，但不会进入后续模型上下文
- `Ctrl+N`：新建会话
- `Ctrl+L`：轮换已保存会话
- `Ctrl+P`：显示当前 Provider/模型切换提示；命令行切换已实现
- `Ctrl+O`：显示配置提示；当前通过 `komari-call config` 修改，配置 TUI 尚未实现
- `Ctrl+C`：安全保存并退出

## Provider

| Provider | 登录方式 | 是否可能免费 | 说明 | 当前状态 |
| --- | --- | ---: | --- | --- |
| DeepSeek | API Key | 否 | 官方兼容 API、SSE 流式输出、推理内容分离 | 已实现 |
| ChatGPT/Codex | OAuth 或本地 Bridge | 取决于订阅 | 使用账户可访问模型 | 计划中 |
| OpenCode Zen | OpenCode API Key | 是 | 免费模型通常限时 | 计划中 |
| Mock | 无 | 是 | 本地开发与离线体验 | 已实现 |

### DeepSeek 配置

凭据优先级是当前进程的 `--api-key`、系统 Keyring、`DEEPSEEK_API_KEY`、配置指定的环境变量。支持自定义 Base URL、超时、模型、输出上限及 thinking 参数；401、429、5xx 和超时会返回分类错误，错误文本及日志不会包含 Key。`Esc` 会取消正在读取的流。

### Codex 登录

计划提供浏览器 OAuth；无法可靠支持时使用用户已登录的 Codex CLI Bridge。当前版本没有 Codex 登录命令，不读取 Cookie 或现有凭据。

### OpenCode Zen 免费模型

计划通过 OpenCode API Key 获取模型列表，标记并筛选免费模型；`free_models_only = true` 时不会自动降级到收费模型。免费模型会变化，因此不会把示例模型当作永久可用。

## 会话与记忆

当前 SQLite migration 创建 `sessions` 和 `messages` 表。启动时恢复最近会话；用户消息立即保存，完整回答在流结束时保存，取消时保存片段并标记 `interrupted`。长期记忆、FTS5 检索、导出与删除命令尚未实现。

## Persona

启动时会校验并嵌入 [`personas/komari.yaml`](personas/komari.yaml) 作为结构化 System Context，包含不可变人格、自然对话规则、官方资料与用户提供小说笔记提炼出的有限事件锚点。Persona 禁止客服式话术、工作型 Agent 行为和机械口癖；仓库不收录小说全文、截图、立绘或大段对白。资料边界见 [`docs/persona-sources.md`](docs/persona-sources.md)。

## 隐私

- 聊天记录只写入本机 SQLite。
- `.env`、SQLite 数据库和日志被 `.gitignore` 排除。
- Mock Provider 不发送网络请求；DeepSeek 模式只向配置的 Base URL 发送当前上下文。
- DeepSeek Key 优先存入系统 Keyring，不写入普通配置文件；Provider 错误会脱敏。
- 推理流只按配置临时显示，不写入 SQLite；只保存最终回答和必要 Token 统计。
- 当前版本尚未提供数据导出/彻底删除命令，可直接删除用户数据目录中的数据库。

## 开发检查

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Roadmap

1. SQLite FTS5 长期记忆、关系状态及管理命令；
2. Codex OAuth/Bridge 与 OpenCode Zen；
3. Provider/模型切换 TUI 和完整配置 TUI；
4. completion 与多平台发行包。

## 声明与许可证

这是非官方同人项目，与《败北女角太多了！》作者、出版社、动画制作委员会及其他权利人无隶属或授权关系。角色名称及相关作品权利归各自权利人所有。项目不附带官方美术或原作文本。

本仓库代码采用 `GPL-3.0-or-later`，见 [LICENSE](LICENSE)。尚未移植或复制 `oh-my-pi` 代码；接入 Codex 时会先核对来源许可证，并在需要时加入 `NOTICE.md`。
