use std::sync::Arc;
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindowBuilder};
use tauri::menu::MenuItemKind;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use serde_json;

static LAST_MONITOR_COUNT: AtomicUsize = AtomicUsize::new(0);
static MONITOR_CHANGING: AtomicBool = AtomicBool::new(false);

const INVALID_COORD_THRESHOLD: i32 = -10000;

#[cfg(target_os = "windows")]
pub fn register_rounded_region(win: &tauri::WebviewWindow) {
    apply_rounded_region(win);

    let window = win.clone();
    win.clone().on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            apply_rounded_region(&window);
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn register_rounded_region(_win: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
fn apply_rounded_region(win: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};

    let (Ok(hwnd), Ok(size)) = (win.hwnd(), win.outer_size()) else {
        return;
    };

    let width = size.width.min(i32::MAX as u32) as i32;
    let height = size.height.min(i32::MAX as u32) as i32;
    if width <= 0 || height <= 0 {
        return;
    }

    // The native region clips the WebView compositor itself, including pixels
    // outside the CSS surface that otherwise appear black on transparent windows.
    unsafe {
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, 24, 24);
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
}

fn is_valid_coord(v: i32) -> bool {
    v > INVALID_COORD_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_coord_rejects_sentinel_values() {
        assert!(is_valid_coord(0));
        assert!(is_valid_coord(-9999));
        assert!(!is_valid_coord(INVALID_COORD_THRESHOLD));
        assert!(!is_valid_coord(-10001));
        assert!(!is_valid_coord(i32::MIN));
    }
}

fn is_position_on_any_monitor(app: &AppHandle, pos: &PhysicalPosition<i32>) -> bool {
    if let Ok(monitors) = app.available_monitors() {
        for m in &monitors {
            let mx = m.position().x;
            let my = m.position().y;
            let mw = m.size().width as i32;
            let mh = m.size().height as i32;
            if pos.x >= mx - 100 && pos.x < mx + mw && pos.y >= my - 100 && pos.y < my + mh {
                return true;
            }
        }
    }
    false
}

fn get_monitor_top_for_point(app: &AppHandle, x: i32, y: i32) -> Option<i32> {
    if let Ok(monitors) = app.available_monitors() {
        for m in &monitors {
            let mx = m.position().x;
            let my = m.position().y;
            let mw = m.size().width as i32;
            let mh = m.size().height as i32;
            if x >= mx && x < mx + mw && y >= my && y < my + mh {
                return Some(my);
            }
        }
    }
    None
}

fn load_saved_pos(app: &AppHandle, win: &str) -> Option<PhysicalPosition<i32>> {
    let storage = app.try_state::<Arc<crate::Storage>>()?;
    let x = storage.load_setting(&format!("{}_x", win))?.parse().ok()?;
    let y = storage.load_setting(&format!("{}_y", win))?.parse().ok()?;
    if !is_valid_coord(x) || !is_valid_coord(y) {
        return None;
    }
    Some(PhysicalPosition::new(x, y))
}

pub fn save_window_pos(app: &AppHandle, label: &str) {
    if let (Some(win), Some(storage)) = (
        app.get_webview_window(label),
        app.try_state::<Arc<crate::Storage>>(),
    ) {
        if let Ok(pos) = win.outer_position() {
            if !is_valid_coord(pos.x) || !is_valid_coord(pos.y) {
                return;
            }
            storage.save_setting(&format!("{}_x", label), &pos.x.to_string());
            storage.save_setting(&format!("{}_y", label), &pos.y.to_string());
        }
    }
}

pub fn save_window_pos_raw(app: &AppHandle, label: &str, x: i32, y: i32) {
    if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
        storage.save_setting(&format!("{}_x", label), &x.to_string());
        storage.save_setting(&format!("{}_y", label), &y.to_string());
    }
}

// ── Window visibility tracking ──

pub fn windows_shown(app: &AppHandle, label: &str) {
    let set = app.state::<Arc<Mutex<HashSet<String>>>>();
    let mut guard = set.lock().unwrap_or_else(|e| e.into_inner());
    if guard.insert(label.to_string()) {
        let counter = app.state::<Arc<AtomicUsize>>();
        counter.fetch_add(1, Ordering::Release);
    }
    drop(guard);
    update_tray_menu_checks(app);
}

