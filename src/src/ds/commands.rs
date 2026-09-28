use crate::ds::api::{self, ApiState};
use crate::ds::models::*;
use crate::storage::Storage;
use chrono::{Datelike, Local, Timelike};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// RAII guard that resets is_refreshing on Drop — ensures panic safety.
pub(crate) struct RefreshGuard<'a> {
    flag: &'a AtomicBool,
}
impl<'a> RefreshGuard<'a> {
    pub fn new(flag: &'a AtomicBool) -> Self {
        Self { flag }
    }
}
impl Drop for RefreshGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

/// Returns "today" date string adjusted for 8 AM cutoff.
/// Before 8 AM, data still belongs to previous day.
fn effective_today(now: &chrono::DateTime<Local>) -> String {
    let hour = now.hour();
    let date = if hour < 8 {
        now.date_naive().pred_opt().unwrap_or(now.date_naive())
    } else {
        now.date_naive()
    };
    date.format("%Y-%m-%d").to_string()
}
use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn ds_refresh_data(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<DashboardData, String> {
    if api_state.is_refreshing.swap(true, Ordering::Acquire) {
        return api_state.cached_data().ok_or_else(|| "refresh_in_progress".into());
    }
    let _guard = RefreshGuard::new(&api_state.is_refreshing);
    let result = do_refresh(&storage, &api_state, Some(&app)).await;
    if let Ok(ref data) = result {
        api_state.cache_data(data);
    }
    result
}

#[tauri::command]
pub fn ds_get_cached_data(
    api_state: tauri::State<'_, ApiState>,
) -> Option<DashboardData> {
    api_state.cached_data()
}

#[tauri::command]
pub fn ds_has_platform_session(api_state: tauri::State<'_, ApiState>) -> bool {
    api_state.has_platform_session()
}

pub(crate) async fn do_refresh(
    storage: &Storage,
    api_state: &ApiState,
    app: Option<&tauri::AppHandle>,
) -> Result<DashboardData, String> {
    let (data, unauth) = ds_refresh_once(storage, api_state).await?;
    if unauth {
        if let Some(app) = app {
            crate::debug_log!("[ds-refresh] 检测到登录失效，尝试静默重抓");
            if crate::ds::login::silent_refresh_session(app).await {
                crate::debug_log!("[ds-refresh] 静默重抓成功，重新刷新");
                let (data2, _) = ds_refresh_once(storage, api_state).await?;
                return Ok(data2);
            }
            crate::debug_log!("[ds-refresh] 静默重抓失败，保持失效状态");
        }
    }
    Ok(data)
}

async fn ds_refresh_once(storage: &Storage, api_state: &ApiState) -> Result<(DashboardData, bool), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let platform_token = api_state.platform_token.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let platform_cookies = api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()).clone();

    let mut data = DashboardData {
        is_account_available: false,
        total_balance: 0.0,
        granted_balance: 0.0,
        topped_up_balance: 0.0,
        balance_info: None,
        flash_usage: None,
        v4flash_usage: None,
        flash_daily_usage: vec![],
        v4flash_daily_usage: vec![],
        current_day_cost: 0.0,
        current_month_cost: 0.0,
        current_day_requests: 0,
        current_day_flash_tokens: 0,
        current_day_v4flash_tokens: 0,
        has_platform_session: platform_token.is_some(),
        is_first_launch: storage.is_first_launch(),
        last_updated: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        error_message: None,
        warning_message: None,
    };

    let mut errors: Vec<String> = Vec::new();
    let mut unauth = false;

    // Fetch balance via platform user_summary API (replaces api_key + fetch_balance)
    let mut summary_unauth = false;
    if let Some(ref token) = platform_token {
        match api::fetch_user_summary(&client, token, platform_cookies.as_deref()).await {
            Ok(summary) => {
                if let Some(biz_data) = summary.data.and_then(|d| d.biz_data) {
                    data.is_account_available = true;
                    let normal_balance = biz_data.normal_wallets.first()
                        .map(|w| w.balance.as_str()).unwrap_or("0");
                    let bonus_balance = biz_data.bonus_wallets.first()
                        .map(|w| w.balance.as_str()).unwrap_or("0");
                    let normal: f64 = normal_balance.parse().unwrap_or(0.0);
                    let bonus: f64 = bonus_balance.parse().unwrap_or(0.0);
                    data.topped_up_balance = normal;
                    data.granted_balance = bonus;
                    data.total_balance = normal + bonus;
                    data.balance_info = Some(BalanceInfo {
                        currency: "CNY".to_string(),
                        total_balance: format!("{:.2}", data.total_balance),
                        granted_balance: format!("{:.2}", data.granted_balance),
                        topped_up_balance: format!("{:.2}", data.topped_up_balance),
                    });
                    if data.total_balance > 0.0 && data.total_balance <= 1.0 {
                        data.warning_message = Some("余额不足 1 元，请及时充值！".to_string());
                    }
                }
            }
            Err(ref e) => {
                crate::debug_log!("[do_refresh] fetch_user_summary 失败: {}", e);
                let msg = e.to_string();
                if !errors.contains(&msg) {
                    errors.push(msg);
                }
                if matches!(e, api::ApiError::PlatformUnauthorized) {
                    summary_unauth = true;
                    unauth = true;
                }
            }
        }
    }

    // Try platform APIs for usage data (skip if summary already detected token expiry)
    if let Some(ref token) = platform_token {
        if summary_unauth {
            let platform_unauth_msg = api::ApiError::PlatformUnauthorized.to_string();
            storage.clear_platform_token();
            storage.clear_platform_cookies();
            *api_state.platform_token.lock().unwrap_or_else(|e| e.into_inner()) = None;
            *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
            data.has_platform_session = false;
            if !errors.contains(&platform_unauth_msg) {
                errors.push(platform_unauth_msg);
            }
        } else {
            let now = Local::now();
            let (cost_res, amount_res) = tokio::join!(
                api::fetch_platform_cost(&client, token, platform_cookies.as_deref(), now.year(), now.month()),
                api::fetch_platform_amount(&client, token, platform_cookies.as_deref(), now.year(), now.month()),
            );
            if cost_res.is_err() {
                crate::debug_log!("[do_refresh] fetch_platform_cost 失败");
            }
            if amount_res.is_err() {
                crate::debug_log!("[do_refresh] fetch_platform_amount 失败");
            }
            if let Ok(ref amount) = amount_res {
                data = apply_platform_amount(data, amount, &now);
            }
            if let Ok(ref cost) = cost_res {
                data = apply_platform_cost(data, cost, &now);
            }
            let platform_unauth_msg = api::ApiError::PlatformUnauthorized.to_string();
            let has_platform_unauth = matches!(&cost_res, Err(api::ApiError::PlatformUnauthorized))
                || matches!(&amount_res, Err(api::ApiError::PlatformUnauthorized));
            if has_platform_unauth {
                unauth = true;
                storage.clear_platform_token();
                storage.clear_platform_cookies();
                *api_state.platform_token.lock().unwrap_or_else(|e| e.into_inner()) = None;
                *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
                data.has_platform_session = false;
                if !errors.contains(&platform_unauth_msg) {
                    errors.push(platform_unauth_msg);
                }
            }
        }
    }

    let unauth_msg = api::ApiError::PlatformUnauthorized.to_string();
    if errors.iter().any(|e| *e == unauth_msg) {
        errors.retain(|e| *e == unauth_msg);
    }

    if !errors.is_empty() {
        data.error_message = Some(errors.join("\n"));
    }

    // Extract today's per-model tokens from daily usage (works with both platform & legacy data)
    let today_str = effective_today(&Local::now());
    if data.current_day_v4flash_tokens == 0 {
        if let Some(p) = data.v4flash_daily_usage.iter().find(|p| p.date == today_str) {
            data.current_day_v4flash_tokens = p.total_tokens as i64;
        }
    }
    if data.current_day_flash_tokens == 0 {
        if let Some(p) = data.flash_daily_usage.iter().find(|p| p.date == today_str) {
            data.current_day_flash_tokens = p.total_tokens as i64;
        }
    }

    api_state.cache_data(&data);
    Ok((data, unauth))
}

