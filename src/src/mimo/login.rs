use crate::storage::Storage;
use std::sync::Arc;
use tauri::{Emitter, Manager, WebviewWindowBuilder};

pub async fn start_login_flow(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("login") {
        let _ = win.close();
    }

    let app_for_watch = app.clone();
    let app_for_poll = app.clone();

    let login_window = WebviewWindowBuilder::new(
        &app,
        "login",
        tauri::WebviewUrl::External("https://platform.xiaomimimo.com/".parse().unwrap()),
    )
    .title("MiMo Platform 登录")
    .inner_size(1180.0, 860.0)
    .center()
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        for _ in 0..120 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let win = match app_for_poll.get_webview_window("login") {
                Some(w) => w,
                None => return,
            };

            if !win.is_visible().unwrap_or(false) {
                return;
            }

            let cookies = match crate::windows::extract_webview_cookies(
                &app_for_poll,
                "login",
                "https://platform.xiaomimimo.com",
            ) {
                Ok(c) if !c.is_empty() => c,
                _ => continue,
            };

            let api_state = app_for_poll.state::<crate::mimo::api::ApiState>();
            {
                let mut guard = api_state
                    .platform_cookies
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                *guard = Some(cookies.clone());
            }

            let balance_res = crate::mimo::api::fetch_balance(Some(&cookies)).await;
            match balance_res {
                Ok(_) => {
                    let storage = app_for_poll.state::<Arc<Storage>>();
                    storage.save_mimo_platform_cookies(&cookies).ok();
                    storage.save_onboarding_completed();

                    if let Some(win) = app_for_poll.get_webview_window("login") {
                        let _ = win.close();
                    }
                    let _ = app_for_poll.emit("mimo-login-complete", ());
                    return;
                }
                Err(ref e) if e.is_auth_error() => {
                    let mut guard = api_state
                        .platform_cookies
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    *guard = None;
                    continue;
                }
                Err(_) => {
                    continue;
                }
            }
        }
    });

    login_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let api_state = app_for_watch.state::<crate::mimo::api::ApiState>();
            if !api_state.has_platform_session() {
                let _ = app_for_watch.emit("mimo-login-cancelled", ());
            }
        }
    });

    Ok(())
}



/// 清理 WebView2 存储的所有 cookie
/// WebView2 所有窗口共享同一个用户数据目录, 从任意存在的 webview 获取 CookieManager 即可
/// 返回是否存在可用的 webview (用于前端提示)
#[cfg(target_os = "windows")]
pub fn clear_webview_cookies(app: &tauri::AppHandle) -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows_core::Interface;

    // 优先用 login 窗口, 其次 main 窗口
    let win = app
        .get_webview_window("login")
        .or_else(|| app.get_webview_window("main"));

    let Some(win) = win else {
        crate::debug_log!("[clear_webview_cookies] 没有可用的 webview 窗口");
        return false;
    };

    let result = win.with_webview(move |platform_webview| unsafe {
        let controller = platform_webview.controller();
        if let Ok(webview) = controller.CoreWebView2() {
            if let Ok(webview2) = webview.cast::<ICoreWebView2_2>() {
                if let Ok(cookie_mgr) = webview2.CookieManager() {
                    if let Err(_e) = cookie_mgr.DeleteAllCookies() {
                        crate::debug_log!("[clear_webview_cookies] DeleteAllCookies 失败: {}", _e);
                    } else {
                        crate::debug_log!("[clear_webview_cookies] DeleteAllCookies 成功");
                    }
                }
            }
        }
    });

    if let Err(_e) = result {
        crate::debug_log!("[clear_webview_cookies] with_webview 失败: {}", _e);
        return false;
    }

    true
}

#[cfg(not(target_os = "windows"))]
pub fn clear_webview_cookies(_app: &tauri::AppHandle) -> bool {
    false
}

/// 静默重抓 MiMo 登录态（隐藏窗口，不打扰用户）。成功返回 true 并更新状态与存储。
pub async fn silent_refresh_session(app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        use tauri::WebviewUrl;

        if let Some(win) = app.get_webview_window("mimo-silent") {
            let _ = win.close();
        }

        let win = match WebviewWindowBuilder::new(
            app,
            "mimo-silent",
            WebviewUrl::External(
                "https://platform.xiaomimimo.com/console/balance"
                    .parse()
                    .unwrap(),
            ),
        )
        .title("mimo silent")
        .inner_size(480.0, 720.0)
        .visible(false)
        .build()
        {
            Ok(w) => w,
            Err(_) => return false,
        };

        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let cookies = match crate::windows::extract_webview_cookies(
                app,
                "mimo-silent",
                "https://platform.xiaomimimo.com",
            ) {
                Ok(c) if !c.is_empty() => c,
                _ => continue,
            };
            if crate::mimo::api::fetch_balance(Some(cookies.as_str())).await.is_ok() {
                let api_state = app.state::<crate::mimo::api::ApiState>();
                *api_state
                    .platform_cookies
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(cookies.clone());
                let storage = app.state::<Arc<Storage>>();
                storage.save_mimo_platform_cookies(&cookies).ok();
                storage.save_onboarding_completed();
                let _ = win.close();
                return true;
            }
        }
        let _ = win.close();
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