pub fn windows_hidden(app: &AppHandle, label: &str) {
    let set = app.state::<Arc<Mutex<HashSet<String>>>>();
    let mut guard = set.lock().unwrap_or_else(|e| e.into_inner());
    if guard.remove(label) {
        let counter = app.state::<Arc<AtomicUsize>>();
        counter.fetch_sub(1, Ordering::Release);
    }
    drop(guard);
    update_tray_menu_checks(app);
}

fn register_visibility_tracking(win: &tauri::WebviewWindow) {
    let handle = win.app_handle().clone();
    let label = win.label().to_string();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            windows_hidden(&handle, &label);
        }
    });
}

pub fn init_monitor_count(app: &AppHandle) {
    let count = app.available_monitors().map(|m| m.len()).unwrap_or(1);
    LAST_MONITOR_COUNT.store(count, Ordering::Release);
}

fn check_monitor_disconnect(app: &AppHandle) {
    let current_count = app.available_monitors().map(|m| m.len()).unwrap_or(1);
    let last_count = LAST_MONITOR_COUNT.swap(current_count, Ordering::AcqRel);

    if last_count > 0 && current_count < last_count {
        MONITOR_CHANGING.store(true, Ordering::Release);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let primary = app.primary_monitor().ok().flatten();
            for label in &["main", "widget", "settings"] {
                if let Some(win) = app.get_webview_window(label) {
                    if !win.is_visible().unwrap_or(false) {
                        continue;
                    }
                    let _ = win.emit("edge-snap-released", serde_json::json!({"label": label}));
                    if let Ok(pos) = win.outer_position() {
                        if is_valid_coord(pos.x) && is_valid_coord(pos.y)
                            && !is_position_on_any_monitor(&app, &pos) {
                            if let Some(ref m) = primary {
                                if let Ok(size) = win.outer_size() {
                                    let px = m.position().x + (m.size().width as i32 - size.width as i32) / 2;
                                    let py = m.position().y + (m.size().height as i32 - size.height as i32) / 2;
                                    let _ = win.set_position(PhysicalPosition::new(px, py));
                                    continue;
                                }
                            }
                            let _ = win.center();
                        }
                    }
                }
            }
            MONITOR_CHANGING.store(false, Ordering::Release);
        });
    }
}

fn register_position_saver(
    win: &tauri::WebviewWindow,
    storage: Arc<crate::Storage>,
    label: &str,
) {
    let inner_win = win.clone();
    let label = label.to_string();

    win.clone().on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Ok(pos) = inner_win.outer_position() {
                if !is_valid_coord(pos.x) || !is_valid_coord(pos.y) {
                    return;
                }
                storage.save_setting(&format!("{}_x", label), &pos.x.to_string());
                storage.save_setting(&format!("{}_y", label), &pos.y.to_string());
            }
        }
    });
}

const EDGE_SNAP_THRESHOLD: i32 = 10;
const DRAG_END_DELAY_MS: u64 = 200;

fn register_edge_snap(
    win: &tauri::WebviewWindow,
    storage: Arc<crate::Storage>,
    label: &str,
) {
    let label = label.to_string();
    let app_handle = win.app_handle().clone();
    let timer: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    win.clone().on_window_event(move |event| {
        if let tauri::WindowEvent::Moved(pos) = event {
            check_monitor_disconnect(&app_handle);

            if MONITOR_CHANGING.load(Ordering::Acquire) {
                return;
            }

            if label == "main" {
                if let Some(detail_win) = app_handle.get_webview_window("trend-detail") {
                    if detail_win.is_visible().unwrap_or(false) {
                        reposition_trend_detail(&app_handle, &detail_win);
                    }
                }
            }

            let app = app_handle.clone();
            let label = label.clone();
            let storage = storage.clone();
            let timer = timer.clone();
            let x = pos.x;
            let y = pos.y;

            {
                let mut guard = match timer.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(handle) = guard.take() {
                    handle.abort();
                }

                let handle = tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(DRAG_END_DELAY_MS)).await;

                    let enabled = storage.load_edge_snap_enabled();

                    if let Some(win) = app.get_webview_window(&label) {
                        // 优先使用 current_monitor() 让系统判断窗口所在显示器（正确处理多屏 DPI）
                        let monitor_top = win.current_monitor()
                            .ok()
                            .flatten()
                            .map(|m| m.position().y)
                            .or_else(|| get_monitor_top_for_point(&app, x, y));

                        if let Some(monitor_top) = monitor_top {
                            let rel_y = y - monitor_top;
                            crate::debug_log!(
                                "[edge-snap] label={} pos=({},{}) monitor_top={} rel_y={} enabled={}",
                                label, x, y, monitor_top, rel_y, enabled
                            );
                            if enabled && rel_y <= EDGE_SNAP_THRESHOLD {
                                let _ = win.set_position(PhysicalPosition::new(x, monitor_top));
                                let _ = win.emit("edge-snap-triggered", serde_json::json!({"label": label}));
                            } else if enabled {
                                let _ = win.emit("edge-snap-released", serde_json::json!({"label": label}));
                            }
                        } else if enabled {
                            crate::debug_log!(
                                "[edge-snap] 无法确定窗口所在显示器: label={} pos=({},{})",
                                label, x, y
                            );
                            let _ = win.emit("edge-snap-released", serde_json::json!({"label": label}));
                        }
                    }
                });

                *guard = Some(handle);
            }
        }
    });
}

