mod args;

pub use args::{ChatArgs, ConfigArgs, CredentialArgs, CredentialProvider, ModelsArgs};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "komari-call",
    version,
    about = "Quiet conversation with Komari in your terminal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Open the chat TUI (default)
    Chat(ChatArgs),
    /// Show or update local configuration
    Config(ConfigArgs),
    /// List models available from a provider
    Models(ModelsArgs),
    /// Save a provider credential in the system keyring
    Login(CredentialArgs),
    /// Remove a provider credential from the system keyring
    Logout(CredentialArgs),
    /// Check configuration, database, credentials, and provider access
    Doctor,
}

#[cfg(test)]
mod tests {
    use super::args::ProviderId;
    use super::*;

    #[test]
    fn chat_is_optional() {
        assert!(Cli::try_parse_from(["komari-call"]).is_ok());
        assert!(matches!(
            Cli::try_parse_from(["komari-call", "chat"])
                .unwrap()
                .command,
            Some(Command::Chat(_))
        ));
    }

    #[test]
    fn parses_configuration_updates() {
        let cli = Cli::try_parse_from([
            "komari-call",
            "config",
            "--provider",
            "deepseek",
            "--model",
            "deepseek-v4-flash",
            "--deepseek-thinking",
            "false",
        ])
        .unwrap();
        let Some(Command::Config(args)) = cli.command else {
            panic!("expected config command");
        };
        assert_eq!(args.provider, Some(ProviderId::Deepseek));
        assert_eq!(args.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(args.deepseek_thinking, Some(false));
        assert!(args.has_changes());
    }

    #[test]
    fn parses_provider_management_commands() {
        assert!(matches!(
            Cli::try_parse_from(["komari-call", "login", "deepseek"])
                .unwrap()
                .command,
            Some(Command::Login(CredentialArgs {
                provider: CredentialProvider::Deepseek
            }))
        ));
        assert!(matches!(
            Cli::try_parse_from(["komari-call", "models", "--provider", "deepseek"])
                .unwrap()
                .command,
            Some(Command::Models(_))
        ));
    }
}
