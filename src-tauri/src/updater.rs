use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

const GITHUB_API: &str = "https://api.github.com/repos/humos-dev/humos/releases/latest";
const INSTALL_TARGET: &str = "/Applications/humOS.app";
const VALID_URL_PREFIX: &str = "https://github.com/humos-dev/humos/releases/download/";

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    pub stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_auto_restart: Option<bool>,
}

pub fn is_installed_build() -> bool {
    std::env::current_exe()
        .map(|p| p.starts_with("/Applications"))
        .unwrap_or(false)
}

fn emit(app: &AppHandle, state: UpdateState) {
    if let Err(e) = app.emit("update:state", &state) {
        log::warn!("emit update:state failed: {}", e);
    }
}

async fn fetch_download_url(client: &reqwest::Client) -> Result<String, String> {
    let resp = client
        .get(GITHUB_API)
        .header("User-Agent", "humOS-updater/1.0")
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if resp.status() == 403 {
        return Err("GitHub API rate limited. Try again in a few minutes.".into());
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(msg) = body.get("message").and_then(|m| m.as_str()) {
        if msg.contains("rate limit") {
            return Err("GitHub API rate limited. Try again in a few minutes.".into());
        }
    }

    let assets = body.get("assets").and_then(|a| a.as_array()).ok_or("No assets in release")?;

    for asset in assets {
        if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
            if url.contains("arm64.zip") {
                if !url.starts_with(VALID_URL_PREFIX) {
                    return Err(format!("Unexpected download URL: {}", url));
                }
                return Ok(url.to_string());
            }
        }
    }

    Err("No arm64 ZIP found in latest release.".into())
}

async fn download_zip(app: &AppHandle, client: &reqwest::Client, url: &str, dest: &str) -> Result<(), String> {
    use futures_util::StreamExt;

    let resp = client
        .get(url)
        .header("User-Agent", "humOS-updater/1.0")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download HTTP error: {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(total as usize);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrupted: {}", e))?;
        downloaded += chunk.len() as u64;
        buf.extend_from_slice(&chunk);

        if total > 0 {
            let pct = ((downloaded * 100) / total) as u8;
            if pct != last_pct {
                last_pct = pct;
                emit(app, UpdateState {
                    stage: "downloading",
                    progress: Some(pct),
                    error: None,
                    can_auto_restart: None,
                });
            }
        }
    }

    std::fs::write(dest, &buf).map_err(|e| format!("Write ZIP failed: {}", e))?;
    Ok(())
}

fn validate_path_no_single_quote(path: &str) -> Result<(), String> {
    if path.contains('\'') {
        return Err(format!("Temp path contains single quote -- cannot safely pass to osascript. Path: {}", path));
    }
    Ok(())
}

fn install_app(extracted_app: &str) -> Result<(), String> {
    validate_path_no_single_quote(extracted_app)?;

    if std::path::Path::new(INSTALL_TARGET).exists() {
        std::fs::remove_dir_all(INSTALL_TARGET)
            .map_err(|e| format!("Failed to remove old app: {}", e))?;
    }

    let apps_writable = std::path::Path::new("/Applications")
        .metadata()
        .map(|m| !m.permissions().readonly())
        .unwrap_or(false);

    if apps_writable {
        std::fs::rename(extracted_app, INSTALL_TARGET)
            .map_err(|e| format!("Move failed: {}", e))?;
    } else {
        let script = format!(
            "do shell script \"mv '{}' '{}'\" with administrator privileges",
            extracted_app, INSTALL_TARGET
        );
        let out = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("osascript spawn failed: {}", e))?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("-128") || err.contains("cancelled") {
                return Err("Update cancelled.".into());
            }
            return Err(format!("Authentication failed: {}", err.trim()));
        }
    }
    Ok(())
}

async fn run_update(app: AppHandle, updating: Arc<AtomicBool>) {
    let _guard = scopeguard::guard(Arc::clone(&updating), |u| u.store(false, Ordering::SeqCst));

    emit(&app, UpdateState { stage: "checking", progress: None, error: None, can_auto_restart: None });

    let client = reqwest::Client::new();

    let download_url = match fetch_download_url(&client).await {
        Ok(u) => u,
        Err(e) => {
            emit(&app, UpdateState { stage: "error", progress: None, error: Some(e), can_auto_restart: None });
            return;
        }
    };

    emit(&app, UpdateState { stage: "downloading", progress: Some(0), error: None, can_auto_restart: None });

    let tmp_dir = match tempfile::TempDir::new() {
        Ok(d) => d,
        Err(e) => {
            emit(&app, UpdateState { stage: "error", progress: None, error: Some(format!("Temp dir failed: {}", e)), can_auto_restart: None });
            return;
        }
    };

    let zip_path = tmp_dir.path().join("humos-update.zip");
    let zip_str = match zip_path.to_str() {
        Some(s) => s,
        None => {
            emit(&app, UpdateState { stage: "error", progress: None, error: Some("Temp path is not valid UTF-8.".into()), can_auto_restart: None });
            return;
        }
    };

    if let Err(e) = download_zip(&app, &client, &download_url, zip_str).await {
        emit(&app, UpdateState { stage: "error", progress: None, error: Some(e), can_auto_restart: None });
        return;
    }

    emit(&app, UpdateState { stage: "installing", progress: None, error: None, can_auto_restart: None });

    let extract_dir = tmp_dir.path().join("extracted");
    let extract_str = match extract_dir.to_str() {
        Some(s) => s,
        None => {
            emit(&app, UpdateState { stage: "error", progress: None, error: Some("Extract path is not valid UTF-8.".into()), can_auto_restart: None });
            return;
        }
    };

    let unzip_ok = Command::new("unzip")
        .arg("-q")
        .arg(zip_str)
        .arg("-d")
        .arg(extract_str)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !unzip_ok {
        emit(&app, UpdateState { stage: "error", progress: None, error: Some("Archive corrupt or unzip failed.".into()), can_auto_restart: None });
        return;
    }

    let app_bundle = extract_dir.join("humOS.app");
    let app_bundle_str = app_bundle.to_str().unwrap_or("");

    let _ = Command::new("xattr").arg("-cr").arg(app_bundle_str).status();

    if let Err(e) = install_app(app_bundle_str) {
        emit(&app, UpdateState { stage: "error", progress: None, error: Some(e), can_auto_restart: None });
        return;
    }

    emit(&app, UpdateState {
        stage: "ready",
        progress: None,
        error: None,
        can_auto_restart: Some(is_installed_build()),
    });
}

#[tauri::command]
pub async fn start_self_update(
    app: AppHandle,
    updating: State<'_, Arc<AtomicBool>>,
) -> Result<(), String> {
    if updating.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let updating_clone = Arc::clone(&updating);
    tokio::spawn(async move {
        run_update(app, updating_clone).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn restart_app() -> Result<(), String> {
    Command::new("open")
        .arg(INSTALL_TARGET)
        .spawn()
        .map_err(|e| format!("Failed to launch new app: {}", e))?;
    std::process::exit(0);
}
