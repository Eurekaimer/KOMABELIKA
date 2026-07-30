use anyhow::Result;

#[cfg(target_os = "linux")]
use anyhow::Context;

#[cfg(target_os = "linux")]
const SERVICE: &str = "komari-call";
#[cfg(target_os = "linux")]
const DEEPSEEK_ACCOUNT: &str = "deepseek-api-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialSource {
    ProcessArgument,
    Keyring,
    DeepSeekEnvironment,
    ConfiguredEnvironment,
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::ProcessArgument => "process argument",
            Self::Keyring => "system keyring",
            Self::DeepSeekEnvironment => "DEEPSEEK_API_KEY",
            Self::ConfiguredEnvironment => "configured environment variable",
        };
        formatter.write_str(name)
    }
}

pub struct ResolvedCredential {
    value: String,
    pub source: CredentialSource,
}

impl ResolvedCredential {
    pub fn expose(&self) -> &str {
        &self.value
    }
}

pub fn resolve_deepseek(
    process_argument: Option<String>,
    configured_environment: &str,
) -> Option<ResolvedCredential> {
    nonempty(process_argument)
        .map(|value| ResolvedCredential {
            value,
            source: CredentialSource::ProcessArgument,
        })
        .or_else(|| {
            keyring_password().map(|value| ResolvedCredential {
                value,
                source: CredentialSource::Keyring,
            })
        })
        .or_else(|| {
            environment_password("DEEPSEEK_API_KEY").map(|value| ResolvedCredential {
                value,
                source: CredentialSource::DeepSeekEnvironment,
            })
        })
        .or_else(|| {
            (configured_environment != "DEEPSEEK_API_KEY")
                .then(|| environment_password(configured_environment))
                .flatten()
                .map(|value| ResolvedCredential {
                    value,
                    source: CredentialSource::ConfiguredEnvironment,
                })
        })
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| (!value.trim().is_empty()).then_some(value))
}

fn environment_password(name: &str) -> Option<String> {
    nonempty(std::env::var(name).ok())
}

#[cfg(target_os = "linux")]
fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, DEEPSEEK_ACCOUNT).context("could not open the system keyring")
}

#[cfg(target_os = "linux")]
fn keyring_password() -> Option<String> {
    entry().ok().and_then(|entry| entry.get_password().ok())
}

#[cfg(not(target_os = "linux"))]
fn keyring_password() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
pub fn store_deepseek(password: &str) -> Result<()> {
    anyhow::ensure!(!password.trim().is_empty(), "API key cannot be empty");
    entry()?
        .set_password(password)
        .context("could not save the DeepSeek API key to the system keyring")
}

#[cfg(not(target_os = "linux"))]
pub fn store_deepseek(_password: &str) -> Result<()> {
    anyhow::bail!("system keyring storage is currently supported on Linux only")
}

#[cfg(target_os = "linux")]
pub fn delete_deepseek() -> Result<bool> {
    let entry = entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(error) => Err(error).context("could not delete the DeepSeek API key"),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn delete_deepseek() -> Result<bool> {
    anyhow::bail!("system keyring storage is currently supported on Linux only")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_blank_process_credentials() {
        assert!(nonempty(Some("  ".into())).is_none());
        assert_eq!(nonempty(Some("secret".into())).as_deref(), Some("secret"));
    }

    #[test]
    fn credential_debug_output_never_contains_value() {
        let credential = ResolvedCredential {
            value: "sk-private".into(),
            source: CredentialSource::ProcessArgument,
        };
        assert!(!format!("{}", credential.source).contains(credential.expose()));
    }
}
