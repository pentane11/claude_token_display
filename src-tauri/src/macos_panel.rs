//! macOS でポップオーバーを他アプリのフルスクリーン上に表示する。
//!
//! やっていること:
//!  1. NSWindow の class を NSPanel に書き換える（object_setClass 相当）
//!  2. styleMask に NonactivatingPanel を立てる
//!     → アプリをアクティブ化せずに前面に出るパネル化
//!  3. collectionBehavior に CanJoinAllSpaces / FullScreenAuxiliary を立てる
//!     → 他アプリのフルスクリーン Space にも出現
//!  4. level を NSPopUpMenuWindowLevel (=101) に
//!     → フルスクリーンアプリの window より上
//!
//! 1+2 が肝。3+4 だけでは Accessory アプリの window では Space 越境が起こらない。

#![cfg(target_os = "macos")]

use block2::RcBlock;
use objc2::class;
use objc2_app_kit::{
    NSEvent, NSEventMask, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use tauri::Manager;

// objc runtime の object_setClass を直接呼ぶ。
// objc2 の `set_class` は old/new クラスの instance_size が等しくないと panic するが、
// Tauri/Tao が NSWindow に 8 バイト分の ivar を足しているため NSPanel への
// swap で必ず panic する。これを迂回するため raw FFI で呼び出す。
// 8 バイトの読み取りは popover として使う限り発生しない想定。
unsafe extern "C" {
    fn object_setClass(obj: *mut c_void, cls: *mut c_void) -> *mut c_void;
}

fn ns_window_ref<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Option<&NSWindow> {
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    unsafe { Some(&*(ptr as *const NSWindow)) }
}

/// 既存の NSWindow を NSPanel に「クラス書き換え」する。
/// 起動時に一度だけ呼ぶ。
pub fn convert_to_nspanel<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Some(ns_window) = ns_window_ref(window) else {
        return;
    };
    let raw_obj = ns_window as *const NSWindow as *mut c_void;
    let cls = class!(NSPanel);
    let cls_ptr = cls as *const _ as *mut c_void;

    // raw FFI で class swap（objc2 の size 安全チェックを迂回）
    unsafe {
        object_setClass(raw_obj, cls_ptr);
    }

    // NonactivatingPanel を styleMask に追加: アプリをアクティブ化せずに前面に出す。
    // 副作用として key state にならないので、外しクリック検出は別途
    // NSEvent グローバルモニタ (install_outside_click_dismiss) で行う。
    let current_mask = ns_window.styleMask();
    let new_mask = current_mask | NSWindowStyleMask::NonactivatingPanel;
    ns_window.setStyleMask(new_mask);
}

/// アプリ外（他アプリのウィンドウやデスクトップ）でマウスダウンが起きたら
/// popover を hide する。NonactivatingPanel 構成では Focused(false) が
/// 発火しないため、これで代替する。
pub fn install_outside_click_dismiss<R: tauri::Runtime + 'static>(handle: tauri::AppHandle<R>) {
    let block = RcBlock::new(move |_event: NonNull<NSEvent>| {
        if crate::is_popover_pinned() || crate::is_popover_auto_hide_suppressed() {
            return;
        }
        if let Some(window) = handle.get_webview_window("popover") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            }
        }
    });

    let mask = NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown;
    unsafe {
        let _ = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
    }
    // monitor が生きている間 block も生かす必要があるので leak（プロセス終了まで保持）
    Box::leak(Box::new(block));
}

/// 表示直前に毎回呼ぶ。フルスク対応のフラグと level を確実に立てる。
pub fn promote_to_floating_panel<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Some(ns_window) = ns_window_ref(window) else {
        return;
    };
    let current = ns_window.collectionBehavior();
    let next = current
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::Transient;
    ns_window.setCollectionBehavior(next);

    let pop_up_level: NSWindowLevel = 101;
    ns_window.setLevel(pop_up_level);

    ns_window.setHidesOnDeactivate(false);
}

#[allow(dead_code)]
pub fn order_front_regardless<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Some(ns_window) = ns_window_ref(window) else {
        return;
    };
    ns_window.orderFrontRegardless();
}
