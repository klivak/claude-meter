#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    CredentialsMissing,
    TokenExpired,
    RateLimited,
    Network,
    Server,
    Api,
    WebAuth,
    Unknown,
}

pub fn classify(message: &str) -> ErrorKind {
    let tag = message
        .strip_prefix('[')
        .and_then(|value| value.split_once(']'))
        .map(|(tag, _)| tag);

    match tag {
        Some("token_expired") => ErrorKind::TokenExpired,
        Some("rate_limited") => ErrorKind::RateLimited,
        Some("network_error") => ErrorKind::Network,
        Some("server_error") => ErrorKind::Server,
        Some("api_error") => ErrorKind::Api,
        Some("web_auth_failed") => ErrorKind::WebAuth,
        _ if message.contains("credentials not found")
            || message.contains("accessToken field not found") =>
        {
            ErrorKind::CredentialsMissing
        }
        _ => ErrorKind::Unknown,
    }
}

pub fn detail(message: &str) -> &str {
    message
        .split_once("] ")
        .map(|(_, detail)| detail)
        .unwrap_or(message)
}

pub fn retry_after_seconds(message: &str) -> Option<u64> {
    let value = detail(message)
        .strip_prefix("Retry after ")?
        .strip_suffix('s')?;
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_tagged_errors() {
        assert_eq!(classify("[token_expired] expired"), ErrorKind::TokenExpired);
        assert_eq!(
            classify("[rate_limited] Retry after 90s"),
            ErrorKind::RateLimited
        );
        assert_eq!(classify("[network_error] offline"), ErrorKind::Network);
    }

    #[test]
    fn extracts_retry_after() {
        assert_eq!(
            retry_after_seconds("[rate_limited] Retry after 120s"),
            Some(120)
        );
        assert_eq!(retry_after_seconds("[rate_limited] limited"), None);
    }

    #[test]
    fn classifies_auth_and_credential_errors() {
        assert_eq!(classify("[web_auth_failed] invalid"), ErrorKind::WebAuth);
        assert_eq!(
            classify("Claude Code credentials not found"),
            ErrorKind::CredentialsMissing
        );
        assert_eq!(
            classify("accessToken field not found"),
            ErrorKind::CredentialsMissing
        );
        assert_eq!(classify("unexpected response"), ErrorKind::Unknown);
    }

    #[test]
    fn extracts_error_detail_without_a_tag() {
        assert_eq!(detail("[network_error] offline"), "offline");
        assert_eq!(detail("plain error"), "plain error");
    }
}
