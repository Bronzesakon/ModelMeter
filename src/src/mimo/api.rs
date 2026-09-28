use crate::mimo::models::*;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::sync::OnceLock;

const PLATFORM_BASE_URL: &str = "https://platform.xiaomimimo.com";

/// 安全截断字符串，确保不截断在 UTF-8 字符中间
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_shared_client() -> &'static reqwest::Client {
    SHARED_CLIENT.get_or_init(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("*/*"));
        headers.insert(reqwest::header::ORIGIN, reqwest::header::HeaderValue::from_static("https://platform.xiaomimimo.com"));
        headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_static("https://platform.xiaomimimo.com/"));
        headers.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
        headers.insert("x-timezone", reqwest::header::HeaderValue::from_static("Asia/Shanghai"));

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36")
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client")
    })
}

fn attach_cookie(mut req: reqwest::RequestBuilder, cookies: Option<&str>) -> reqwest::RequestBuilder {
    if let Some(cookie_str) = cookies {
        crate::debug_log!("[api] Cookie header: {}", cookie_str);
        if let Ok(val) = reqwest::header::HeaderValue::from_str(cookie_str) {
            req = req.header(reqwest::header::COOKIE, val);
        }
    }
    req
}

/// 复用统一错误类型（变体与历史代码兼容）。
pub use crate::error::AppError as ApiError;

pub struct ApiState {
    pub platform_cookies: Mutex<Option<String>>,
    pub is_refreshing: AtomicBool,
    pub last_data: Mutex<Option<DashboardData>>,
}

impl ApiState {
    pub fn new() -> Self {
        Self {
            platform_cookies: Mutex::new(None),
            is_refreshing: AtomicBool::new(false),
            last_data: Mutex::new(None),
        }
    }

    pub fn has_platform_session(&self) -> bool {
        self.platform_cookies.lock().unwrap_or_else(|e| e.into_inner()).is_some()
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

pub fn is_pro_model(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("pro")
}

pub fn is_standard_model(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.contains("pro") && (lower.contains("mimo") || lower.contains("standard") || lower.contains("v2"))
}

fn extract_ph_from_cookies(cookies: Option<&str>) -> Option<String> {
    cookies.and_then(|c| {
        for part in c.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("api-platform_ph=") {
                return Some(val.trim_matches('"').to_string());
            }
        }
        None
    })
}

fn append_ph(url: &str, cookies: Option<&str>) -> String {
    if let Some(ph) = extract_ph_from_cookies(cookies) {
        let encoded = urlencoding::encode(&ph);
        if url.contains('?') {
            format!("{}&api-platform_ph={}", url, encoded)
        } else {
            format!("{}?api-platform_ph={}", url, encoded)
        }
    } else {
        url.to_string()
    }
}

pub async fn fetch_balance(
    cookies: Option<&str>,
) -> Result<MimoApiBalanceResponse, ApiError> {
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/balance", PLATFORM_BASE_URL), cookies);
    crate::debug_log!("[api] fetch_balance GET {} cookie_len={}", url, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.get(&url), cookies).send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_balance 网络错误: {} - {}", url, e);
        ApiError::Http(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_balance 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_balance HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::Http(format!("HTTP {}: {}", status.as_u16(), short)));
    }
    let parsed: MimoApiBalanceResponse = serde_json::from_str(&body).map_err(|e| {
        crate::debug_log!("[api] fetch_balance 解析失败: {} body: {}", e, safe_truncate(&body, 500));
        ApiError::Http(format!("parse: {}", e))
    })?;
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_balance code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Err(ApiError::PlatformUnauthorized);
    }
    if let Some(ref _data) = parsed.data {
        crate::debug_log!(
            "[api] fetch_balance 成功 - URL: {}, 余额: {}, 充值: {}, 赠送: {}, 冻结: {}, 货币: {}",
            url, _data.balance, _data.cash_balance, _data.gift_balance, _data.frozen_balance, _data.currency
        );
    } else {
        crate::debug_log!("[api] fetch_balance 成功 - URL: {}, 数据为空", url);
    }
    Ok(parsed)
}

