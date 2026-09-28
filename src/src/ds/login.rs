use crate::storage::Storage;
use std::sync::Arc;
use tauri::{Emitter, Manager, WebviewWindowBuilder};

pub async fn start_login_flow(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("login") {
        let _ = win.close();
    }

    let api_state = app.state::<crate::ds::api::ApiState>();
    let already_logged_in = api_state
        .platform_token
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some();

    let app_for_poll = app.clone();
    let app_for_nav = app.clone();
    let app_for_watch = app.clone();

    let builder = WebviewWindowBuilder::new(
        &app,
        "login",
        tauri::WebviewUrl::External("https://platform.deepseek.com/usage".parse().unwrap()),
    )
    .title("DeepSeek Platform 登录")
    .inner_size(1180.0, 860.0)
    .center()
    .always_on_top(true);

    let builder = if !already_logged_in {
        builder.initialization_script("localStorage.removeItem('userToken');")
    } else {
        builder
    };

    let login_window = builder
        .on_navigation(move |url| {
            let url_str = url.to_string();
            if url_str.starts_with("https://dsm.local/token") {
                let mut token = String::new();

                if let Some(query) = url_str.split('?').nth(1) {
                    for pair in query.split('&') {
                        let mut parts = pair.splitn(2, '=');
                        let key = parts.next().unwrap_or("");
                        let val = parts.next().unwrap_or("");
                        if key == "t" {
                            token = urlencoding::decode(val).unwrap_or_default().into_owned();
                        }
                    }
                }

                if !token.is_empty() {
                    let api_state = app_for_nav.state::<crate::ds::api::ApiState>();
                    *api_state.platform_token.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(token.clone());

                    let storage = app_for_nav.state::<Arc<Storage>>();
                    storage.save_platform_token(&token).ok();
                    storage.save_onboarding_completed();

                    // 用 CookieManager 提取完整 cookie（含 HttpOnly），避免 document.cookie 缺失
                    let app_for_cookie = app_for_nav.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Ok(cookies) = crate::windows::extract_webview_cookies(
                            &app_for_cookie,
                            "login",
                            "https://platform.deepseek.com",
                        ) {
                            let api_state = app_for_cookie.state::<crate::ds::api::ApiState>();
                            *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(cookies.clone());
                            let storage = app_for_cookie.state::<Arc<Storage>>();
                            storage.save_platform_cookies(&cookies).ok();
                        }
                        if let Some(win) = app_for_cookie.get_webview_window("login") {
                            let _ = win.close();
                        }
                        let _ = app_for_cookie.emit("ds-login-complete", ());
                    });
                }

                false
            } else {
                true
            }
        })
        .build()
        .map_err(|e| e.to_string())?;

    // Inject JS polling script after 3 seconds
    let inject_js = r#"
        (function() {
            var checkInterval = setInterval(function() {
                var ut = localStorage.getItem('userToken');
                if (ut) {
                    try {
                        var o = JSON.parse(ut);
                        if (o && o.value) {
                            clearInterval(checkInterval);
                            var token = encodeURIComponent(o.value);
                            window.location.href = 'https://dsm.local/token?t=' + token;
                        }
                    } catch(e) {}
                }
            }, 2000);
        })();
    "#;

    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        if let Some(win) = app_for_poll.get_webview_window("login") {
            let _ = win.eval(inject_js);
        }
    });

    // Detect login window close via event (replaces polling)
    login_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let api_state = app_for_watch.state::<crate::ds::api::ApiState>();
            if !api_state.has_platform_session() {
                let _ = app_for_watch.emit("ds-login-cancelled", ());
            }
        }
    });

    Ok(())
}

/// 静默重抓用的注入脚本：轮询 localStorage.userToken，通过 document.title 传递给后端。
const DS_SILENT_POLL_JS: &str = r#"
    (function() {
        var iv = setInterval(function() {
            var ut = localStorage.getItem('userToken');
            if (ut) {
                try {
                    var o = JSON.parse(ut);
                    if (o && o.value) {
                        clearInterval(iv);
                        document.title = 'DSM_SILENT:' + o.value;
                    }
                } catch(e) {}
            }
        }, 1000);
    })();
"#;

/// 静默重抓 DeepSeek 登录态（隐藏窗口，不打扰用户）。成功返回 true 并更新状态与存储。
pub async fn silent_refresh_session(app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        use tauri::WebviewUrl;

        if let Some(win) = app.get_webview_window("ds-silent") {
            let _ = win.close();
        }

        let win = match WebviewWindowBuilder::new(
            app,
            "ds-silent",
            WebviewUrl::External("https://platform.deepseek.com/usage".parse().unwrap()),
        )
        .title("ds silent")
        .inner_size(480.0, 720.0)
        .visible(false)
        .build()
        {
            Ok(w) => w,
            Err(_) => return false,
        };

        let app_for_poll = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if let Some(win) = app_for_poll.get_webview_window("ds-silent") {
                let _ = win.eval(DS_SILENT_POLL_JS);
            }
        });

        // 轮询窗口标题提取 token，并提取完整 cookie（含 HttpOnly）
        for _ in 0..25 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let title = win.title().unwrap_or_default();
            let Some(rest) = title.strip_prefix("DSM_SILENT:") else {
                continue;
            };
            let token = rest.to_string();
            if token.is_empty() {
                continue;
            }
            let cookies = crate::windows::extract_webview_cookies(
                app,
                "ds-silent",
                "https://platform.deepseek.com",
            )
            .ok()
            .filter(|c| !c.is_empty());

            let api_state = app.state::<crate::ds::api::ApiState>();
            *api_state
                .platform_token
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(token.clone());
            if let Some(ref c) = cookies {
                *api_state
                    .platform_cookies
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(c.clone());
            }
            let storage = app.state::<Arc<Storage>>();
            storage.save_platform_token(&token).ok();
            if let Some(ref c) = cookies {
                storage.save_platform_cookies(c).ok();
            }
            storage.save_onboarding_completed();
            let _ = win.close();
            return true;
        }
        let _ = win.close();
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
