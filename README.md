# Komari Call

一个运行在终端里的小鞠知花风格聊天程序。

> 小鞠不需要替我工作。 <del>写代码、修 Linux、整理项目和管理日程，已经有太多 Agent 抢着做了。</del>
> 她只需要陪我聊天就可以了。

## 现在可以做什么

- 在终端 TUI 中聊天，回答会逐字显示；
- 使用 DeepSeek 官方 API，支持 SSE 流式回答和推理内容分离；
- 保存会话，重新启动后继续最近一次聊天；
- 新建和切换会话，取消正在生成的回答；
- 显示简单的 Token 用量；
- 使用结构化的小鞠 Persona；
- 在本地保存配置和 SQLite 数据库。

目前还没有长期记忆、Codex、OpenCode Zen 和配置 TUI。

## 安装

需要 Rust stable 和 C/C++ 链接器。

```bash
cargo install --git https://github.com/Eurekaimer/KOMABELIKA.git --locked
```

也可以直接运行源码：

```bash
git clone https://github.com/Eurekaimer/KOMABELIKA.git
cd KOMABELIKA
cargo run --release
```

Nix 用户可以直接运行：

```bash
nix run github:Eurekaimer/KOMABELIKA
```

DeepSeek 是默认聊天通道，第一次运行前需要设置 API Key。

## 使用 DeepSeek

Linux 下可以把 API Key 保存到系统 Keyring：

```bash
komari-call login deepseek
komari-call
```

默认模型是当前 API 提供的快速模型 `deepseek-v4-flash`。需要更换时可以运行：

```bash
komari-call config --model <模型 ID>
```

也可以通过环境变量或当前进程参数传入 Key：

```bash
export DEEPSEEK_API_KEY="你的 API Key"
komari-call

# 或者只传给当前进程
komari-call chat --api-key "$DEEPSEEK_API_KEY"
```

查看 DeepSeek 当前返回的模型：

```bash
komari-call models --provider deepseek
```

凭据读取顺序为：`--api-key`、系统 Keyring、`DEEPSEEK_API_KEY`、配置中指定的环境变量。Key 不会写入配置文件或日志。

## 键位

- `Enter`：发送
- `Shift+Enter`：换行
- `Esc`：取消生成
- `Ctrl+N`：新建会话
- `Ctrl+L`：切换会话
- `Ctrl+P`：输入 Provider 切换命令
- `Ctrl+C`：保存并退出

聊天界面支持以下命令：

```text
/help
/status
/providers
/provider deepseek
/models
/model deepseek-v4-flash
/new
/reasoning on|off
```

`/provider` 和 `/model` 会校验可用性并保存选择，下次启动继续使用。

## 本地数据

聊天记录只保存在本机 SQLite 数据库中。推理内容只在启用显示时临时出现在界面中，不会保存。

可以用环境变量指定配置和数据目录：

```bash
KOMARI_CALL_CONFIG="$PWD/config.toml" \
KOMARI_CALL_DATA_DIR="$PWD/.local-data" \
komari-call
```

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Persona 的资料范围和整理方式见 [`docs/persona-sources.md`](docs/persona-sources.md)。本项目是非官方同人项目，不附带官方图片或原作文本。代码使用 GPL-3.0-or-later 许可证。
