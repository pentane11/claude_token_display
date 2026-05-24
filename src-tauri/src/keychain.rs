//! Claude Code が保存している OAuth access_token を OS の keystore から取り出す。
//!
//! - macOS: `/usr/bin/security find-generic-password -a <user> -s "Claude Code-credentials" -w`
//!          を spawn する。直接 Security framework を叩くと ACL ダイアログが出ない
//!          ケースがあるため CLI 経由のほうが安定。
//! - Windows: Claude Code 公式の保存先 `%USERPROFILE%\.claude\.credentials.json`
//!            から読み取り。`CLAUDE_CONFIG_DIR` があればその配下を優先する。
//!
//! 値は JSON 文字列で、`claudeAiOauth.accessToken` を取り出す。

use serde::Deserialize;

const SERVICE_NAME: &str = "Claude Code-credentials";

#[derive(thiserror::Error, Debug)]
pub enum KeychainError {
    #[error("OS keystore access failed: {0}")]
    Access(String),

    #[error("Claude Code credentials not found. Have you logged in via the `claude` CLI?")]
    NotFound,

    #[error("Access to the Claude Code keychain item was denied. Re-run and choose \"Always Allow\" in the dialog.")]
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    AccessDenied,

    #[error("Keychain payload is not valid JSON: {0}")]
    Decode(String),

    #[error("Keychain payload has no claudeAiOauth.accessToken")]
    EmptyToken,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuth>,
}

#[derive(Deserialize)]
struct OAuth {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
}

pub fn read_access_token() -> Result<String, KeychainError> {
    let raw = read_raw()?;
    let payload: Payload =
        serde_json::from_str(&raw).map_err(|e| KeychainError::Decode(e.to_string()))?;
    payload
        .claude_ai_oauth
        .and_then(|o| o.access_token)
        .filter(|t| !t.is_empty())
        .ok_or(KeychainError::EmptyToken)
}

#[cfg(target_os = "macos")]
fn read_raw() -> Result<String, KeychainError> {
    use std::process::Command;

    let user = whoami::username();
    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-a",
            &user,
            "-s",
            SERVICE_NAME,
            "-w",
        ])
        .output()
        .map_err(|e| KeychainError::Access(e.to_string()))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    // 終了コードのざっくり分類
    // 44 = errSecItemNotFound, 51 = errSecAuthFailed (ACL拒否)
    match output.status.code() {
        Some(44) => Err(KeychainError::NotFound),
        Some(51) => Err(KeychainError::AccessDenied),
        Some(45) => Err(KeychainError::AccessDenied), // errSecInteractionNotAllowed
        _ => Err(KeychainError::Access(stderr.trim().to_string())),
    }
}

#[cfg(target_os = "windows")]
fn read_raw() -> Result<String, KeychainError> {
    // Claude Code 2.x on Windows stores OAuth credentials in this JSON file, not in
    // Windows Credential Manager. Keep the old Credential Manager lookup as a
    // fallback in case earlier installs used it.
    match read_raw_from_credentials_file() {
        Ok(s) => return Ok(s),
        Err(KeychainError::NotFound) => {}
        Err(e) => return Err(e),
    }

    read_raw_from_credential_manager()
}

#[cfg(target_os = "windows")]
fn read_raw_from_credentials_file() -> Result<String, KeychainError> {
    let path = claude_credentials_path()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(KeychainError::NotFound),
        Err(e) => Err(KeychainError::Access(format!(
            "failed to read {}: {}",
            path.display(),
            e
        ))),
    }
}

#[cfg(target_os = "windows")]
fn claude_credentials_path() -> Result<std::path::PathBuf, KeychainError> {
    if let Some(config_dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Ok(std::path::PathBuf::from(config_dir).join(".credentials.json"));
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| {
            KeychainError::Access("could not determine Claude Code config directory".into())
        })?;
    Ok(std::path::PathBuf::from(home)
        .join(".claude")
        .join(".credentials.json"))
}

#[cfg(target_os = "windows")]
fn read_raw_from_credential_manager() -> Result<String, KeychainError> {
    let user = whoami::username();
    let entry = keyring::Entry::new(SERVICE_NAME, &user)
        .map_err(|e| KeychainError::Access(e.to_string()))?;
    match entry.get_password() {
        Ok(s) => Ok(s),
        Err(keyring::Error::NoEntry) => Err(KeychainError::NotFound),
        Err(e) => Err(KeychainError::Access(e.to_string())),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn read_raw() -> Result<String, KeychainError> {
    Err(KeychainError::Access(
        "unsupported platform — only macOS / Windows supported".into(),
    ))
}