pub async fn fetch_detail_list(
    cookies: Option<&str>,
    year: i32,
    month: u32,
) -> Result<MimoApiDetailListResponse, ApiError> {
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/usage/token-plan/list", PLATFORM_BASE_URL), cookies);
    let json_body = serde_json::json!({"year": year, "month": month});
    crate::debug_log!("[api] fetch_detail_list POST {} body={} cookie_len={}", url, json_body, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.post(&url).header(reqwest::header::CONTENT_TYPE, "application/json").json(&json_body), cookies)
        .send()
        .await
        .map_err(|e| {
            crate::debug_log!("[api] fetch_detail_list 网络错误: {} - {}", url, e);
            ApiError::Http(e.to_string())
        })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_detail_list 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_detail_list HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::Http(format!("HTTP {}: {}", status.as_u16(), short)));
    }
    let parsed: MimoApiDetailListResponse = serde_json::from_str(&body).map_err(|e| {
        crate::debug_log!("[api] fetch_detail_list 解析失败: {} body: {}", e, safe_truncate(&body, 500));
        ApiError::Http(format!("parse: {}", e))
    })?;
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_detail_list code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Err(ApiError::PlatformUnauthorized);
    }
    let _count = parsed.data.as_ref().map(|d| d.len()).unwrap_or(0);
    crate::debug_log!("[api] fetch_detail_list 成功 - URL: {}, {}-{}月, 条目数: {}", url, year, month, _count);
    Ok(parsed)
}

pub async fn fetch_token_detail_list(
    cookies: Option<&str>,
    year: i32,
    month: u32,
) -> Result<MimoApiTokenDetailListResponse, ApiError> {
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/usage/detail/list", PLATFORM_BASE_URL), cookies);
    let json_body = serde_json::json!({"year": year, "month": month});
    crate::debug_log!("[api] fetch_token_detail_list POST {} body={} cookie_len={}", url, json_body, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.post(&url).header(reqwest::header::CONTENT_TYPE, "application/json").json(&json_body), cookies)
        .send()
        .await
        .map_err(|e| {
            crate::debug_log!("[api] fetch_token_detail_list 网络错误: {} - {}", url, e);
            ApiError::Http(e.to_string())
        })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_token_detail_list 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_token_detail_list HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::Http(format!("HTTP {}: {}", status.as_u16(), short)));
    }
    let parsed: MimoApiTokenDetailListResponse = serde_json::from_str(&body).map_err(|e| {
        crate::debug_log!("[api] fetch_token_detail_list 解析失败: {} body: {}", e, safe_truncate(&body, 500));
        ApiError::Http(format!("parse: {}", e))
    })?;
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_token_detail_list code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Err(ApiError::PlatformUnauthorized);
    }
    let _count = parsed.data.as_ref().map(|d| d.len()).unwrap_or(0);
    crate::debug_log!("[api] fetch_token_detail_list 成功 - URL: {}, {}-{}月, 条目数: {}", url, year, month, _count);
    Ok(parsed)
}

pub async fn fetch_monthly_bill(
    cookies: Option<&str>,
) -> Result<MimoApiMonthlyBillResponse, ApiError> {
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/usage/bill/monthly", PLATFORM_BASE_URL), cookies);
    crate::debug_log!("[api] fetch_monthly_bill GET {} cookie_len={}", url, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.get(&url), cookies).send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_monthly_bill 网络错误: {} - {}", url, e);
        ApiError::Http(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_monthly_bill 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_monthly_bill HTTP {}: {}", status.as_u16(), short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Err(ApiError::Http(format!("HTTP {}: {}", status.as_u16(), short)));
    }
    let parsed: MimoApiMonthlyBillResponse = serde_json::from_str(&body).map_err(|e| {
        crate::debug_log!("[api] fetch_monthly_bill 解析失败: {} body: {}", e, safe_truncate(&body, 500));
        ApiError::Http(format!("parse: {}", e))
    })?;
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_monthly_bill code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Err(ApiError::PlatformUnauthorized);
    }
    if let Some(ref bills) = parsed.data {
        for _b in bills {
            crate::debug_log!(
                "[api] fetch_monthly_bill 成功 - URL: {}, 月份: {:?}, 消费: {:?}, 赠送: {:?}, 现金: {:?}",
                url, _b.report_month, _b.consumption_amount, _b.gift_consumption, _b.cash_consumption
            );
        }
    } else {
        crate::debug_log!("[api] fetch_monthly_bill 成功 - URL: {}, 数据为空", url);
    }
    Ok(parsed)
}