fn apply_platform_amount(
    mut data: DashboardData,
    amount: &PlatformAmountResponse,
    now: &chrono::DateTime<Local>,
) -> DashboardData {
    let today_str = effective_today(now);

    if let Some(ref inner) = amount.data {
        if let Some(ref biz_data) = inner.biz_data {
            // Monthly model summaries (from biz_data.total)，未知模型不进卡片
            let mut month_flash_tokens: i64 = 0;
            let mut month_v4flash_tokens: i64 = 0;
            for total_entry in &biz_data.total {
                let model_total: i64 = total_entry.usage.iter()
                    .filter(|u| u.usage_type != "REQUEST")
                    .filter_map(|u| u.amount.parse::<i64>().ok())
                    .sum();
                match api::classify_model(&total_entry.model) {
                    Some(DeepSeekModel::Flash) => month_flash_tokens += model_total,
                    Some(DeepSeekModel::V4Flash) => month_v4flash_tokens += model_total,
                    None => {}
                }
            }

            // Set model summaries (cost_in_cents via cost API separately)
            if month_flash_tokens > 0 {
                data.flash_usage = Some(ModelUsageSummary {
                    model: DeepSeekModel::Flash,
                    total_tokens: month_flash_tokens as i32,
                    cost_in_cents: 0,
                    total_tokens_formatted: api::format_number(month_flash_tokens as i32),
                    cost_formatted: "¥0.00".into(),
                });
            }
            if month_v4flash_tokens > 0 {
                data.v4flash_usage = Some(ModelUsageSummary {
                    model: DeepSeekModel::V4Flash,
                    total_tokens: month_v4flash_tokens as i32,
                    cost_in_cents: 0,
                    total_tokens_formatted: api::format_number(month_v4flash_tokens as i32),
                    cost_formatted: "¥0.00".into(),
                });
            }

            // Today's per-model tokens + request count (from biz_data.days)
            for day in &biz_data.days {
                if day.date == today_str {
                    for model_data in &day.data {
                        let model_tokens: i64 = model_data.usage.iter()
                            .filter(|u| u.usage_type != "REQUEST")
                            .filter_map(|u| u.amount.parse::<i64>().ok())
                            .sum();
                        match api::classify_model(&model_data.model) {
                            Some(DeepSeekModel::Flash) => data.current_day_flash_tokens = model_tokens,
                            Some(DeepSeekModel::V4Flash) => data.current_day_v4flash_tokens = model_tokens,
                            None => {}
                        }
                    }
                    data.current_day_requests = day.data.iter()
                        .flat_map(|m| &m.usage)
                        .filter(|u| u.usage_type == "REQUEST")
                        .filter_map(|u| u.amount.parse::<i32>().ok())
                        .sum();
                    break;
                }
            }
            // Populate daily usage arrays
            data.flash_daily_usage = api::build_daily_from_platform(&biz_data.days, DeepSeekModel::Flash);
            data.v4flash_daily_usage = api::build_daily_from_platform(&biz_data.days, DeepSeekModel::V4Flash);
        }
    }

    data
}

