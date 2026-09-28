use crate::ds::models::*;
use reqwest::Client;
use std::sync::{Mutex, atomic::AtomicBool};

/// 复用统一错误类型（变体与历史代码兼容）。
pub use crate::error::AppError as ApiError;

pub struct ApiState {
    pub platform_token: Mutex<Option<String>>,
    pub platform_cookies: Mutex<Option<String>>,
    pub is_refreshing: AtomicBool,
    pub last_data: Mutex<Option<DashboardData>>,
}

impl ApiState {
    pub fn new() -> Self {
        Self {
            platform_token: Mutex::new(None),
            platform_cookies: Mutex::new(None),
            is_refreshing: AtomicBool::new(false),
            last_data: Mutex::new(None),
        }
    }

    pub fn has_platform_session(&self) -> bool {
        self.platform_token.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn cached_data(&self) -> Option<DashboardData> {
        self.last_data.lock().ok().and_then(|data| data.clone())
    }

    pub fn cache_data(&self, data: &DashboardData) {
        if let Ok(mut cached) = self.last_data.lock() {
            *cached = Some(data.clone());
        }
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
    }
}

const PLATFORM_BASE_URL: &str = "https://platform.deepseek.com";

pub async fn fetch_platform_cost(
    client: &Client,
    token: &str,
    cookies: Option<&str>,
    year: i32,
    month: u32,
) -> Result<PlatformCostResponse, ApiError> {
    let url = format!(
        "{}/api/v0/usage/cost?month={}&year={}",
        PLATFORM_BASE_URL, month, year
    );
    let mut req = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .header("Origin", PLATFORM_BASE_URL)
        .header("Referer", format!("{}/usage", PLATFORM_BASE_URL));

    if let Some(c) = cookies {
        req = req.header("Cookie", c);
    }

    let resp = req.send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_platform_cost 网络错误: {} - {}", url, e);
        ApiError::NetworkError(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_platform_cost 读取响应失败: {}", e);
        ApiError::NetworkError(e.to_string())
    })?;

    if !status.is_success() {
        let short = if body.len() > 500 { &body[..500] } else { &body };
        crate::debug_log!("[api] fetch_platform_cost HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::PlatformError(status.as_u16(), short.to_string()));
    }

    let resp: PlatformCostResponse = serde_json::from_str(&body)
        .map_err(|e| {
            crate::debug_log!("[api] fetch_platform_cost 解析失败: {} body: {}", e, &body[..body.len().min(500)]);
            ApiError::DecodingError(format!("platform_cost parse: {}", e))
        })?;
    if resp.code != 0 {
        crate::debug_log!("[api] fetch_platform_cost code != 0: {}, body: {}", resp.code, &body[..body.len().min(500)]);
        return Err(ApiError::PlatformUnauthorized);
    }
    Ok(resp)
}

pub async fn fetch_platform_amount(
    client: &Client,
    token: &str,
    cookies: Option<&str>,
    year: i32,
    month: u32,
) -> Result<PlatformAmountResponse, ApiError> {
    let url = format!(
        "{}/api/v0/usage/amount?month={}&year={}",
        PLATFORM_BASE_URL, month, year
    );
    let mut req = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .header("Origin", PLATFORM_BASE_URL)
        .header("Referer", format!("{}/usage", PLATFORM_BASE_URL));

    if let Some(c) = cookies {
        req = req.header("Cookie", c);
    }

    let resp = req.send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_platform_amount 网络错误: {} - {}", url, e);
        ApiError::NetworkError(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_platform_amount 读取响应失败: {}", e);
        ApiError::NetworkError(e.to_string())
    })?;

    if !status.is_success() {
        let short = if body.len() > 500 { &body[..500] } else { &body };
        crate::debug_log!("[api] fetch_platform_amount HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::PlatformError(status.as_u16(), short.to_string()));
    }

    let resp: PlatformAmountResponse = serde_json::from_str(&body)
        .map_err(|e| {
            crate::debug_log!("[api] fetch_platform_amount 解析失败: {} body: {}", e, &body[..body.len().min(500)]);
            ApiError::DecodingError(format!("platform_amount parse: {}", e))
        })?;
    if resp.code != 0 {
        crate::debug_log!("[api] fetch_platform_amount code != 0: {}, body: {}", resp.code, &body[..body.len().min(500)]);
        return Err(ApiError::PlatformUnauthorized);
    }
    Ok(resp)
}

pub async fn fetch_user_summary(
    client: &Client,
    token: &str,
    cookies: Option<&str>,
) -> Result<UserSummaryResponse, ApiError> {
    let url = format!("{}/api/v0/users/get_user_summary", PLATFORM_BASE_URL);
    let mut req = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .header("Origin", PLATFORM_BASE_URL)
        .header("Referer", format!("{}/usage", PLATFORM_BASE_URL));

    if let Some(c) = cookies {
        req = req.header("Cookie", c);
    }

    let resp = req.send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_user_summary 网络错误: {} - {}", url, e);
        ApiError::NetworkError(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_user_summary 读取响应失败: {}", e);
        ApiError::NetworkError(e.to_string())
    })?;

