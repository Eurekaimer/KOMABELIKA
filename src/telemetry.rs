pub fn redact_secrets(input: &str, secrets: &[&str]) -> String {
    let mut redacted = input.to_owned();
    for secret in secrets {
        if !secret.is_empty() {
            redacted = redacted.replace(secret, "[REDACTED]");
        }
    }
    redact_prefixed_token(&mut redacted, "Bearer ");
    redact_prefixed_token(&mut redacted, "sk-");
    redacted
}

fn redact_prefixed_token(value: &mut String, prefix: &str) {
    let mut search_from = 0;
    while let Some(relative_start) = value[search_from..].find(prefix) {
        let prefix_start = search_from + relative_start;
        let token_start = prefix_start + prefix.len();
        let token_end = value[token_start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '"' | '\'' | ',' | '}' | ']' | ')')
            })
            .map_or(value.len(), |offset| token_start + offset);
        if token_end == token_start {
            search_from = token_start;
            continue;
        }
        let replacement_start = if prefix == "sk-" {
            prefix_start
        } else {
            token_start
        };
        value.replace_range(replacement_start..token_end, "[REDACTED]");
        search_from = replacement_start + "[REDACTED]".len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_bearer_and_deepseek_tokens() {
        let message = "request Bearer abc123 failed for sk-secret and exact-value";
        let redacted = redact_secrets(message, &["exact-value"]);

        assert_eq!(
            redacted,
            "request Bearer [REDACTED] failed for [REDACTED] and [REDACTED]"
        );
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("secret"));
    }
}
