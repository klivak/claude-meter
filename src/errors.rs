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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorAction {
    CopyLoginCommand,
    Retry,
    InstallClaude,
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

pub fn action_for(kind: ErrorKind) -> ErrorAction {
    match kind {
        ErrorKind::CredentialsMissing | ErrorKind::TokenExpired => ErrorAction::CopyLoginCommand,
        ErrorKind::RateLimited
        | ErrorKind::Network
        | ErrorKind::Server
        | ErrorKind::Api
        | ErrorKind::WebAuth => ErrorAction::Retry,
        ErrorKind::Unknown => ErrorAction::InstallClaude,
    }
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

    #[test]
    fn chooses_action_for_each_error_class() {
        assert_eq!(
            action_for(ErrorKind::TokenExpired),
            ErrorAction::CopyLoginCommand
        );
        assert_eq!(
            action_for(ErrorKind::CredentialsMissing),
            ErrorAction::CopyLoginCommand
        );
        assert_eq!(action_for(ErrorKind::RateLimited), ErrorAction::Retry);
        assert_eq!(action_for(ErrorKind::Network), ErrorAction::Retry);
        assert_eq!(action_for(ErrorKind::Unknown), ErrorAction::InstallClaude);
    }
}
