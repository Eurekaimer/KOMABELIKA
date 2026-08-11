use anyhow::Result;

#[cfg(target_os = "linux")]
use anyhow::Context;

#[cfg(target_os = "linux")]
const SERVICE: &str = "komari-call";
const DEEPSEEK_ACCOUNT: &str = "deepseek-api-key";
const OPENCODE_GO_ACCOUNT: &str = "opencode-go-api-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialSource {
    ProcessArgument,
    Keyring,
    DeepSeekEnvironment,
    OpenCodeEnvironment,
    ConfiguredEnvironment,
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::ProcessArgument => "process argument",
            Self::Keyring => "system keyring",
            Self::DeepSeekEnvironment => "DEEPSEEK_API_KEY",
            Self::OpenCodeEnvironment => "OPENCODE_API_KEY",
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
    resolve_from_sources(
        process_argument,
        keyring_password(DEEPSEEK_ACCOUNT),
        environment_password("DEEPSEEK_API_KEY"),
        (configured_environment != "DEEPSEEK_API_KEY")
            .then(|| environment_password(configured_environment))
            .flatten(),
        CredentialSource::DeepSeekEnvironment,
    )
}

pub fn resolve_opencode_go(
    process_argument: Option<String>,
    configured_environment: &str,
) -> Option<ResolvedCredential> {
    resolve_from_sources(
        process_argument,
        keyring_password(OPENCODE_GO_ACCOUNT),
        environment_password("OPENCODE_API_KEY"),
        (configured_environment != "OPENCODE_API_KEY")
            .then(|| environment_password(configured_environment))
            .flatten(),
        CredentialSource::OpenCodeEnvironment,
    )
}

fn resolve_from_sources(
    process_argument: Option<String>,
    keyring: Option<String>,
    provider_environment: Option<String>,
    configured_environment: Option<String>,
    provider_environment_source: CredentialSource,
) -> Option<ResolvedCredential> {
    [
        (process_argument, CredentialSource::ProcessArgument),
        (keyring, CredentialSource::Keyring),
        (provider_environment, provider_environment_source),
        (
            configured_environment,
            CredentialSource::ConfiguredEnvironment,
        ),
    ]
    .into_iter()
    .find_map(|(value, source)| nonempty(value).map(|value| ResolvedCredential { value, source }))
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| (!value.trim().is_empty()).then_some(value))
}

fn environment_password(name: &str) -> Option<String> {
    nonempty(std::env::var(name).ok())
}

#[cfg(target_os = "linux")]
fn entry(account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).context("could not open the system keyring")
}

#[cfg(target_os = "linux")]
fn keyring_password(account: &str) -> Option<String> {
    entry(account)
        .ok()
        .and_then(|entry| entry.get_password().ok())
}

#[cfg(not(target_os = "linux"))]
fn keyring_password(_account: &str) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
pub fn store_deepseek(password: &str) -> Result<()> {
    store(password, DEEPSEEK_ACCOUNT, "DeepSeek")
}

#[cfg(target_os = "linux")]
pub fn store_opencode_go(password: &str) -> Result<()> {
    store(password, OPENCODE_GO_ACCOUNT, "OpenCode Go")
}

#[cfg(target_os = "linux")]
fn store(password: &str, account: &str, provider_name: &str) -> Result<()> {
    anyhow::ensure!(!password.trim().is_empty(), "API key cannot be empty");
    entry(account)?.set_password(password).with_context(|| {
        format!("could not save the {provider_name} API key to the system keyring")
    })
}

#[cfg(not(target_os = "linux"))]
pub fn store_deepseek(_password: &str) -> Result<()> {
    anyhow::bail!("system keyring storage is currently supported on Linux only")
}

#[cfg(not(target_os = "linux"))]
pub fn store_opencode_go(_password: &str) -> Result<()> {
    anyhow::bail!("system keyring storage is currently supported on Linux only")
}

#[cfg(target_os = "linux")]
pub fn delete_deepseek() -> Result<bool> {
    delete(DEEPSEEK_ACCOUNT, "DeepSeek")
}

#[cfg(target_os = "linux")]
pub fn delete_opencode_go() -> Result<bool> {
    delete(OPENCODE_GO_ACCOUNT, "OpenCode Go")
}

#[cfg(target_os = "linux")]
fn delete(account: &str, provider_name: &str) -> Result<bool> {
    let entry = entry(account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("could not delete the {provider_name} API key"))
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn delete_deepseek() -> Result<bool> {
    anyhow::bail!("system keyring storage is currently supported on Linux only")
}

#[cfg(not(target_os = "linux"))]
pub fn delete_opencode_go() -> Result<bool> {
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

    #[test]
    fn resolves_open_code_sources_in_priority_order_without_leaking_values() {
        let credential = resolve_from_sources(
            Some("  ".into()),
            Some("keyring-secret".into()),
            Some("environment-secret".into()),
            Some("configured-secret".into()),
            CredentialSource::OpenCodeEnvironment,
        )
        .unwrap();
        assert_eq!(credential.expose(), "keyring-secret");
        assert_eq!(credential.source, CredentialSource::Keyring);
        assert!(!format!("{}", credential.source).contains(credential.expose()));

        let credential = resolve_from_sources(
            None,
            None,
            Some("environment-secret".into()),
            Some("configured-secret".into()),
            CredentialSource::OpenCodeEnvironment,
        )
        .unwrap();
        assert_eq!(credential.source, CredentialSource::OpenCodeEnvironment);
        assert_eq!(credential.source.to_string(), "OPENCODE_API_KEY");
        assert!(!credential.source.to_string().contains(credential.expose()));
    }
}