pub async fn fetch_token_plan(
    cookies: Option<&str>,
) -> Result<MimoApiTokenPlanResponse, ApiError> {
    let empty_resp = || MimoApiTokenPlanResponse { code: 0, data: None };
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/tokenPlan/detail", PLATFORM_BASE_URL), cookies);
    crate::debug_log!("[api] fetch_token_plan GET {} cookie_len={}", url, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.get(&url), cookies).send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_token_plan 网络错误: {} - {}", url, e);
        ApiError::Http(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_token_plan 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let _short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_token_plan HTTP {}: {}", status.as_u16(), _short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Ok(empty_resp());
    }
    if body.is_empty() {
        crate::debug_log!("[api] fetch_token_plan 响应体为空，返回空数据");
        return Ok(empty_resp());
    }
    let parsed: MimoApiTokenPlanResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_e) => {
            crate::debug_log!("[api] fetch_token_plan 解析失败: {} body: {}", _e, safe_truncate(&body, 500));
            return Ok(empty_resp());
        }
    };
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_token_plan code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Ok(empty_resp());
    }
    if let Some(ref _data) = parsed.data {
        crate::debug_log!(
            "[api] fetch_token_plan 成功 - URL: {}, 套餐: {:?}, 过期: {:?}, 周期结束: {:?}",
            url, _data.plan_name, _data.expired, _data.current_period_end
        );
    } else {
        crate::debug_log!("[api] fetch_token_plan 成功 - URL: {}, 数据为空(无套餐)", url);
    }
    Ok(parsed)
}

pub async fn fetch_token_plan_usage(
    cookies: Option<&str>,
) -> Result<MimoApiTokenPlanUsageResponse, ApiError> {
    let empty_resp = || MimoApiTokenPlanUsageResponse { code: 0, message: None, data: None };
    let client = get_shared_client();
    let url = append_ph(&format!("{}/api/v1/tokenPlan/usage", PLATFORM_BASE_URL), cookies);
    crate::debug_log!("[api] fetch_token_plan_usage GET {} cookie_len={}", url, cookies.map_or(0, |c| c.len()));
    let resp = attach_cookie(client.get(&url), cookies).send().await.map_err(|e| {
        crate::debug_log!("[api] fetch_token_plan_usage 网络错误: {} - {}", url, e);
        ApiError::Http(e.to_string())
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        crate::debug_log!("[api] fetch_token_plan_usage 读取响应失败: {}", e);
        ApiError::Http(e.to_string())
    })?;
    if !status.is_success() {
        let _short = safe_truncate(&body, 500);
        crate::debug_log!("[api] fetch_token_plan_usage HTTP {}: {}", status.as_u16(), _short);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::PlatformUnauthorized);
        }
        return Ok(empty_resp());
    }
    if body.is_empty() {
        crate::debug_log!("[api] fetch_token_plan_usage 响应体为空，返回空数据");
        return Ok(empty_resp());
    }
    let parsed: MimoApiTokenPlanUsageResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_e) => {
            crate::debug_log!("[api] fetch_token_plan_usage 解析失败: {} body: {}", _e, safe_truncate(&body, 500));
            return Ok(empty_resp());
        }
    };
    if parsed.code != 0 {
        crate::debug_log!("[api] fetch_token_plan_usage code != 0: {}, body: {}", parsed.code, safe_truncate(&body, 500));
        return Ok(empty_resp());
    }
    if let Some(ref data) = parsed.data {
        let _percent = data.month_usage.as_ref().and_then(|m| m.percent);
        crate::debug_log!(
            "[api] fetch_token_plan_usage 成功 - URL: {}, 月度用量: {:?}%",
            url, _percent
        );
    } else {
        crate::debug_log!("[api] fetch_token_plan_usage 成功 - URL: {}, 数据为空", url);
    }
    Ok(parsed)
}