    if !status.is_success() {
        let short = if body.len() > 500 { &body[..500] } else { &body };
        crate::debug_log!("[api] fetch_user_summary HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::PlatformError(status.as_u16(), short.to_string()));
    }

    let resp: UserSummaryResponse = serde_json::from_str(&body)
        .map_err(|e| {
            crate::debug_log!("[api] fetch_user_summary 解析失败: {} body: {}", e, &body[..body.len().min(500)]);
            ApiError::DecodingError(format!("user_summary parse: {}", e))
        })?;
    if resp.code != 0 {
        crate::debug_log!("[api] fetch_user_summary code != 0: {}", resp.code);
        return Err(ApiError::PlatformUnauthorized);
    }
    Ok(resp)
}

// MARK: - Model name normalization（2026-09-28 官网定价页核实）
//
// 在役模型只有两个：deepseek-flash（V4.1 Flash）与 deepseek-v4-pro（V4 Pro）。
// 旧名 deepseek-v4-flash / deepseek-v4-flash-vision-exp 的请求由 V4.1-Flash
// 提供服务并按 Flash 价格计费，与 deepseek-flash 同属一桶；deepseek-reasoner
// 为 V4 Pro 的历史别名。白名单精确匹配，未知名称（如未来的 deepseek-v5-flash）
// 一律返回 None，避免误计入现有模型卡片。

pub(crate) fn classify_model(model: &str) -> Option<DeepSeekModel> {
    match model.trim().to_lowercase().as_str() {
        "deepseek-flash" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp" | "deepseek-chat" => {
            Some(DeepSeekModel::Flash)
        }
        "deepseek-v4-pro" | "deepseek-reasoner" => Some(DeepSeekModel::Pro),
        _ => None,
    }
}

/// Find first model belonging to the given bucket
pub(crate) fn find_model<'a>(
    models: &'a [crate::ds::models::PlatformModelData],
    target: DeepSeekModel,
) -> Option<&'a crate::ds::models::PlatformModelData> {
    models.iter().find(|m| classify_model(&m.model) == Some(target))
}

// MARK: - Data Processing (ported from DashboardViewModel)

pub(crate) fn format_number(n: i32) -> String {
    let is_negative = n < 0;
    let abs_n = n.unsigned_abs();
    let mut s = String::new();
    let n_str = abs_n.to_string();
    let len = n_str.len();
    for (i, c) in n_str.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }
    if is_negative {
        format!("-{}", s)
    } else {
        s
    }
}

pub(crate) fn format_date_short(date_str: &str) -> String {
    if date_str.len() >= 10 {
        date_str[5..10].to_string()
    } else {
        date_str.to_string()
    }
}

/// Build daily usage from platform API amount response
pub(crate) fn build_daily_from_platform(
    days: &[crate::ds::models::PlatformDayData],
    target: DeepSeekModel,
) -> Vec<crate::ds::models::ModelDailyUsagePoint> {
    use crate::ds::models::ModelDailyUsagePoint;

    let result: Vec<_> = days.iter()
        .filter_map(|day| {
            let model_data = find_model(&day.data, target)?;
            let mut total_tokens: i32 = 0;
            let mut cache_hit: i32 = 0;
            let mut cache_miss: i32 = 0;
            let mut output: i32 = 0;
            let mut request_count: i32 = 0;

            for entry in &model_data.usage {
                let val = entry.amount.parse::<i32>().unwrap_or(0);
                match entry.usage_type.as_str() {
                    "PROMPT_TOKEN" => total_tokens = val,
                    "PROMPT_CACHE_HIT_TOKEN" => cache_hit = val,
                    "PROMPT_CACHE_MISS_TOKEN" => cache_miss = val,
                    "RESPONSE_TOKEN" => output = val,
                    "REQUEST" => request_count = val,
                    _ => {}
                }
            }

            if total_tokens == 0 {
                total_tokens = cache_hit + cache_miss + output;
            }

            if total_tokens == 0 && request_count == 0 {
                return None;
            }

            Some(ModelDailyUsagePoint {
                date: day.date.clone(),
                label: format_date_short(&day.date),
                total_tokens,
                input_cache_hit_tokens: cache_hit,
                input_cache_miss_tokens: cache_miss,
                output_tokens: output,
                request_count,
                cost_in_cents: 0,
            })
        })
        .collect();

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ds::models::DeepSeekModel;

    #[test]
    fn classify_model_uses_current_and_historical_names_only() {
        // 当前名称
        assert_eq!(classify_model("deepseek-flash"), Some(DeepSeekModel::Flash));
        assert_eq!(classify_model("deepseek-v4-pro"), Some(DeepSeekModel::Pro));
        // 大小写与空白容错
        assert_eq!(classify_model(" DEEPSEEK-FLASH "), Some(DeepSeekModel::Flash));
        // V4 Flash 旧名已路由至 V4.1 Flash，与 deepseek-chat 一并归入 Flash 桶
        assert_eq!(classify_model("deepseek-v4-flash"), Some(DeepSeekModel::Flash));
        assert_eq!(classify_model("deepseek-v4-flash-vision-exp"), Some(DeepSeekModel::Flash));
        assert_eq!(classify_model("deepseek-chat"), Some(DeepSeekModel::Flash));
        // deepseek-reasoner 是 V4 Pro 的历史别名
        assert_eq!(classify_model("deepseek-reasoner"), Some(DeepSeekModel::Pro));
        // 未知模型不入卡片，避免污染聚合
        assert_eq!(classify_model("deepseek-v5-flash"), None);
        assert_eq!(classify_model(""), None);
    }

    #[test]
    fn find_model_picks_only_requested_bucket() {
        let models = vec![
            PlatformModelData {
                model: "deepseek-v4-pro".to_string(),
                usage: vec![],
            },
            PlatformModelData {
                model: "deepseek-flash".to_string(),
                usage: vec![],
            },
        ];
        assert!(find_model(&models, DeepSeekModel::Pro).unwrap().model == "deepseek-v4-pro");
        assert!(find_model(&models, DeepSeekModel::Flash).unwrap().model == "deepseek-flash");
    }
}