fn apply_platform_cost(
    mut data: DashboardData,
    cost: &PlatformCostResponse,
    now: &chrono::DateTime<Local>,
) -> DashboardData {
    let today_str = effective_today(now);
    let current_year = now.year();
    let current_month = now.month();

    let mut flash_cost_map: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    let mut v4flash_cost_map: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    if let Some(ref inner) = cost.data {
        if let Some(ref biz_data_list) = inner.biz_data {
            for biz_data in biz_data_list {
                for day in &biz_data.days {
                    let day_total = sum_day_cost(&day.data);
                    if day.date == today_str {
                        data.current_day_cost = day_total;
                    }
                    if let Ok(d) = chrono::NaiveDate::parse_from_str(&day.date, "%Y-%m-%d") {
                        if d.year() == current_year && d.month() == current_month {
                            data.current_month_cost += day_total;
                        }
                    }

                    // Extract per-model daily cost
                    if let Some(flash_model) = api::find_model(&day.data, DeepSeekModel::Flash) {
                        let cost_cents = (sum_model_cost(flash_model) * 100.0).round() as i32;
                        flash_cost_map.insert(day.date.clone(), cost_cents);
                    }
                    if let Some(v4flash_model) = api::find_model(&day.data, DeepSeekModel::V4Flash) {
                        let cost_cents = (sum_model_cost(v4flash_model) * 100.0).round() as i32;
                        v4flash_cost_map.insert(day.date.clone(), cost_cents);
                    }
                }
            }
        }
    }

    // Enrich daily usage vectors with cost data
    for point in &mut data.flash_daily_usage {
        if let Some(&cents) = flash_cost_map.get(&point.date) {
            point.cost_in_cents = cents;
        }
    }
    for point in &mut data.v4flash_daily_usage {
        if let Some(&cents) = v4flash_cost_map.get(&point.date) {
            point.cost_in_cents = cents;
        }
    }

    data
}