static MAIN_EDGE_SNAP_REGISTERED: AtomicBool = AtomicBool::new(false);
static WIDGET_EDGE_SNAP_REGISTERED: AtomicBool = AtomicBool::new(false);

fn ensure_edge_snap(app: &AppHandle, label: &str, flag: &AtomicBool) {
    if flag.swap(true, Ordering::Acquire) {
        return;
    }
    if let (Some(win), Some(storage)) = (
        app.get_webview_window(label),
        app.try_state::<Arc<crate::Storage>>(),
    ) {
        register_edge_snap(&win, storage.inner().clone(), label);
    }
}

/// 显示窗口后检测是否已在贴边位置，如果是则 snap 到 monitor_top 并发射 edge-snap-triggered
fn trigger_edge_snap_if_docked(app: &AppHandle, win: &tauri::WebviewWindow) {
    if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
        if storage.load_edge_snap_enabled() {
            if let Ok(pos) = win.outer_position() {
                // 优先用 current_monitor() 判断窗口所在显示器（正确处理多屏 DPI）
                let monitor_top = win.current_monitor()
                    .ok()
                    .flatten()
                    .map(|m| m.position().y)
                    .or_else(|| get_monitor_top_for_point(app, pos.x, pos.y));
                if let Some(monitor_top) = monitor_top {
                    let rel_y = pos.y - monitor_top;
                    if rel_y <= EDGE_SNAP_THRESHOLD {
                        let win = win.clone();
                        let label = win.label().to_string();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(DRAG_END_DELAY_MS)).await;
                            // 延迟后重新获取当前显示器顶部（窗口可能已移动）
                            let mtop = win.current_monitor()
                                .ok()
                                .flatten()
                                .map(|m| m.position().y)
                                .or_else(|| get_monitor_top_for_point(&win.app_handle(), pos.x, pos.y));
                            if let Some(mtop) = mtop {
                                let _ = win.set_position(PhysicalPosition::new(pos.x, mtop));
                                let _ = win.emit("edge-snap-triggered", serde_json::json!({"label": label}));
                            }
                        });
                    }
                }
            }
        }
    }
}

macro_rules! build_window {
    ($app:expr, $label:expr, $url:expr, $title:expr, $w:expr, $h:expr, $skip_tsk:expr, $aot:expr) => {{
        let b = WebviewWindowBuilder::new($app, $label, $url)
            .title($title)
            .inner_size($w, $h)
            .min_inner_size(1.0, 1.0)
            .decorations(false)
            .transparent(true)
            .resizable(false)
            .devtools(false)
            .shadow(false);
        let b = if $aot { b.always_on_top(true) } else { b };
        if $skip_tsk { b.skip_taskbar(true) } else { b }
    }};
}

/// 启动时初始化主面板并显示：注册主窗口的位置保存与可见性跟踪，然后显示面板。
pub fn init_main_panel(app: &AppHandle, win: &tauri::WebviewWindow) {
    if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
        register_position_saver(win, storage.inner().clone(), "main");
    }
    register_visibility_tracking(win);
    show_panel(app);
}

