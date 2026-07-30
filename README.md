# Komari Call

一个运行在终端里的小鞠知花风格聊天程序。

> 小鞠不需要替我工作。 <del>写代码、修 Linux、整理项目和管理日程，已经有太多 Agent 抢着做了。</del>
> 她只需要陪我聊天就可以了。

## 重要：需要你自己的 DeepSeek API Key

**本仓库不提供、内置或共享任何 DeepSeek API Key。仓库维护者的 Key 不会随程序发布，也不会提供给其他用户。**

安装后，每位用户都必须前往 DeepSeek 官方平台申请并使用自己的 API Key。API 调用产生的额度消耗或费用由该 Key 所属账户承担。

`deepseek-v4-flash` 只是程序预填的默认**模型 ID**，不代表项目附送模型服务或维护者账户权限。你的 DeepSeek 账户如果不能使用它，请先查询自己实际可用的模型，再修改配置：

```bash
komari-call models --provider deepseek
komari-call config --model <你的账户可用的模型 ID>
```

程序不会在模型不可用时偷偷切换到其他收费模型。

## 功能

- 终端 TUI 聊天和流式输出；
- DeepSeek 官方 API；
- 会话保存、恢复、新建和切换；
- 生成取消和简单 Token 用量；
- 结构化小鞠 Persona；
- 本地 TOML 配置和 SQLite 数据库。

目前尚未实现长期记忆、Codex、OpenCode Zen 和配置 TUI。

## 安装

需要 Rust stable 和 C/C++ 链接器：

```bash
cargo install --git https://github.com/Eurekaimer/KOMABELIKA.git --locked
```

Nix 用户：

```bash
nix run github:Eurekaimer/KOMABELIKA
```

也可以从源码运行：

```bash
git clone https://github.com/Eurekaimer/KOMABELIKA.git
cd KOMABELIKA
cargo run --release
```

## 配置自己的 API Key

推荐保存到系统 Keyring（当前系统支持时）：

```bash
komari-call login deepseek
komari-call
```

也可以只在当前 shell 中设置环境变量：

```bash
export DEEPSEEK_API_KEY="你的 DeepSeek API Key"
komari-call
```

或只传给本次进程：

```bash
komari-call chat --api-key "$DEEPSEEK_API_KEY"
```

凭据读取顺序：`--api-key`、系统 Keyring、`DEEPSEEK_API_KEY`、配置中指定的环境变量。Key 不会写入普通配置文件、聊天数据库或日志；请勿把 `.env`、终端输出或 Key 提交到 Git。

## 常用命令

```bash
komari-call                                      # 进入聊天
komari-call login deepseek                       # 保存自己的 Key
komari-call logout deepseek                      # 删除 Keyring 中的 Key
komari-call models --provider deepseek            # 查看账户可用模型
komari-call config --model <模型 ID>              # 修改默认模型
komari-call doctor                               # 检查配置、数据库、凭据和模型
komari-call --help                               # 查看全部命令
```

聊天界面命令：

```text
/help                     显示命令帮助
/status                   显示当前 Provider 和模型
/models                   查询当前 Provider 的模型
/model <模型 ID>           切换并保存模型
/new                      新建会话
/reasoning on|off         显示或隐藏推理内容
```

输入 `/` 会显示候选；用 `↑`、`↓` 选择，`Tab` 补全。

## 键位

- `Enter`：发送
- `Shift+Enter`：换行
- `Esc`：取消当前生成
- `Ctrl+N`：新建会话
- `Ctrl+L`：切换会话
- `Ctrl+P`：输入 Provider 切换命令
- `Ctrl+C`：保存并退出

## 本地数据与隐私

聊天记录只保存在本机 SQLite 数据库中。推理内容只在启用显示时临时出现在界面中，不会保存。可以通过 `KOMARI_CALL_CONFIG` 和 `KOMARI_CALL_DATA_DIR` 修改配置与数据目录。

Persona 资料来源见 [`docs/persona-sources.md`](docs/persona-sources.md)。本项目是非官方同人项目，不附带官方图片或原作文本。代码使用 GPL-3.0-or-later 许可证。