fn sum_day_cost(model_data: &[PlatformModelData]) -> f64 {
    model_data
        .iter()
        .flat_map(|m| &m.usage)
        .filter_map(|u| u.amount.parse::<f64>().ok())
        .sum::<f64>()
}

fn sum_model_cost(model_data: &PlatformModelData) -> f64 {
    model_data
        .usage
        .iter()
        .filter_map(|u| u.amount.parse::<f64>().ok())
        .sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local_dt(y: i32, m: u32, d: u32, h: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn effective_today_cutoff_at_8am() {
        // 8 点前仍算前一天，8 点起算当天
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 7)), "2026-09-09");
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 8)), "2026-09-10");
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 23)), "2026-09-10");
    }

    #[test]
    fn effective_today_handles_month_boundary() {
        assert_eq!(effective_today(&local_dt(2026, 9, 1, 3)), "2026-08-31");
    }

    fn entry(kind: &str, amount: &str) -> PlatformUsageEntry {
        PlatformUsageEntry { usage_type: kind.to_string(), amount: amount.to_string() }
    }

    #[test]
    fn sum_costs_skip_unparsable_amounts() {
        let models = vec![
            PlatformModelData {
                model: "deepseek-v4-flash".to_string(),
                usage: vec![entry("PROMPT_TOKEN", "0.5"), entry("REQUEST", "not-a-number")],
            },
            PlatformModelData {
                model: "deepseek-flash".to_string(),
                usage: vec![entry("RESPONSE_TOKEN", "1.25")],
            },
        ];
        assert!((sum_day_cost(&models) - 1.75).abs() < 1e-9);
        assert!((sum_model_cost(&models[0]) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn sum_model_cost_empty_is_zero() {
        let empty = PlatformModelData { model: "x".to_string(), usage: vec![] };
        assert_eq!(sum_model_cost(&empty), 0.0);
    }
}

#[tauri::command]
pub fn ds_mark_onboarding_completed(
    storage: tauri::State<'_, Arc<Storage>>,
) -> Result<(), String> {
    storage.save_onboarding_completed();
    Ok(())
}

// MARK: - Simple commands

pub fn save_setting(
    storage: tauri::State<'_, Arc<Storage>>,
    notifier: tauri::State<'_, Arc<crate::RefreshNotifier>>,
    key: String,
    value: String,
) -> Result<(), String> {
    storage.save_setting(&key, &value);
    if key == "refresh_interval" {
        if let Ok(secs) = value.parse::<u64>() {
            let _ = notifier.tx.send(secs);
            crate::debug_log!("[save_setting] 已通知定时器，刷新间隔变更为 {} 秒", secs);
        }
    }
    Ok(())
}

pub fn load_setting(
    storage: tauri::State<'_, Arc<Storage>>,
    key: String,
) -> Result<Option<String>, String> {
    Ok(storage.load_setting(&key))
}

#[tauri::command]
pub async fn ds_open_login_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::ds::login::start_login_flow(app).await
}

#[tauri::command]
pub async fn ds_clear_platform_session(
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<(), String> {
    storage.clear_platform_token();
    storage.clear_platform_cookies();
    *api_state.platform_token.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

pub async fn show_settings_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::show_settings(&app);
    Ok(())
}

pub fn is_windows11() -> bool {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct OsVersionInfoExW {
            dw_os_version_info_size: u32,
            _major: u32,
            _minor: u32,
            dw_build_number: u32,
            _platform_id: u32,
            _csd_version: [u16; 128],
            _service_pack_major: u16,
            _service_pack_minor: u16,
            _suite_mask: u16,
            _product_type: u8,
            _reserved: u8,
        }
        extern "system" {
            fn RtlGetVersion(info: *mut OsVersionInfoExW) -> i32;
        }
        unsafe {
            let mut info: OsVersionInfoExW = std::mem::zeroed();
            info.dw_os_version_info_size = std::mem::size_of::<OsVersionInfoExW>() as u32;
            if RtlGetVersion(&mut info) == 0 {
                return info.dw_build_number >= 22000;
            }
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    false
}

#[tauri::command]
pub async fn ds_toggle_trend_detail(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::TrendDetailState>,
    date: String,
    cache_hit: i64,
    cache_miss: i64,
    output: i64,
    cache_hit_rate: String,
    cost: i64,
    audio_duration: Option<i64>,
) -> Result<(), String> {
    let audio_duration = audio_duration.unwrap_or(0);
    let new_data = serde_json::json!({
        "date": date,
        "cacheHit": cache_hit,
        "cacheMiss": cache_miss,
        "output": output,
        "cacheHitRate": cache_hit_rate,
        "cost": cost,
        "audioDuration": audio_duration,
    });

    // If window is visible, compare dates
    if let Some(detail_win) = app.get_webview_window("trend-detail") {
        if detail_win.is_visible().unwrap_or(false) {
            if let Ok(data) = state.data.lock() {
                let current_date = data.get("date").and_then(|v| v.as_str()).unwrap_or("");
                if current_date == date {
                    // Same date → close the window
                    drop(data);
                    crate::windows::hide_trend_detail(&app);
                    return Ok(());
                }
            }
            // Different date → emit updated data to existing window
            if let Ok(mut data) = state.data.lock() {
                *data = new_data.clone();
            }
            let _ = detail_win.emit("trend-detail-data", new_data);
            return Ok(());
        }
    }

    // Window not visible → store data and show new window
    if let Ok(mut data) = state.data.lock() {
        *data = new_data;
    }
    crate::windows::show_trend_detail(&app, &date, cache_hit, cache_miss, output, &cache_hit_rate, cost, audio_duration).await;
    Ok(())
}

// get_trend_detail_data is the shared command (all views use this).
#[tauri::command]
pub async fn get_trend_detail_data(
    state: tauri::State<'_, crate::TrendDetailState>,
) -> Result<serde_json::Value, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    Ok(data.clone())
}

#[tauri::command]
pub async fn ds_get_trend_detail_data(
    state: tauri::State<'_, crate::TrendDetailState>,
) -> Result<serde_json::Value, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    Ok(data.clone())
}

#[tauri::command]
pub async fn ds_hide_trend_detail(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::hide_trend_detail(&app);
    Ok(())
}

pub async fn hide_main_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::close_panel(&app);
    Ok(())
}