pub fn show_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        ensure_edge_snap(app, "main", &MAIN_EDGE_SNAP_REGISTERED);
        let mut was_docked = false;
        if let Some(pos) = load_saved_pos(app, "main") {
            if !is_position_on_any_monitor(app, &pos) {
                let _ = win.center();
            } else {
                let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
                if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
                    // 优先用 current_monitor() 判断窗口所在显示器（正确处理多屏 DPI）
                    let monitor_top = win.current_monitor()
                        .ok()
                        .flatten()
                        .map(|m| m.position().y)
                        .or_else(|| get_monitor_top_for_point(app, pos.x, pos.y));
                    if let Some(monitor_top) = monitor_top {
                        let rel_y = pos.y - monitor_top;
                        if storage.load_edge_snap_enabled() && rel_y <= EDGE_SNAP_THRESHOLD {
                            let _ = win.set_position(PhysicalPosition::new(pos.x, monitor_top));
                            let _ = win.emit("edge-snap-triggered", serde_json::json!({"label": "main"}));
                            was_docked = true;
                            let win_clone = win.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                let _ = win_clone.show();
                                let _ = win_clone.set_focus();
                                let _ = win_clone.set_always_on_top(true);
                            });
                        }
                    }
                }
            }
        }
        if !was_docked {
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.set_always_on_top(true);
            trigger_edge_snap_if_docked(app, &win);
        }
        windows_shown(app, "main");
        update_tray_menu_checks(app);
        return;
    }

    let saved_pos = load_saved_pos(app, "main").filter(|p| is_position_on_any_monitor(app, p));
    let mut b = WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("面板")
        .inner_size(400.0, 780.0)
        .min_inner_size(1.0, 1.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .devtools(false)
        .shadow(false)
        .always_on_top(true);
    b = b.visible(false);
    if saved_pos.is_none() {
        b = b.center();
    }
    match b.build() {
        Ok(win) => {
            register_rounded_region(&win);
            if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
                register_position_saver(&win, storage.inner().clone(), "main");
                register_edge_snap(&win, storage.inner().clone(), "main");
                MAIN_EDGE_SNAP_REGISTERED.store(true, Ordering::Release);
            }
            register_visibility_tracking(&win);
            if let Some(pos) = saved_pos {
                let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
            }
            let _ = win.show();
            let _ = win.set_focus();
            windows_shown(app, "main");
            update_tray_menu_checks(app);
        }
        Err(_) => {}
    }
}

pub fn close_panel(app: &AppHandle) {
    save_window_pos(app, "main");
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    windows_hidden(app, "main");
    hide_trend_detail(app);
    update_tray_menu_checks(app);
}

pub fn toggle_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            save_window_pos(app, "main");
            let _ = win.hide();
            windows_hidden(app, "main");
            update_tray_menu_checks(app);
        } else {
            show_panel(app);
        }
    } else {
        show_panel(app);
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.center();
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.set_always_on_top(true);
        windows_shown(app, "settings");
        return;
    }

    let mut b = build_window!(app, "settings", tauri::WebviewUrl::App("/#/settings".into()), "设置", 400.0, 600.0, false, false);
    b = b.visible(false).center();
    match b.build() {
        Ok(win) => {
            register_visibility_tracking(&win);
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.set_always_on_top(true);
            windows_shown(app, "settings");
        }
        Err(_) => {}
    }
}

pub fn hide_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.hide();
    }
    windows_hidden(app, "settings");
}

