#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
    pub takes_argument: bool,
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/help",
        usage: "/help",
        description: "显示所有内置命令",
        takes_argument: false,
    },
    SlashCommand {
        name: "/status",
        usage: "/status",
        description: "查看当前 Provider 和模型",
        takes_argument: false,
    },
    SlashCommand {
        name: "/providers",
        usage: "/providers",
        description: "列出可用 Provider",
        takes_argument: false,
    },
    SlashCommand {
        name: "/provider",
        usage: "/provider <名称>",
        description: "切换并保存 Provider",
        takes_argument: true,
    },
    SlashCommand {
        name: "/models",
        usage: "/models",
        description: "列出当前 Provider 的模型",
        takes_argument: false,
    },
    SlashCommand {
        name: "/model",
        usage: "/model <模型 ID>",
        description: "切换并保存模型",
        takes_argument: true,
    },
    SlashCommand {
        name: "/new",
        usage: "/new",
        description: "新建会话",
        takes_argument: false,
    },
    SlashCommand {
        name: "/reasoning",
        usage: "/reasoning on|off",
        description: "显示或隐藏推理内容",
        takes_argument: true,
    },
];

pub fn suggestions(input: &str) -> Vec<SlashCommand> {
    if !input.starts_with('/') || input.contains('\n') {
        return Vec::new();
    }
    let command = input.split_whitespace().next().unwrap_or(input);
    let entering_arguments = input.len() > command.len();
    COMMANDS
        .iter()
        .copied()
        .filter(|candidate| {
            if entering_arguments {
                candidate.name == command
            } else {
                candidate.name.starts_with(command)
            }
        })
        .collect()
}

pub fn help_text() -> String {
    COMMANDS
        .iter()
        .map(|command| format!("{:<24} {}", command.usage, command.description))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrows_command_prefixes_and_keeps_argument_usage() {
        let names = suggestions("/pro")
            .into_iter()
            .map(|command| command.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["/providers", "/provider"]);

        let provider = suggestions("/provider ");
        assert_eq!(provider.len(), 1);
        assert_eq!(provider[0].usage, "/provider <名称>");
        assert!(suggestions("晚上好").is_empty());
    }
}