pub async fn show_main_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    if let Some(main_win) = app.get_webview_window("main") {
        main_win.show().map_err(|e| e.to_string())?;
        main_win.set_focus().map_err(|e| e.to_string())?;
        crate::windows::windows_shown(&app, "main");
        crate::windows::update_tray_menu_checks(&app);
    }
    Ok(())
}

pub async fn hide_settings_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::hide_settings_window(&app);
    Ok(())
}

pub async fn hide_widget_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::save_window_pos(&app, "widget");
    if let Some(win) = app.get_webview_window("widget") {
        let _ = win.hide();
    }
    crate::windows::windows_hidden(&app, "widget");
    crate::windows::update_tray_menu_checks(&app);
    Ok(())
}

pub fn save_window_position(
    storage: tauri::State<'_, Arc<Storage>>,
    window: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    storage.save_setting(&format!("{}_x", window), &x.to_string());
    storage.save_setting(&format!("{}_y", window), &y.to_string());
    Ok(())
}

pub fn load_window_position(
    storage: tauri::State<'_, Arc<Storage>>,
    window: String,
) -> Result<Option<(i32, i32)>, String> {
    let x = storage.load_setting(&format!("{}_x", window))
        .and_then(|v| v.parse().ok());
    let y = storage.load_setting(&format!("{}_y", window))
        .and_then(|v| v.parse().ok());
    match (x, y) {
        (Some(x), Some(y)) => Ok(Some((x, y))),
        _ => Ok(None),
    }
}