pub fn toggle_widget(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("widget") {
        if win.is_visible().unwrap_or(false) {
            save_window_pos(app, "widget");
            let _ = win.hide();
            windows_hidden(app, "widget");
        } else {
            ensure_edge_snap(app, "widget", &WIDGET_EDGE_SNAP_REGISTERED);
            let mut was_docked = false;
            if let Some(pos) = load_saved_pos(app, "widget") {
                if !is_position_on_any_monitor(app, &pos) {
                    let _ = win.center();
                } else {
                    let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
                    if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
                        // 优先用 current_monitor() 判断窗口所在显示器（正确处理多屏 DPI）
                        let monitor_top = win.current_monitor()
                            .ok()
                            .flatten()
                            .map(|m| m.position().y)
                            .or_else(|| get_monitor_top_for_point(app, pos.x, pos.y));
                        if let Some(monitor_top) = monitor_top {
                            let rel_y = pos.y - monitor_top;
                            if storage.load_edge_snap_enabled() && rel_y <= EDGE_SNAP_THRESHOLD {
                                let _ = win.set_position(PhysicalPosition::new(pos.x, monitor_top));
                                let _ = win.emit("edge-snap-triggered", serde_json::json!({"label": "widget"}));
                                was_docked = true;
                                let win_clone = win.clone();
                                tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                    let _ = win_clone.show();
                                    let _ = win_clone.set_focus();
                                    let _ = win_clone.set_always_on_top(true);
                                });
                            }
                        }
                    }
                }
            }
            emit_cached_snapshots(app);
            if !was_docked {
                let _ = win.show();
                let _ = win.set_focus();
                trigger_edge_snap_if_docked(app, &win);
            }
            windows_shown(app, "widget");

            // 数据由 WidgetView 的统一刷新入口读取；窗口显示本身不再发起重复网络请求。
        }
        update_tray_menu_checks(app);
        return;
    }

    let saved_pos = load_saved_pos(app, "widget").filter(|p| is_position_on_any_monitor(app, p));
    let mut b = build_window!(app, "widget", tauri::WebviewUrl::App("/#/widget".into()), "小部件", 400.0, 470.0, true, true);
    b = b.visible(false);
    if saved_pos.is_none() {
        b = b.center();
    }
    match b.build() {
        Ok(win) => {
            if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
                register_position_saver(&win, storage.inner().clone(), "widget");
                register_edge_snap(&win, storage.inner().clone(), "widget");
                WIDGET_EDGE_SNAP_REGISTERED.store(true, Ordering::Release);
            }
            register_visibility_tracking(&win);
            if let Some(pos) = saved_pos {
                let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
            }
            emit_cached_snapshots(app);
            let _ = win.show();
            let _ = win.set_focus();
            windows_shown(app, "widget");
            update_tray_menu_checks(app);
        }
        Err(_) => {}
    }
}

/// Push the last successful provider snapshots when the widget is shown.
fn emit_cached_snapshots(app: &AppHandle) {
    if let Some(state) = app.try_state::<crate::ds::api::ApiState>() {
        if let Some(data) = state.cached_data() {
            let _ = app.emit("ds-trigger-refresh", data);
        }
    }
    if let Some(state) = app.try_state::<crate::mimo::api::ApiState>() {
        if let Some(data) = state.cached_data() {
            let _ = app.emit("mimo-trigger-refresh", data);
        }
    }
}

pub fn init_widget(app: &AppHandle) {
    if app.get_webview_window("widget").is_some() {
        return;
    }
    let saved_pos = load_saved_pos(app, "widget").filter(|p| is_position_on_any_monitor(app, p));
    let mut b = build_window!(app, "widget", tauri::WebviewUrl::App("/#/widget".into()), "小部件", 400.0, 470.0, true, true);
    b = b.visible(false);
    if saved_pos.is_none() {
        b = b.center();
    }
    match b.build() {
        Ok(win) => {
            if let Some(storage) = app.try_state::<Arc<crate::Storage>>() {
                register_position_saver(&win, storage.inner().clone(), "widget");
                register_edge_snap(&win, storage.inner().clone(), "widget");
                WIDGET_EDGE_SNAP_REGISTERED.store(true, Ordering::Release);
            }
            register_visibility_tracking(&win);
            if let Some(pos) = saved_pos {
                let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
            }
        }
        Err(_) => {}
    }
}

// ---- Trend detail slide-out window ----

const DETAIL_W: f64 = 400.0;
const DETAIL_H: f64 = 200.0;
const GAP: i32 = 16;

pub async fn show_trend_detail(
    app: &AppHandle,
    date: &str,
    cache_hit: i64,
    cache_miss: i64,
    output: i64,
    cache_hit_rate: &str,
    cost: i64,
    audio_duration: i64,
) {
    // Ensure window exists, then show/update
    init_trend_detail(app);
    let detail_win = match app.get_webview_window("trend-detail") {
        Some(win) => win,
        None => return,
    };

    position_detail(app, &detail_win);
    let _ = detail_win.show();
    let _ = detail_win.set_always_on_top(true);
    let _ = detail_win.emit("trend-detail-data", serde_json::json!({
        "date": date,
        "cacheHit": cache_hit,
        "cacheMiss": cache_miss,
        "output": output,
        "cacheHitRate": cache_hit_rate,
        "cost": cost,
        "audioDuration": audio_duration,
    }));
}

