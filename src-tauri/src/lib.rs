mod api;
mod keychain;
#[cfg(target_os = "macos")]
mod macos_panel;
mod tray;

use api::UsageSnapshot;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use tauri::Manager;

/// popover が show() された最後の時刻 (epoch ms)。表示直後の
/// Focused(false) によるオートクローズを抑制するための grace 用。
pub static SHOWN_AT_MS: AtomicI64 = AtomicI64::new(0);
const FOCUS_LOSS_GRACE_MS: i64 = 300;
const RESIZE_AUTO_HIDE_SUPPRESSION_MS: i64 = 4_000;

static POPOVER_PINNED: AtomicBool = AtomicBool::new(false);
static POPOVER_AUTO_HIDE_SUPPRESSED_UNTIL_MS: AtomicI64 = AtomicI64::new(0);

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchResult {
    Ok(UsageSnapshot),
    RateLimited { retry_after_secs: Option<u64> },
    Err { message: String },
}

#[tauri::command]
async fn get_usage() -> FetchResult {
    fetch_usage_inner().await
}

#[tauri::command]
fn get_popover_pinned() -> bool {
    is_popover_pinned()
}

#[tauri::command]
fn set_popover_pinned(pinned: bool) -> bool {
    POPOVER_PINNED.store(pinned, Ordering::SeqCst);
    pinned
}

#[tauri::command]
fn suppress_popover_auto_hide() {
    suppress_popover_auto_hide_for(RESIZE_AUTO_HIDE_SUPPRESSION_MS);
}

pub fn is_popover_pinned() -> bool {
    POPOVER_PINNED.load(Ordering::SeqCst)
}

pub fn is_popover_auto_hide_suppressed() -> bool {
    now_ms() < POPOVER_AUTO_HIDE_SUPPRESSED_UNTIL_MS.load(Ordering::SeqCst)
}

fn suppress_popover_auto_hide_for(duration_ms: i64) {
    POPOVER_AUTO_HIDE_SUPPRESSED_UNTIL_MS.store(now_ms() + duration_ms, Ordering::SeqCst);
}

fn focus_loss_should_be_ignored(
    pinned: bool,
    shown_at: i64,
    suppressed_until: i64,
    now: i64,
) -> bool {
    pinned || now < suppressed_until || now - shown_at < FOCUS_LOSS_GRACE_MS
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub async fn fetch_usage_inner() -> FetchResult {
    let token = match keychain::read_access_token() {
        Ok(t) => t,
        Err(e) => {
            return FetchResult::Err {
                message: e.to_string(),
            }
        }
    };
    match api::fetch_usage(&token).await {
        Ok(snapshot) => FetchResult::Ok(snapshot),
        Err(api::ApiError::RateLimited { retry_after_secs }) => {
            FetchResult::RateLimited { retry_after_secs }
        }
        Err(e) => FetchResult::Err {
            message: e.to_string(),
        },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![
            get_usage,
            get_popover_pinned,
            set_popover_pinned,
            suppress_popover_auto_hide
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            if let Some(popover) = app.get_webview_window("popover") {
                let _ = popover.set_visible_on_all_workspaces(true);
                #[cfg(target_os = "macos")]
                {
                    // 起動時に NSWindow → NSPanel に class 書き換え + NonactivatingPanel
                    macos_panel::convert_to_nspanel(&popover);
                    macos_panel::promote_to_floating_panel(&popover);
                    // アプリ外クリックを監視して popover を hide するモニタを登録
                    macos_panel::install_outside_click_dismiss(app.handle().clone());
                }
            }
            tray::setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if window.label() == "popover" {
                    let shown_at = SHOWN_AT_MS.load(Ordering::SeqCst);
                    let suppressed_until =
                        POPOVER_AUTO_HIDE_SUPPRESSED_UNTIL_MS.load(Ordering::SeqCst);
                    let now = now_ms();
                    if focus_loss_should_be_ignored(
                        is_popover_pinned(),
                        shown_at,
                        suppressed_until,
                        now,
                    ) {
                        return;
                    }
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_loss_is_ignored_while_pinned() {
        assert!(focus_loss_should_be_ignored(true, 0, 0, 1_000));
    }

    #[test]
    fn focus_loss_is_ignored_during_show_grace() {
        assert!(focus_loss_should_be_ignored(false, 1_000, 0, 1_100));
    }

    #[test]
    fn focus_loss_is_ignored_during_resize_suppression() {
        assert!(focus_loss_should_be_ignored(false, 0, 2_000, 1_000));
    }

    #[test]
    fn focus_loss_is_not_ignored_after_grace_and_suppression() {
        assert!(!focus_loss_should_be_ignored(false, 1_000, 1_500, 2_000));
    }
}