pub async fn reposition_trend_detail(
    app: tauri::AppHandle,
) -> Result<(), String> {
    if let Some(detail_win) = app.get_webview_window("trend-detail") {
        if detail_win.is_visible().unwrap_or(false) {
            crate::windows::reposition_trend_detail(&app, &detail_win);
        }
    }
    Ok(())
}

// ── manual refresh broadcast ──

/// 手动触发刷新 — 由前端保存/清除 API Key、登录/退出等操作调用
#[tauri::command]
pub async fn ds_broadcast_refresh(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<(), String> {
    if api_state.is_refreshing.swap(true, Ordering::Acquire) {
        return Ok(());
    }
    let _guard = RefreshGuard::new(&api_state.is_refreshing);
    let result = do_refresh(&storage, &api_state, Some(&app)).await;
    if let Ok(data) = result {
        api_state.cache_data(&data);
        let _ = app.emit("ds-trigger-refresh", data);
    }
    Ok(())
}

/// 通知所有窗口 edge-snap 设置已变更
pub async fn broadcast_edge_snap_setting(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let _ = app.emit("edge-snap-setting-changed", enabled);
    Ok(())
}

#[tauri::command]
pub async fn ds_open_top_up_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::ds::top_up::open_top_up_window(app).await
}

// ── 开机启动注册表操作 ──

const AUTOSTART_REG_VALUE_NAME: &str = "ModelMeter";
const LEGACY_AUTOSTART_REG_VALUE_NAME: &str = "DeepSeekDesktopAssistant";

fn read_autostart_registry() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ,
    )
    .ok()
    .and_then(|key| {
        key.get_value::<String, _>(AUTOSTART_REG_VALUE_NAME)
            .or_else(|_| key.get_value::<String, _>(LEGACY_AUTOSTART_REG_VALUE_NAME))
            .ok()
    })
}

fn write_autostart_registry(exe_path: &str) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegValue;

    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("无法打开注册表项: {}", e))?;

    // 写入注册表值
    let utf16: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes: Vec<u8> = utf16.iter().flat_map(|c| c.to_le_bytes()).collect();
    let reg_value = RegValue {
        bytes,
        vtype: winreg::enums::RegType::REG_SZ,
    };
    key.set_raw_value(AUTOSTART_REG_VALUE_NAME, &reg_value)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    let _ = key.delete_value(LEGACY_AUTOSTART_REG_VALUE_NAME);

    // 轮询等待验证：杀毒软件可能弹窗让用户确认，最长等待 60 秒
    // 每 500ms 回读一次，直到验证通过或超时
    for _ in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        match read_autostart_registry() {
            Some(val) if val == exe_path => return Ok(()),
            _ => continue,
        }
    }

    // 超时仍未验证通过
    match read_autostart_registry() {
        Some(_) => Err("注册表值已写入但内容不匹配，可能被安全软件拦截".into()),
        None => Err("注册表写入超时（60秒），值未被验证，可能被安全软件拦截".into()),
    }
}

fn delete_autostart_registry() -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("无法打开注册表项: {}", e))?;

    for name in [AUTOSTART_REG_VALUE_NAME, LEGACY_AUTOSTART_REG_VALUE_NAME] {
        match key.delete_value(name) {
            Ok(()) => {}
            Err(e) if e.raw_os_error() == Some(2) => {}
            Err(e) => return Err(format!("删除注册表失败: {}", e)),
        }
    }
    Ok(())
}

pub async fn get_auto_start() -> Result<bool, String> {
    // 在后台线程执行注册表操作，避免阻塞 UI
    let result = tokio::task::spawn_blocking(|| read_autostart_registry().is_some()).await;
    result.map_err(|e| format!("任务执行失败: {}", e))
}

pub async fn set_auto_start(enabled: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("无法获取程序路径: {}", e))?
                .to_string_lossy()
                .to_string();
            write_autostart_registry(&exe_path)
        } else {
            delete_autostart_registry()
        }
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}