fn position_detail(app: &AppHandle, detail_win: &tauri::WebviewWindow) {
    if let Some(main_win) = app.get_webview_window("main") {
        if let (Ok(main_pos), Ok(main_size)) = (main_win.outer_position(), main_win.outer_size()) {
            let main_x = main_pos.x;
            let main_y = main_pos.y;
            let main_w = main_size.width as i32;
            let main_h = main_size.height as i32;

            if let Ok(detail_size) = detail_win.outer_size() {
                let detail_w = detail_size.width as i32;
                let detail_h = detail_size.height as i32;

                let mut side = "right";
                if let Ok(monitors) = main_win.available_monitors() {
                    for m in monitors {
                        let m_pos = m.position();
                        let m_w = m.size().width as i32;
                        let m_h = m.size().height as i32;
                        let m_x = m_pos.x;
                        let m_y = m_pos.y;
                        // 检查主窗口中心是否在此显示器上（X 和 Y 轴都检查）
                        let main_center_x = main_x + main_w / 2;
                        let main_center_y = main_y + main_h / 2;
                        if main_center_x >= m_x && main_center_x < m_x + m_w
                            && main_center_y >= m_y && main_center_y < m_y + m_h {
                            let space_right = (m_x + m_w) - (main_x + main_w);
                            let space_left = main_x - m_x;
                            if space_right >= detail_w + GAP {
                                side = "right";
                            } else if space_left >= detail_w + GAP {
                                side = "left";
                            } else if space_right > space_left {
                                side = "right";
                            } else {
                                side = "left";
                            }
                            break;
                        }
                    }
                }

                let detail_x = if side == "right" {
                    main_x + main_w + GAP
                } else {
                    main_x - detail_w - GAP
                };
                let detail_y = main_y + main_h - detail_h;

                let _ = detail_win.set_position(PhysicalPosition::new(detail_x, detail_y));
            }
        }
    }
}

pub fn init_trend_detail(app: &AppHandle) {
    if app.get_webview_window("trend-detail").is_some() {
        return;
    }
    match WebviewWindowBuilder::new(app, "trend-detail", tauri::WebviewUrl::App("/#/trend-detail".into()))
        .title("详情")
        .inner_size(DETAIL_W, DETAIL_H)
        .min_inner_size(1.0, 1.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .devtools(false)
        .visible(false)
        .shadow(false)
        .build()
    {
        Ok(_) => {}
        Err(_) => {}
    }
}

pub fn hide_trend_detail(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("trend-detail") {
        let _ = win.hide();
    }
}

pub fn reposition_trend_detail(app: &AppHandle, detail_win: &tauri::WebviewWindow) {
    position_detail(app, detail_win);
}

/// 更新托盘菜单中“显示面板”和“显示小组件”的勾选状态
pub fn update_tray_menu_checks(app: &AppHandle) {
    let menu_state = match app.try_state::<Arc<tauri::menu::Menu<tauri::Wry>>>() {
        Some(state) => state,
        None => return,
    };
    let menu: &tauri::menu::Menu<tauri::Wry> = &**menu_state.inner();

    let main_visible = app.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    let widget_visible = app.get_webview_window("widget")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);

    if let Some(MenuItemKind::Check(item)) = menu.get("show_panel") {
        let _ = item.set_checked(main_visible);
    }
    if let Some(MenuItemKind::Check(item)) = menu.get("toggle_widget") {
        let _ = item.set_checked(widget_visible);
    }
}

// ─── MiMo-specific window functions ────────────────────────────

pub fn open_top_up_browser() {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "https://aistudio.xiaomimimo.com/"])
        .spawn();
}

