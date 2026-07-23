#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};

#[derive(Debug, Clone)]
pub struct CredentialInfo {
    pub access_token: String,
    /// e.g. "max", "pro" — from credentials file, if available
    pub subscription_type: Option<String>,
    /// e.g. "default_claude_max_5x" — rate limit tier from credentials
    pub rate_limit_tier: Option<String>,
    /// Token expiry time in milliseconds since epoch
    #[cfg_attr(not(windows), allow(dead_code))] // read by the Windows app
    pub expires_at: Option<u64>,
}

#[derive(Debug)]
pub enum CredentialError {
    NotFound,
    #[cfg_attr(not(windows), allow(dead_code))] // constructed by the Windows credential path
    WindowsError(String),
    ParseError(String),
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_os = "macos")]
            Self::NotFound => write!(
                f,
                "Claude Code credentials not found in Keychain or ~/.claude/.credentials.json"
            ),
            #[cfg(not(target_os = "macos"))]
            Self::NotFound => write!(
                f,
                "Claude Code credentials not found in ~/.claude/.credentials.json"
            ),
            Self::WindowsError(e) => write!(f, "Platform credential store error: {e}"),
            Self::ParseError(e) => write!(f, "Failed to parse credential JSON: {e}"),
        }
    }
}

/// Read the Claude OAuth access token.
///
/// Checks locations in order:
/// 1. File-based: `~/.claude/.credentials.json` (Claude Code v2.x on Linux; macOS fallback)
/// 2. Platform credential store:
///    - Windows Credential Manager, target "Claude Code-credentials"
///    - macOS Keychain, generic password with service "Claude Code-credentials"
pub fn read_claude_token() -> Result<CredentialInfo, CredentialError> {
    // Try file-based credentials first (cheap check, no user prompts)
    if let Ok(info) = read_token_from_file() {
        return Ok(info);
    }

    // Fall back to the platform credential store
    read_token_from_credential_store()
}

/// Read token from ~/.claude/.credentials.json
fn read_token_from_file() -> Result<CredentialInfo, CredentialError> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| CredentialError::NotFound)?;

    let cred_path = std::path::Path::new(&home)
        .join(".claude")
        .join(".credentials.json");

    let contents = std::fs::read_to_string(&cred_path).map_err(|_| CredentialError::NotFound)?;

    extract_credential_info(&contents)
}

/// Read token from Windows Credential Manager (legacy path)
#[cfg(windows)]
fn read_token_from_credential_store() -> Result<CredentialInfo, CredentialError> {
    const TARGET: &str = "Claude Code-credentials";

    let target_wide: Vec<u16> = TARGET.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut pcredential: *mut CREDENTIALW = std::ptr::null_mut();
        let result = CredReadW(
            PCWSTR(target_wide.as_ptr()),
            CRED_TYPE_GENERIC,
            0,
            &mut pcredential,
        );

        if result.is_err() {
            let err = windows::core::Error::from_win32();
            if err.code().0 == 0x80070490u32 as i32 {
                return Err(CredentialError::NotFound);
            }
            return Err(CredentialError::WindowsError(err.to_string()));
        }

        if pcredential.is_null() {
            return Err(CredentialError::NotFound);
        }

        let cred = &*pcredential;
        let blob_size = cred.CredentialBlobSize as usize;
        let blob_ptr = cred.CredentialBlob;

        if blob_ptr.is_null() || blob_size == 0 {
            CredFree(pcredential as *mut _);
            return Err(CredentialError::ParseError(
                "Empty credential blob".to_string(),
            ));
        }

        let json_string = if blob_size % 2 == 0 {
            let wide_len = blob_size / 2;
            let wide_slice = std::slice::from_raw_parts(blob_ptr as *const u16, wide_len);
            let wide_slice = if wide_slice.last() == Some(&0) {
                &wide_slice[..wide_len - 1]
            } else {
                wide_slice
            };
            match String::from_utf16(wide_slice) {
                Ok(s) if s.starts_with('{') => s,
                _ => {
                    let bytes = std::slice::from_raw_parts(blob_ptr, blob_size);
                    String::from_utf8_lossy(bytes)
                        .trim_end_matches('\0')
                        .to_string()
                }
            }
        } else {
            let bytes = std::slice::from_raw_parts(blob_ptr, blob_size);
            String::from_utf8_lossy(bytes)
                .trim_end_matches('\0')
                .to_string()
        };

        CredFree(pcredential as *mut _);

        extract_credential_info(&json_string)
    }
}

