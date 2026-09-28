#![allow(non_snake_case)]

pub mod crypto;
pub mod ds;
pub mod error;
pub mod mimo;
pub mod storage;
pub mod tray;
pub mod windows;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        eprintln!($($arg)*);
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}

use storage::Storage;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::collections::HashSet;
use futures::FutureExt;
use tokio::sync::watch;
use tauri::{Emitter, Manager};

pub struct TrendDetailState {
    pub data: Mutex<serde_json::Value>,
}

pub struct RefreshNotifier {
    pub tx: watch::Sender<u64>,
}

const APP_DATA_DIR_NAME: &str = "ModelMeter";
const LEGACY_APP_DATA_DIR_NAME: &str = "DeepSeekDesktopAssistant";

/// 定期静默续期登录态的间隔（秒）。每 30 分钟用现有 WebView 会话刷新一次登录态快照。
const SILENT_REFRESH_INTERVAL_SECS: u64 = 1800;
static LAST_SILENT_REFRESH: AtomicU64 = AtomicU64::new(0);

pub fn run() {
    let storage = Arc::new(Storage::new());

    // 初始化 storage
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let app_data_root = std::path::PathBuf::from(local_app_data);
        let current_dir = app_data_root.join(APP_DATA_DIR_NAME);
        let legacy_dir = app_data_root.join(LEGACY_APP_DATA_DIR_NAME);
        let data_dir = if current_dir.exists() || !legacy_dir.exists() {
            current_dir
        } else if std::fs::rename(&legacy_dir, &current_dir).is_ok() {
            current_dir
        } else {
            // Keep reading the legacy directory if another process or a security
            // tool prevents the one-time rename.
            legacy_dir
        };
        storage.init(data_dir);
    }

    // 初始化刷新间隔 watch channel，默认 60 秒
    let default_interval = storage.load_refresh_interval() as u64;
    let (refresh_tx, refresh_rx) = watch::channel(default_interval);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .manage(storage)
        .manage(ds::api::ApiState::new())
        .manage(mimo::api::ApiState::new())
        .manage(TrendDetailState {
            data: Mutex::new(serde_json::json!({})),
        })
        .manage(Arc::new(Mutex::new(HashSet::<String>::new())))
        .manage(Arc::new(AtomicUsize::new(0)))
        .manage(Arc::new(RefreshNotifier { tx: refresh_tx }))
        .invoke_handler(tauri::generate_handler![
            // DeepSeek commands (ds_ prefix)
            ds::commands::ds_refresh_data,
            ds::commands::ds_get_cached_data,
            ds::commands::ds_has_platform_session,
            ds::commands::ds_mark_onboarding_completed,
            ds::commands::ds_open_login_window,
            ds::commands::ds_clear_platform_session,
            ds::commands::ds_toggle_trend_detail,
            ds::commands::ds_hide_trend_detail,
            ds::commands::ds_get_trend_detail_data,
            ds::commands::get_trend_detail_data,
            ds::commands::ds_broadcast_refresh,
            ds::commands::ds_open_top_up_window,
            // MiMo commands (mimo_ prefix)
            mimo::commands::mimo_refresh_data,
            mimo::commands::mimo_get_cached_data,
            mimo::commands::mimo_has_platform_session,
            mimo::commands::mimo_mark_onboarding_completed,
            mimo::commands::mimo_open_login_window,
            mimo::commands::mimo_clear_platform_session,
            mimo::commands::mimo_toggle_trend_detail,
            mimo::commands::mimo_hide_trend_detail,
            mimo::commands::mimo_get_trend_detail_data,
            mimo::commands::get_mimo_trend_detail_data,
            mimo::commands::mimo_broadcast_refresh,
            mimo::commands::broadcast_payment_mode,
            mimo::commands::do_refresh,
            mimo::commands::open_platform_login,
            mimo::commands::show_widget_window,
            mimo::commands::open_plan_manage,
            mimo::commands::open_balance_page,
            mimo::commands::open_top_up,
            // Shared commands
            tray::save_setting,
            tray::load_setting,
            tray::show_settings_window,
            tray::is_windows11,
            tray::hide_main_window,
            tray::show_main_window,
            tray::hide_settings_window,
            tray::recenter_settings,
            tray::hide_widget_window,
            tray::save_window_position,
            tray::load_window_position,
            tray::reposition_trend_detail,
            tray::broadcast_edge_snap_setting,
            tray::get_auto_start,
            tray::set_auto_start,
            tray::show_trend_detail_window,
            tray::is_window_docked,
            tray::snap_to_edge,
            tray::set_dragging,
            tray::get_monitor_top_y,
        ])
        .setup(move |app| {
            tray::setup(app)?;

            // 预热主窗口
            {
                if let Ok(window) = tauri::WebviewWindowBuilder::new(
                    app,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("ModelMeter")
                .inner_size(400.0, 780.0)
                .decorations(false)
                .transparent(true)
                .resizable(false)
                .visible(false)
                .skip_taskbar(true)
                .devtools(false)
                .center()
                .shadow(false)
                .build() {
                    crate::windows::register_rounded_region(&window);
                    // 启动时显示主面板（同时注册位置保存与可见性跟踪）
                    crate::windows::init_main_panel(app.handle(), &window);
                }
            }

            // 预热趋势详情窗口
            crate::windows::init_trend_detail(&app.handle());

            // 预热小组件窗口
            crate::windows::init_widget(&app.handle());

            // 初始化屏幕数量记录
            crate::windows::init_monitor_count(&app.handle());

            // Win11 圆角由 CSS border-radius 处理（调 DWMWA_WINDOW_CORNER_PREFERENCE 会导致 DWM 为无边框透明窗口添加默认阴影）

            // 后台定时器：依次刷新 DeepSeek 和 MiMo
            let handle = app.handle().clone();
            let mut rx = refresh_rx.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let interval_secs = *rx.borrow();

                    // 检查是否有可见窗口
                    let visible_count = handle.state::<Arc<AtomicUsize>>().load(Ordering::Acquire);
                    if visible_count == 0 {
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                            changed = rx.changed() => {
                                if changed.is_err() { break; }
                                debug_log!("[refresh-timer] 刷新间隔已变更为 {} 秒", *rx.borrow());
                            }
                        }
                        continue;
                    }

                    debug_log!(
                        "[refresh-timer] 等待 {} 秒后刷新，可见窗口数: {}",
                        interval_secs, visible_count
                    );

                    let interrupted = tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(interval_secs)) => false,
                        changed = rx.changed() => {
                            if changed.is_err() { break; }
                            debug_log!("[refresh-timer] 刷新间隔已变更为 {} 秒，立即生效", *rx.borrow());
                            true
                        }
                    };

                    if interrupted { continue; }

                    let storage = handle.state::<Arc<Storage>>();

                    // 定期静默续期登录态（每 30 分钟一次），降低 401 概率
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if now_secs.saturating_sub(LAST_SILENT_REFRESH.load(Ordering::Acquire))
                        >= SILENT_REFRESH_INTERVAL_SECS
                    {
                        LAST_SILENT_REFRESH.store(now_secs, Ordering::Release);
                        debug_log!("[refresh-timer] 定期静默续期登录态");
                        let _ = ds::login::silent_refresh_session(&handle).await;
                        let _ = mimo::login::silent_refresh_session(&handle).await;
                    }

                    // DeepSeek 刷新
                    let ds_state = handle.state::<ds::api::ApiState>();
                    if !ds_state.is_refreshing.swap(true, Ordering::Acquire) {
                        let _ds_guard = ds::commands::RefreshGuard::new(&ds_state.is_refreshing);
                        debug_log!("[refresh-timer] 开始刷新 DeepSeek");
                        let ds_result = std::panic::AssertUnwindSafe(ds::commands::do_refresh(&storage, &ds_state, Some(&handle)))
                            .catch_unwind()
                            .await;
                        match ds_result {
                            Ok(Ok(data)) => {
                                debug_log!("[refresh-timer] DeepSeek 刷新成功");
                                let _ = handle.emit("ds-trigger-refresh", data);
                            }
                            Ok(Err(_e)) => { debug_log!("[refresh-timer] DeepSeek 刷新失败: {}", _e); }
                            Err(_) => { debug_log!("[refresh-timer] DeepSeek 刷新 panic"); }
                        }
                    }

                    // MiMo 刷新
                    let mimo_state = handle.state::<mimo::api::ApiState>();
                    if !mimo_state.is_refreshing.swap(true, Ordering::Acquire) {
                        let _mimo_guard = mimo::commands::RefreshGuard::new(&mimo_state.is_refreshing);
                        debug_log!("[refresh-timer] 开始刷新 MiMo");
                        let mimo_result = std::panic::AssertUnwindSafe(mimo::commands::do_refresh_inner(&storage, &mimo_state))
                            .catch_unwind()
                            .await;
                        match mimo_result {
                            Ok(Ok(data)) => {
                                debug_log!("[refresh-timer] MiMo 刷新成功");
                                let _ = handle.emit("mimo-trigger-refresh", data);
                            }
                            Ok(Err(_e)) => { debug_log!("[refresh-timer] MiMo 刷新失败: {}", _e); }
                            Err(_) => { debug_log!("[refresh-timer] MiMo 刷新 panic"); }
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