pub async fn open_plan_manage(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("plan-manage") {
        let _ = win.close();
    }
    WebviewWindowBuilder::new(
        &app,
        "plan-manage",
        tauri::WebviewUrl::External("https://platform.xiaomimimo.com/console/plan-manage".parse().unwrap()),
    )
    .title("MiMo 套餐管理")
    .inner_size(1180.0, 860.0)
    .center()
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn open_balance_page(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("balance-page") {
        let _ = win.close();
    }
    WebviewWindowBuilder::new(
        &app,
        "balance-page",
        tauri::WebviewUrl::External("https://platform.xiaomimimo.com/console/balance".parse().unwrap()),
    )
    .title("MiMo 余额管理")
    .inner_size(1180.0, 860.0)
    .center()
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_window_docked(window: &tauri::WebviewWindow) -> bool {
    window.is_visible().unwrap_or(false) && window.is_always_on_top().unwrap_or(false)
}

pub fn snap_to_edge(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
}

pub fn set_dragging(_label: &str, _dragging: bool, _app: &AppHandle) {
    crate::debug_log!("[set_dragging] label={}, dragging={}", _label, _dragging);
}

// ─── WebView2 Cookie 提取（含 HttpOnly）────────────────────

/// 从指定 WebView 窗口提取某域的全部 cookie（含 HttpOnly），返回 `name=value; ...` 串。
#[cfg(target_os = "windows")]
pub fn extract_webview_cookies(app: &AppHandle, win_label: &str, host: &str) -> Result<String, String> {
    use std::sync::mpsc;
    use webview2_com::GetCookiesCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows_core::{Interface, PWSTR};

    let win = app
        .get_webview_window(win_label)
        .ok_or_else(|| format!("{win_label} window not found"))?;

    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let tx_for_err = tx.clone();
    let host_owned = host.to_string();
    let result = win.with_webview(move |platform_webview| unsafe {
        let controller = platform_webview.controller();

        let webview = match controller.CoreWebView2() {
            Ok(wv) => wv,
            Err(e) => {
                let _ = tx.send(Err(format!("get webview failed: {}", e)));
                return;
            }
        };

        let webview2: ICoreWebView2_2 = match webview.cast() {
            Ok(wv2) => wv2,
            Err(e) => {
                let _ = tx.send(Err(format!("cast failed: {}", e)));
                return;
            }
        };

        let cookie_mgr = match webview2.CookieManager() {
            Ok(mgr) => mgr,
            Err(e) => {
                let _ = tx.send(Err(format!("CookieManager failed: {}", e)));
                return;
            }
        };

        let uri = windows_core::HSTRING::from(&host_owned);

        let handler = GetCookiesCompletedHandler::create(Box::new(
            move |error_code: windows_core::Result<()>,
                  cookie_list: Option<ICoreWebView2CookieList>|
                  -> windows_core::Result<()> {
                if let Err(e) = error_code {
                    let _ = tx.send(Err(format!("GetCookies callback error: {}", e)));
                    return Ok(());
                }
                let cookie_list = match cookie_list {
                    Some(list) => list,
                    None => {
                        let _ = tx.send(Err("cookie_list is null".to_string()));
                        return Ok(());
                    }
                };
                let mut count: u32 = 0;
                if let Err(e) = cookie_list.Count(&mut count) {
                    let _ = tx.send(Err(format!("Count failed: {}", e)));
                    return Ok(());
                }
                let mut parts = Vec::new();
                for i in 0..count {
                    let cookie = match cookie_list.GetValueAtIndex(i) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    let mut name_pwstr = PWSTR::null();
                    let mut value_pwstr = PWSTR::null();
                    if cookie.Name(&mut name_pwstr).is_err() {
                        continue;
                    }
                    if cookie.Value(&mut value_pwstr).is_err() {
                        windows::Win32::System::Com::CoTaskMemFree(
                            Some(name_pwstr.as_ptr() as *const _),
                        );
                        continue;
                    }
                    let name = name_pwstr.to_string().unwrap_or_default();
                    let value = value_pwstr.to_string().unwrap_or_default();
                    windows::Win32::System::Com::CoTaskMemFree(
                        Some(name_pwstr.as_ptr() as *const _),
                    );
                    windows::Win32::System::Com::CoTaskMemFree(
                        Some(value_pwstr.as_ptr() as *const _),
                    );
                    if !name.is_empty() {
                        let needs_quoting = value.contains('=')
                            || value.contains(',')
                            || value.contains(';')
                            || value.contains(' ')
                            || value.contains('"');
                        let formatted_value = if needs_quoting
                            && !value.starts_with('"')
                            && !value.ends_with('"')
                        {
                            format!("\"{}\"", value.replace('"', "\\\""))
                        } else {
                            value
                        };
                        parts.push(format!("{}={}", name, formatted_value));
                    }
                }
                let _ = tx.send(Ok(parts.join("; ")));
                Ok(())
            },
        ));

        if let Err(e) = cookie_mgr.GetCookies(&uri, &handler) {
            let _ = tx_for_err.send(Err(format!("GetCookies call failed: {}", e)));
        }
    });

    if let Err(e) = result {
        return Err(format!("with_webview failed: {}", e));
    }

    rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "cookie extraction timed out".to_string())?
}

#[cfg(not(target_os = "windows"))]
pub fn extract_webview_cookies(
    _app: &AppHandle,
    _win_label: &str,
    _host: &str,
) -> Result<String, String> {
    Err("cookie extraction only supported on Windows".to_string())
}