pub fn format_number(n: i64) -> String {
    let s = n.to_string();
    let (sign, digits) = if let Some(stripped) = s.strip_prefix('-') {
        ("-", stripped)
    } else {
        ("", s.as_str())
    };
    let len = digits.len();
    let mut result = String::with_capacity(s.len() + len / 3);
    result.push_str(sign);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_truncate_respects_utf8_boundaries() {
        let s = "中文abc"; // 3 + 3 + 3 = 9 bytes
        assert_eq!(safe_truncate(s, 9), "中文abc");
        assert_eq!(safe_truncate(s, 6), "中文");
        // 4/5 落在“文”的字节中间，应退回上一个字符边界
        assert_eq!(safe_truncate(s, 4), "中");
        assert_eq!(safe_truncate(s, 5), "中");
        assert_eq!(safe_truncate(s, 3), "中");
        assert_eq!(safe_truncate(s, 0), "");
        assert_eq!(safe_truncate("", 10), "");
    }

    #[test]
    fn safe_truncate_ascii() {
        assert_eq!(safe_truncate("hello", 3), "hel");
        assert_eq!(safe_truncate("hello", 5), "hello");
        assert_eq!(safe_truncate("hello", 99), "hello");
    }

    #[test]
    fn model_classification() {
        assert!(is_pro_model("mimo-v2.5-pro"));
        assert!(is_pro_model("PRO"));
        assert!(!is_pro_model("mimo-v2.5"));

        assert!(is_standard_model("mimo-v2.5"));
        assert!(is_standard_model("standard-x"));
        assert!(is_standard_model("v2-flash"));
        assert!(!is_standard_model("mimo-v2.5-pro"));
        assert!(!is_standard_model("gpt-4o"));
    }

    #[test]
    fn extract_ph_from_cookies_finds_param() {
        assert_eq!(
            extract_ph_from_cookies(Some("a=1; api-platform_ph=abc123; b=2")).as_deref(),
            Some("abc123")
        );
        assert_eq!(
            extract_ph_from_cookies(Some("api-platform_ph=\"quoted\"")).as_deref(),
            Some("quoted")
        );
        assert_eq!(extract_ph_from_cookies(Some("a=1; b=2")), None);
        assert_eq!(extract_ph_from_cookies(None), None);
    }

    #[test]
    fn append_ph_builds_query() {
        let cookies = Some("api-platform_ph=p h+x");
        assert_eq!(
            append_ph("https://platform.xiaomimimo.com/api/v1/balance", cookies),
            "https://platform.xiaomimimo.com/api/v1/balance?api-platform_ph=p%20h%2Bx"
        );
        assert_eq!(
            append_ph("https://host/path?a=1", cookies),
            "https://host/path?a=1&api-platform_ph=p%20h%2Bx"
        );
        assert_eq!(append_ph("https://host/path", None), "https://host/path");
    }

    #[test]
    fn format_number_groups_thousands() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(-1234), "-1,234");
        assert_eq!(format_number(-1), "-1");
    }

    #[test]
    fn api_state_session_and_cache() {
        let state = ApiState::new();
        assert!(!state.has_platform_session());
        assert!(state.cached_data().is_none());

        *state.platform_cookies.lock().unwrap() = Some("c=1".to_string());
        assert!(state.has_platform_session());
    }
}