/// Read token from the macOS Keychain.
///
/// Claude Code on macOS stores credentials as a generic password with service
/// "Claude Code-credentials" (same JSON blob as the Windows Credential Manager),
/// NOT in `~/.claude/.credentials.json`. Uses the `security` CLI to avoid a
/// Security.framework dependency. The first read may show a Keychain access
/// prompt — the user should click "Always Allow".
#[cfg(target_os = "macos")]
fn read_token_from_credential_store() -> Result<CredentialInfo, CredentialError> {
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .map_err(|_| CredentialError::NotFound)?;

    if !output.status.success() {
        return Err(CredentialError::NotFound);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CredentialError::NotFound);
    }

    // `security -w` prints hex instead of text when the stored value contains
    // non-printable bytes (e.g. embedded newlines). Decode if it looks like hex.
    if !raw.starts_with('{') && raw.len() % 2 == 0 && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        let bytes: Vec<u8> = (0..raw.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
            .collect();
        let decoded = String::from_utf8_lossy(&bytes);
        return extract_credential_info(decoded.trim());
    }

    extract_credential_info(raw)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn read_token_from_credential_store() -> Result<CredentialInfo, CredentialError> {
    Err(CredentialError::NotFound)
}

fn extract_credential_info(json: &str) -> Result<CredentialInfo, CredentialError> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| CredentialError::ParseError(format!("Invalid JSON: {e}")))?;

    let oauth = v.get("claudeAiOauth");

    // Try nested: {"claudeAiOauth": {"accessToken": "..."}}
    if let Some(token) = oauth
        .and_then(|o| o.get("accessToken"))
        .and_then(|t| t.as_str())
    {
        let subscription_type = oauth
            .and_then(|o| o.get("subscriptionType"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let rate_limit_tier = oauth
            .and_then(|o| o.get("rateLimitTier"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let expires_at = oauth
            .and_then(|o| o.get("expiresAt"))
            .and_then(|v| v.as_u64());
        return Ok(CredentialInfo {
            access_token: token.to_string(),
            subscription_type,
            rate_limit_tier,
            expires_at,
        });
    }

    // Try flat: {"accessToken": "..."}
    if let Some(token) = v.get("accessToken").and_then(|t| t.as_str()) {
        let subscription_type = v
            .get("subscriptionType")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let rate_limit_tier = v
            .get("rateLimitTier")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let expires_at = v.get("expiresAt").and_then(|v| v.as_u64());
        return Ok(CredentialInfo {
            access_token: token.to_string(),
            subscription_type,
            rate_limit_tier,
            expires_at,
        });
    }

    Err(CredentialError::ParseError(
        "accessToken field not found in credential JSON".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_nested() {
        let json = r#"{"claudeAiOauth": {"accessToken": "test-token-nested"}}"#;
        let info = extract_credential_info(json).unwrap();
        assert_eq!(info.access_token, "test-token-nested");
        assert_eq!(info.subscription_type, None);
    }

    #[test]
    fn test_extract_nested_with_subscription() {
        let json = r#"{"claudeAiOauth": {"accessToken": "tok", "subscriptionType": "max"}}"#;
        let info = extract_credential_info(json).unwrap();
        assert_eq!(info.access_token, "tok");
        assert_eq!(info.subscription_type, Some("max".to_string()));
    }

    #[test]
    fn test_extract_flat() {
        let json = r#"{"accessToken": "test-token-flat"}"#;
        let info = extract_credential_info(json).unwrap();
        assert_eq!(info.access_token, "test-token-flat");
    }

    #[test]
    fn test_extract_missing() {
        let json = r#"{"other": "data"}"#;
        assert!(matches!(
            extract_credential_info(json),
            Err(CredentialError::ParseError(_))
        ));
    }
}
