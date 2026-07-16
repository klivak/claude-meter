//! Auto-update checker: queries GitHub Releases API for newer versions.

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/klivak/claudemeter/releases/latest";

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub tag: String,
    pub html_url: String,
    pub asset_url: String,
    pub checksum_url: String,
}

/// Check if a newer version is available on GitHub.
/// Returns release metadata if a newer version is available.
pub async fn check_for_update() -> Option<UpdateInfo> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("ClaudeMeter/{}", CURRENT_VERSION))
        .build()
        .ok()?;

    let resp = client.get(GITHUB_RELEASES_URL).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    let html_url = json.get("html_url")?.as_str()?;
    let assets = json.get("assets")?.as_array()?;
    let asset = assets.iter().find(|asset| {
        asset.get("name").and_then(|name| name.as_str()) == Some("claudemeter.exe")
    })?;
    let asset_url = asset.get("browser_download_url")?.as_str()?;
    let checksum_url = assets
        .iter()
        .find(|asset| {
            asset.get("name").and_then(|name| name.as_str()) == Some("claudemeter.exe.sha256")
        })
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(|url| url.as_str())
        .map(str::to_string)?;

    let remote_ver = tag.trim_start_matches('v');
    if is_newer(remote_ver, CURRENT_VERSION) {
        Some(UpdateInfo {
            tag: tag.to_string(),
            html_url: html_url.to_string(),
            asset_url: asset_url.to_string(),
            checksum_url,
        })
    } else {
        None
    }
}

/// Download and stage an update, then launch a helper that replaces the running executable.
pub async fn download_and_install(update: &UpdateInfo) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(format!("ClaudeMeter/{}", CURRENT_VERSION))
        .build()
        .map_err(|error| error.to_string())?;

    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let download_path = current_exe.with_extension("exe.download");
    let bytes = client
        .get(&update.asset_url)
        .send()
        .await
        .map_err(|error| format!("Download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download failed: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("Download failed: {error}"))?;

    let checksum = client
        .get(&update.checksum_url)
        .send()
        .await
        .map_err(|error| format!("Checksum download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Checksum download failed: {error}"))?
        .text()
        .await
        .map_err(|error| format!("Checksum download failed: {error}"))?;
    let expected_hash = parse_checksum(&checksum)?;

    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(&bytes);
    let actual_hash = format!("{:x}", hasher.finalize());
    if expected_hash != actual_hash {
        return Err("Downloaded update failed SHA-256 verification".to_string());
    }

    std::fs::write(&download_path, &bytes).map_err(|error| error.to_string())?;
    let script_path =
        std::env::temp_dir().join(format!("claudemeter-update-{}.ps1", std::process::id()));
    let backup_path = current_exe.with_extension("exe.backup");
    let script = format!(
        "$ErrorActionPreference = 'Stop'\nStart-Sleep -Seconds 2\nCopy-Item -LiteralPath '{}' -Destination '{}' -Force\ntry {{\n  Move-Item -LiteralPath '{}' -Destination '{}' -Force\n  Start-Process -FilePath '{}'\n  Remove-Item -LiteralPath '{}' -Force\n}} catch {{\n  Move-Item -LiteralPath '{}' -Destination '{}' -Force\n  throw\n}}\nRemove-Item -LiteralPath $PSCommandPath -Force\n",
        powershell_quote(&current_exe),
        powershell_quote(&backup_path),
        powershell_quote(&download_path),
        powershell_quote(&current_exe),
        powershell_quote(&current_exe),
        powershell_quote(&backup_path),
        powershell_quote(&backup_path),
        powershell_quote(&current_exe),
    );
    std::fs::write(&script_path, script).map_err(|error| error.to_string())?;
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        .arg(&script_path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn powershell_quote(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn parse_checksum(content: &str) -> Result<String, String> {
    let value = content
        .split_whitespace()
        .next()
        .ok_or_else(|| "Checksum file was empty".to_string())?
        .to_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Checksum file did not contain a valid SHA-256 hash".to_string());
    }
    Ok(value)
}

/// Simple semver comparison: returns true if `remote` > `current`.
fn is_newer(remote: &str, current: &str) -> bool {
    let parse =
        |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };
    let r = parse(remote);
    let c = parse(current);
    for i in 0..3 {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv > cv {
            return true;
        }
        if rv < cv {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.4.0", "1.3.6"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.3.6", "1.3.6"));
        assert!(!is_newer("1.3.5", "1.3.6"));
        assert!(is_newer("1.3.7", "1.3.6"));
    }

    #[test]
    fn test_is_newer_handles_missing_components() {
        assert!(is_newer("2", "1.9.9"));
        assert!(!is_newer("2.0", "2.0.0"));
        assert!(!is_newer("not-a-version", "2.0.0"));
    }

    #[test]
    fn test_parse_checksum() {
        let hash = "A".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{}  claudemeter.exe\n", hash)),
            Ok(hash.to_lowercase())
        );
        assert!(parse_checksum("").is_err());
        assert!(parse_checksum("not-a-sha256").is_err());
        assert!(parse_checksum(&"a".repeat(63)).is_err());
    }

    #[test]
    fn test_powershell_quote_escapes_apostrophes() {
        let path = std::path::Path::new(r"C:\Users\O'Neil\ClaudeMeter.exe");
        assert_eq!(powershell_quote(path), r"C:\Users\O''Neil\ClaudeMeter.exe");
    }
}
