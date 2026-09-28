use crate::mimo::api::{self, ApiState};
use crate::mimo::models::*;
use crate::storage::Storage;
use chrono::{Datelike, Local, Timelike};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

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

fn effective_today(now: &chrono::DateTime<Local>) -> String {
    let hour = now.hour();
    let date = if hour < 7 {
        now.date_naive().pred_opt().unwrap_or(now.date_naive())
    } else {
        now.date_naive()
    };
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local_dt(y: i32, m: u32, d: u32, h: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn effective_today_cutoff_at_7am() {
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 6)), "2026-09-09");
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 7)), "2026-09-10");
        assert_eq!(effective_today(&local_dt(2026, 9, 10, 23)), "2026-09-10");
    }

    #[test]
    fn effective_today_handles_month_boundary() {
        assert_eq!(effective_today(&local_dt(2026, 1, 1, 0)), "2025-12-31");
    }
}
use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn mimo_refresh_data(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<DashboardData, String> {
    if api_state.is_refreshing.swap(true, Ordering::Acquire) {
        return api_state.cached_data().ok_or_else(|| "refresh_in_progress".into());
    }
    let _guard = RefreshGuard::new(&api_state.is_refreshing);
    let result = perform_refresh(&storage, &api_state, Some(&app)).await;
    if let Ok(ref data) = result {
        api_state.cache_data(data);
    }
    result
}

#[tauri::command]
pub fn mimo_get_cached_data(
    api_state: tauri::State<'_, ApiState>,
) -> Option<DashboardData> {
    api_state.cached_data()
}

#[tauri::command]
pub fn mimo_has_platform_session(api_state: tauri::State<'_, ApiState>) -> bool {
    api_state.has_platform_session()
}

pub(crate) async fn perform_refresh(
    storage: &Storage,
    api_state: &ApiState,
    app: Option<&tauri::AppHandle>,
) -> Result<DashboardData, String> {
    let (data, unauth) = mimo_refresh_once(storage, api_state).await?;
    if unauth {
        if let Some(app) = app {
            crate::debug_log!("[mimo-refresh] 检测到登录失效，尝试静默重抓");
            if crate::mimo::login::silent_refresh_session(app).await {
                crate::debug_log!("[mimo-refresh] 静默重抓成功，重新刷新");
                let (data2, _) = mimo_refresh_once(storage, api_state).await?;
                return Ok(data2);
            }
            crate::debug_log!("[mimo-refresh] 静默重抓失败，保持失效状态");
        }
    }
    Ok(data)
}

async fn mimo_refresh_once(storage: &Storage, api_state: &ApiState) -> Result<(DashboardData, bool), String> {
    let platform_cookies = api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let payment_mode = storage.load_setting("payment_mode").unwrap_or_else(|| "plan".to_string());
    let is_plan_mode = payment_mode != "token";
    crate::debug_log!("[perform_refresh] payment_mode = '{}', is_plan_mode = {}", payment_mode, is_plan_mode);

    let mut data = DashboardData {
        is_account_available: false,
        total_balance: 0.0,
        granted_balance: 0.0,
        topped_up_balance: 0.0,
        flash_usage: None,
        pro_usage: None,
        flash_daily_usage: vec![],
        pro_daily_usage: vec![],
        current_day_cost: 0.0,
        current_month_cost: 0.0,
        current_day_requests: 0,
        current_day_flash_tokens: 0,
        current_day_pro_tokens: 0,
        current_day_audio_duration: 0,
        plan_name: None,
        plan_expired: None,
        plan_period_end: None,
        plan_usage_percent: None,
        plan_used: None,
        plan_limit: None,
        has_platform_session: platform_cookies.is_some(),
        is_first_launch: storage.is_first_launch(),
        last_updated: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        error_message: None,
        warning_message: None,
        payment_mode: payment_mode.clone(),
    };

    let mut errors: Vec<String> = Vec::new();
    let mut unauth = false;

    if let Some(ref cookies) = platform_cookies {
        let now = Local::now();
        let day = now.day();
        let need_prev_month = day <= 6;

        struct TokenRecord {
            date: String,
            model: String,
            total_token: i64,
            input_hit_token: i64,
            input_miss_token: i64,
            output_token: i64,
            request_count: i64,
            consumed_amount: f64,
            input_audio_duration: i64,
        }

        let mut all_records: Vec<TokenRecord> = Vec::new();
        let platform_unauth_msg = api::ApiError::PlatformUnauthorized.to_string();
        let has_platform_unauth;

        if is_plan_mode {
            let (balance_res, detail_res, detail_prev_res, plan_res, plan_usage_res) = tokio::join!(
                api::fetch_balance(Some(cookies.as_str())),
                api::fetch_detail_list(Some(cookies.as_str()), now.year(), now.month()),
                async {
                    if need_prev_month {
                        let prev = now.date_naive().pred_opt().unwrap_or(now.date_naive());
                        api::fetch_detail_list(Some(cookies.as_str()), prev.year(), prev.month()).await
                    } else {
                        Err(crate::mimo::api::ApiError::Http("skip".to_string()))
                    }
                },
                api::fetch_token_plan(Some(cookies.as_str())),
                api::fetch_token_plan_usage(Some(cookies.as_str())),
            );

            if let Ok(ref balance) = balance_res {
                if let Some(ref bd) = balance.data {
                    data.total_balance = bd.balance.parse().unwrap_or(0.0);
                    data.topped_up_balance = bd.cash_balance.parse().unwrap_or(0.0);
                    data.granted_balance = bd.gift_balance.parse().unwrap_or(0.0);
                    data.is_account_available = true;
                }
            }

            if let Ok(ref detail) = detail_res {
                if let Some(ref items) = detail.data {
                    for item in items {
                        all_records.push(TokenRecord {
                            date: item.date.clone().unwrap_or_default(),
                            model: item.model.clone().unwrap_or_default(),
                            total_token: item.total_token.unwrap_or(0),
                            input_hit_token: item.input_hit_token.unwrap_or(0),
                            input_miss_token: item.input_miss_token.unwrap_or(0),
                            output_token: item.output_token.unwrap_or(0),
                            request_count: item.request_count.unwrap_or(0),
                            consumed_amount: 0.0,
                            input_audio_duration: item.input_audio_duration.unwrap_or(0),
                        });
                    }
                }
            }
            if let Ok(ref detail) = detail_prev_res {
                if let Some(ref items) = detail.data {
                    for item in items {
                        all_records.push(TokenRecord {
                            date: item.date.clone().unwrap_or_default(),
                            model: item.model.clone().unwrap_or_default(),
                            total_token: item.total_token.unwrap_or(0),
                            input_hit_token: item.input_hit_token.unwrap_or(0),
                            input_miss_token: item.input_miss_token.unwrap_or(0),
                            output_token: item.output_token.unwrap_or(0),
                            request_count: item.request_count.unwrap_or(0),
                            consumed_amount: 0.0,
                            input_audio_duration: item.input_audio_duration.unwrap_or(0),
                        });
                    }
                }
            }

            if let Ok(ref plan) = plan_res {
                if let Some(ref pd) = plan.data {
                    data.plan_name = pd.plan_name.clone();
                    data.plan_expired = pd.expired;
                    data.plan_period_end = pd.current_period_end.clone();
                }
            }
            if let Ok(ref plan_usage) = plan_usage_res {
                if let Some(ref pud) = plan_usage.data {
                    // 从 usage.items 主套餐条目 (非 compensation_total_token) 提取
                    // used/limit/percent (percent 为 0~1, 需 ×100 转百分比)
                    // 未订阅时 usage 为 null, plan_used/plan_limit/plan_usage_percent 保持 None
                    // 前端显示 "— / — 已使用 —" (与官网一致)
                    if let Some(ref usage) = pud.usage {
                        if let Some(ref items) = usage.items {
                            for item in items.iter() {
                                let name = item.name.as_deref().unwrap_or("");
                                if name != "compensation_total_token" {
                                    data.plan_used = item.used;
                                    data.plan_limit = item.limit;
                                    if let Some(p) = item.percent {
                                        data.plan_usage_percent = Some(p * 100.0);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            has_platform_unauth = balance_res.as_ref().err().map_or(false, |e| e.is_auth_error());

            if has_platform_unauth {
                unauth = true;
                storage.clear_mimo_platform_cookies();
                *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
                data.is_account_available = false;
                data.has_platform_session = false;
                if !errors.contains(&platform_unauth_msg) { errors.push(platform_unauth_msg.clone()); }
            }
            if let Err(ref e) = balance_res { let msg = e.to_string(); if !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = detail_res { let msg = e.to_string(); if !e.is_auth_error() && !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = detail_prev_res { let msg = e.to_string(); if !e.is_auth_error() && !msg.contains("skip") && !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = plan_res { let msg = e.to_string(); if !e.is_auth_error() && !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = plan_usage_res { let msg = e.to_string(); if !e.is_auth_error() && !errors.contains(&msg) { errors.push(msg); } }

        } else {
            let (balance_res, detail_res, detail_prev_res, monthly_bill_res) = tokio::join!(
                api::fetch_balance(Some(cookies.as_str())),
                api::fetch_token_detail_list(Some(cookies.as_str()), now.year(), now.month()),
                async {
                    if need_prev_month {
                        let prev = now.date_naive().pred_opt().unwrap_or(now.date_naive());
                        api::fetch_token_detail_list(Some(cookies.as_str()), prev.year(), prev.month()).await
                    } else {
                        Err(crate::mimo::api::ApiError::Http("skip".to_string()))
                    }
                },
                api::fetch_monthly_bill(Some(cookies.as_str())),
            );

            if let Ok(ref balance) = balance_res {
                if let Some(ref bd) = balance.data {
                    data.total_balance = bd.balance.parse().unwrap_or(0.0);
                    data.topped_up_balance = bd.cash_balance.parse().unwrap_or(0.0);
                    data.granted_balance = bd.gift_balance.parse().unwrap_or(0.0);
                    data.is_account_available = true;
                }
            }

            if let Ok(ref monthly_bill) = monthly_bill_res {
                if let Some(ref bills) = monthly_bill.data {
                    let current_ym: i64 = (now.year() as i64) * 100 + now.month() as i64;
                    for bill in bills {
                        if bill.report_month == Some(current_ym) {
                            if let Some(ref amt) = bill.consumption_amount {
                                data.current_month_cost = amt.parse().unwrap_or(data.current_month_cost);
                            }
                            break;
                        }
                    }
                }
            }

            if let Ok(ref detail) = detail_res {
                if let Some(ref items) = detail.data {
                    for item in items {
                        all_records.push(TokenRecord {
                            date: item.date.clone().unwrap_or_default(),
                            model: item.model.clone().unwrap_or_default(),
                            total_token: item.total_token.unwrap_or(0),
                            input_hit_token: item.input_hit_token.unwrap_or(0),
                            input_miss_token: item.input_miss_token.unwrap_or(0),
                            output_token: item.output_token.unwrap_or(0),
                            request_count: item.request_count.unwrap_or(0),
                            consumed_amount: item.consumed_amount.as_deref().unwrap_or("0").parse().unwrap_or(0.0),
                            input_audio_duration: item.input_audio_duration.unwrap_or(0),
                        });
                    }
                }
            }
            if let Ok(ref detail) = detail_prev_res {
                if let Some(ref items) = detail.data {
                    for item in items {
                        all_records.push(TokenRecord {
                            date: item.date.clone().unwrap_or_default(),
                            model: item.model.clone().unwrap_or_default(),
                            total_token: item.total_token.unwrap_or(0),
                            input_hit_token: item.input_hit_token.unwrap_or(0),
                            input_miss_token: item.input_miss_token.unwrap_or(0),
                            output_token: item.output_token.unwrap_or(0),
                            request_count: item.request_count.unwrap_or(0),
                            consumed_amount: item.consumed_amount.as_deref().unwrap_or("0").parse().unwrap_or(0.0),
                            input_audio_duration: item.input_audio_duration.unwrap_or(0),
                        });
                    }
                }
            }

            let today_str = effective_today(&now);
            let mut day_cost: f64 = 0.0;
            for rec in &all_records {
                if rec.date == today_str {
                    if api::is_pro_model(&rec.model) || api::is_standard_model(&rec.model) {
                        day_cost += rec.consumed_amount;
                    }
                }
            }
            data.current_day_cost = day_cost;

            // 本月消费：从明细列表累加当月所有记录（月度账单接口月底才统计，无法实时获取）
            let current_ym_prefix = format!("{:04}-{:02}", now.year(), now.month());
            let mut month_cost: f64 = 0.0;
            for rec in &all_records {
                if rec.date.starts_with(&current_ym_prefix) {
                    if api::is_pro_model(&rec.model) || api::is_standard_model(&rec.model) {
                        month_cost += rec.consumed_amount;
                    }
                }
            }
            data.current_month_cost = month_cost;

            has_platform_unauth = balance_res.as_ref().err().map_or(false, |e| e.is_auth_error());

            if has_platform_unauth {
                unauth = true;
                storage.clear_mimo_platform_cookies();
                *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
                data.is_account_available = false;
                data.has_platform_session = false;
                if !errors.contains(&platform_unauth_msg) { errors.push(platform_unauth_msg.clone()); }
            }
            if let Err(ref e) = balance_res { let msg = e.to_string(); if !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = detail_res { let msg = e.to_string(); if !e.is_auth_error() && !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = detail_prev_res { let msg = e.to_string(); if !e.is_auth_error() && !msg.contains("skip") && !errors.contains(&msg) { errors.push(msg); } }
            if let Err(ref e) = monthly_bill_res { let msg = e.to_string(); if !e.is_auth_error() && !errors.contains(&msg) { errors.push(msg); } }
        }

        if !all_records.is_empty() {
            let today_str = effective_today(&now);
            let mut flash_map: std::collections::HashMap<String, ModelDailyUsagePoint> = std::collections::HashMap::new();
            let mut pro_map: std::collections::HashMap<String, ModelDailyUsagePoint> = std::collections::HashMap::new();
            // 使用 f64 精确累加每日费用，最后统一四舍五入，避免逐条四舍五入导致精度丢失
            let mut flash_cost_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            let mut pro_cost_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            let mut flash_total_tokens: i64 = 0;
            let mut pro_total_tokens: i64 = 0;
            let mut flash_total_cost: f64 = 0.0;
            let mut pro_total_cost: f64 = 0.0;

            for rec in &all_records {
                if rec.model.is_empty() { continue; }
                let is_standard = api::is_standard_model(&rec.model);
                let is_pro = api::is_pro_model(&rec.model);
                if !is_standard && !is_pro { continue; }
                if rec.date.is_empty() { continue; }

                let tok = rec.total_token;
                let hit = rec.input_hit_token;
                let miss = rec.input_miss_token;
                let out = rec.output_token;
                let req = rec.request_count;
                let cost = rec.consumed_amount;

                let target_map = if is_pro { &mut pro_map } else { &mut flash_map };
                let point = target_map.entry(rec.date.clone()).or_insert_with(|| ModelDailyUsagePoint {
                    date: rec.date.clone(),
                    label: chrono::NaiveDate::parse_from_str(&rec.date, "%Y-%m-%d")
                        .map(|d| d.format("%m-%d").to_string())
                        .unwrap_or(rec.date.clone()),
                    total_tokens: 0,
                    input_cache_hit_tokens: 0,
                    input_cache_miss_tokens: 0,
                    output_tokens: 0,
                    request_count: 0,
                    cost_in_cents: 0,
                    input_audio_duration: 0,
                });

                point.total_tokens += tok;
                point.input_cache_hit_tokens += hit;
                point.input_cache_miss_tokens += miss;
                point.output_tokens += out;
                point.request_count += req;
                point.input_audio_duration += rec.input_audio_duration;
                // 精确累加费用到 f64 map，不在此处四舍五入
                let cost_map = if is_pro { &mut pro_cost_map } else { &mut flash_cost_map };
                *cost_map.entry(rec.date.clone()).or_insert(0.0) += cost;

                if is_pro {
                    pro_total_tokens += tok;
                    pro_total_cost += cost;
                    if rec.date == today_str {
                        data.current_day_requests += req;
                        data.current_day_pro_tokens += tok;
                        data.current_day_audio_duration += rec.input_audio_duration;
                    }
                } else {
                    flash_total_tokens += tok;
                    flash_total_cost += cost;
                    if rec.date == today_str {
                        data.current_day_requests += req;
                        data.current_day_flash_tokens += tok;
                        data.current_day_audio_duration += rec.input_audio_duration;
                    }
                }
            }

            // 循环结束后，将精确费用统一四舍五入写入 cost_in_cents（与官网一致）
            for point in flash_map.values_mut() {
                if let Some(exact) = flash_cost_map.get(&point.date) {
                    point.cost_in_cents = (exact * 100.0).round() as i64;
                }
            }
            for point in pro_map.values_mut() {
                if let Some(exact) = pro_cost_map.get(&point.date) {
                    point.cost_in_cents = (exact * 100.0).round() as i64;
                }
            }

            let mut flash_daily: Vec<ModelDailyUsagePoint> = flash_map.into_values().collect();
            flash_daily.sort_by(|a, b| a.date.cmp(&b.date));
            let mut pro_daily: Vec<ModelDailyUsagePoint> = pro_map.into_values().collect();
            pro_daily.sort_by(|a, b| a.date.cmp(&b.date));

            if flash_total_tokens > 0 {
                data.flash_usage = Some(ModelUsageSummary {
                    model: MimoModel::Standard,
                    total_tokens: flash_total_tokens,
                    cost_in_cents: (flash_total_cost * 100.0).round() as i64,
                    total_tokens_formatted: api::format_number(flash_total_tokens),
                    cost_formatted: if is_plan_mode { "套餐".to_string() } else { format!("¥{:.2}", flash_total_cost) },
                });
            }
            if pro_total_tokens > 0 {
                data.pro_usage = Some(ModelUsageSummary {
                    model: MimoModel::Pro,
                    total_tokens: pro_total_tokens,
                    cost_in_cents: (pro_total_cost * 100.0).round() as i64,
                    total_tokens_formatted: api::format_number(pro_total_tokens),
                    cost_formatted: if is_plan_mode { "套餐".to_string() } else { format!("¥{:.2}", pro_total_cost) },
                });
            }

            data.flash_daily_usage = flash_daily;
            data.pro_daily_usage = pro_daily;
        }
    }

    let unauth_msg = api::ApiError::PlatformUnauthorized.to_string();
    if errors.iter().any(|e| *e == unauth_msg) {
        errors.retain(|e| *e == unauth_msg);
    }

    if !errors.is_empty() {
        data.error_message = Some(errors.join("\n"));
    }

    api_state.cache_data(&data);
    Ok((data, unauth))
}

#[tauri::command]
pub fn mimo_mark_onboarding_completed(
    storage: tauri::State<'_, Arc<Storage>>,
) -> Result<(), String> {
    storage.save_onboarding_completed();
    Ok(())
}

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
    if key == "payment_mode" {
        crate::debug_log!("[save_setting] payment_mode 已保存为 '{}'", value);
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
pub async fn mimo_open_login_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::mimo::login::start_login_flow(app).await
}

#[tauri::command]
pub async fn mimo_clear_platform_session(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<(), String> {
    storage.clear_mimo_platform_cookies();
    *api_state.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // 清理 WebView2 自身存储的 cookie, 避免下次打开登录窗口仍为已登录状态
    // WebView2 cookie 与 platform_cookies.json 是两套独立存储
    let _ = crate::mimo::login::clear_webview_cookies(&app);
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
pub async fn mimo_toggle_trend_detail(
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

    if let Some(detail_win) = app.get_webview_window("trend-detail") {
        if detail_win.is_visible().unwrap_or(false) {
            if let Ok(data) = state.data.lock() {
                let current_date = data.get("date").and_then(|v| v.as_str()).unwrap_or("");
                if current_date == date {
                    drop(data);
                    crate::windows::hide_trend_detail(&app);
                    return Ok(());
                }
            }
            if let Ok(mut data) = state.data.lock() {
                *data = new_data.clone();
            }
            let _ = detail_win.emit("trend-detail-data", new_data);
            return Ok(());
        }
    }

    if let Ok(mut data) = state.data.lock() {
        *data = new_data;
    }
    crate::windows::show_trend_detail(&app, &date, cache_hit, cache_miss, output, &cache_hit_rate, cost, audio_duration).await;
    Ok(())
}

// get_mimo_trend_detail_data is the shared command for MiMo trend detail views.
#[tauri::command]
pub fn get_mimo_trend_detail_data(
    state: tauri::State<'_, crate::TrendDetailState>,
) -> Option<serde_json::Value> {
    let data = state.data.lock().ok()?;
    if data.is_null() || data.as_object().is_some_and(|o| o.is_empty()) {
        return None;
    }
    Some(data.clone())
}

#[tauri::command]
pub fn mimo_get_trend_detail_data(
    state: tauri::State<'_, crate::TrendDetailState>,
) -> Option<serde_json::Value> {
    let data = state.data.lock().ok()?;
    if data.is_null() || data.as_object().is_some_and(|o| o.is_empty()) {
        return None;
    }
    Some(data.clone())
}

#[tauri::command]
pub async fn mimo_hide_trend_detail(
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

#[tauri::command]
pub async fn mimo_broadcast_refresh(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<(), String> {
    if api_state.is_refreshing.swap(true, Ordering::Acquire) {
        return Ok(());
    }
    let _guard = RefreshGuard::new(&api_state.is_refreshing);
    let result = perform_refresh(&storage, &api_state, Some(&app)).await;
    if let Ok(data) = result {
        api_state.cache_data(&data);
        let _ = app.emit("mimo-trigger-refresh", data);
    }
    Ok(())
}

pub async fn broadcast_edge_snap_setting(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let _ = app.emit("edge-snap-setting-changed", enabled);
    Ok(())
}

#[tauri::command]
pub async fn broadcast_payment_mode(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
    mode: String,
) -> Result<(), String> {
    crate::debug_log!("[broadcast_payment_mode] 模式变更为 '{}', 刷新数据（payload 携带新模式）", mode);
    // payment_mode 已由前端 save_setting 保存，perform_refresh 会读取并放入 data.payment_mode
    // 前端 applyData 时同步切换 paymentMode，保证数据与模式在同一事件中更新，避免闪烁
    if !api_state.is_refreshing.swap(true, Ordering::Acquire) {
        let _guard = RefreshGuard::new(&api_state.is_refreshing);
        match perform_refresh(&storage, &api_state, Some(&app)).await {
            Ok(data) => {
                api_state.cache_data(&data);
                let _ = app.emit("mimo-trigger-refresh", &data);
            }
            Err(_e) => {
                crate::debug_log!("[broadcast_payment_mode] 刷新失败: {}", _e);
            }
        }
    }
    // 兜底：如果 is_refreshing 被占用（后台定时器正在刷新），本次刷新被跳过
    // 此时 emit payment-mode-changed 让前端先切换 UI，后台定时器完成后 trigger-refresh 会携带正确模式自动修正
    let _ = app.emit("payment-mode-changed", &mode);
    Ok(())
}

const AUTOSTART_VALUE_NAME: &str = "MimoAssistant";

fn get_autostart_key() -> Result<winreg::RegKey, String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map(|(key, _)| key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_autostart_enabled() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        let key = get_autostart_key()?;
        Ok(key.get_value::<String, _>(AUTOSTART_VALUE_NAME).is_ok())
    }
    #[cfg(not(target_os = "windows"))]
    Ok(false)
}

#[tauri::command]
pub fn enable_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = exe.to_string_lossy().to_string();
        let key = get_autostart_key()?;
        key.set_value(AUTOSTART_VALUE_NAME, &path).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    Ok(())
}

#[tauri::command]
pub fn disable_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let key = get_autostart_key()?;
        match key.delete_value(AUTOSTART_VALUE_NAME) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    #[cfg(not(target_os = "windows"))]
    Ok(())
}

pub fn is_window_docked(
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    Ok(window.is_visible().unwrap_or(false) && window.is_always_on_top().unwrap_or(false))
}

pub fn snap_to_edge(
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let _ = window.set_always_on_top(true);
    Ok(())
}

pub fn set_dragging(
    _window: tauri::WebviewWindow,
    _dragging: bool,
) -> Result<(), String> {
    crate::debug_log!("[set_dragging] label={}, dragging={}", _window.label(), _dragging);
    Ok(())
}

pub fn get_monitor_top_y(
    window: tauri::WebviewWindow,
) -> Result<i32, String> {
    window.current_monitor()
        .ok()
        .flatten()
        .map(|m| (m.position().y as f64 / m.scale_factor()) as i32)
        .ok_or("no monitor".to_string())
}

#[tauri::command]
pub async fn do_refresh(
    app: tauri::AppHandle,
    storage: tauri::State<'_, Arc<Storage>>,
    api_state: tauri::State<'_, ApiState>,
) -> Result<DashboardData, String> {
    if api_state.is_refreshing.swap(true, Ordering::Acquire) {
        return api_state.cached_data().ok_or_else(|| "refresh_in_progress".into());
    }
    let _guard = RefreshGuard::new(&api_state.is_refreshing);
    let result = perform_refresh(&storage, &api_state, Some(&app)).await;
    if let Ok(ref data) = result {
        api_state.cache_data(data);
    }
    result
}

/// 内部版本：从 lib.rs 的定时器调用，不通过 Tauri command 系统
pub async fn do_refresh_inner(
    storage: &Storage,
    api_state: &ApiState,
) -> Result<DashboardData, String> {
    perform_refresh(storage, api_state, None).await
}

#[tauri::command]
pub async fn open_platform_login(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::mimo::login::start_login_flow(app).await
}

#[tauri::command]
pub fn show_widget_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    crate::windows::toggle_widget(&app);
    Ok(())
}

#[tauri::command]
pub fn open_top_up() -> Result<(), String> {
    crate::windows::open_top_up_browser();
    Ok(())
}

#[tauri::command]
pub async fn open_plan_manage(app: tauri::AppHandle) -> Result<(), String> {
    crate::windows::open_plan_manage(app).await
}

#[tauri::command]
pub async fn open_balance_page(app: tauri::AppHandle) -> Result<(), String> {
    crate::windows::open_balance_page(app).await
}

pub async fn show_trend_detail_window(
    state: tauri::State<'_, crate::TrendDetailState>,
    app: tauri::AppHandle,
    date: Option<String>,
    center_x: Option<f64>,
    center_y: Option<f64>,
    _parent_width: Option<f64>,
    _parent_height: Option<f64>,
) -> Result<(), String> {
    if let Some(ref date) = date {
        let data = serde_json::json!({
            "date": date,
            "center_x": center_x,
            "center_y": center_y,
        });
        if let Ok(mut lock) = state.data.lock() {
            *lock = data;
        }
        crate::windows::show_trend_detail(&app, date, 0, 0, 0, "0%", 0, 0).await;
    }
    Ok(())
}
