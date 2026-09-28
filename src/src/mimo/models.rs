use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MimoModel {
    Standard,
    Pro,
}

impl MimoModel {
    pub fn display_name(&self) -> &str {
        match self {
            MimoModel::Standard => "mimo-v2.5",
            MimoModel::Pro => "mimo-v2.5-pro",
        }
    }

    pub fn short_name(&self) -> &str {
        match self {
            MimoModel::Standard => "Standard",
            MimoModel::Pro => "Pro",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiBalanceResponse {
    pub code: i32,
    pub data: Option<MimoBalanceData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoBalanceData {
    pub balance: String,
    #[serde(rename = "cashBalance")]
    pub cash_balance: String,
    #[serde(rename = "giftBalance")]
    pub gift_balance: String,
    #[serde(rename = "frozenBalance")]
    pub frozen_balance: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiDetailListResponse {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<Vec<MimoTokenPlanItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoTokenPlanItem {
    pub date: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "totalToken")]
    pub total_token: Option<i64>,
    #[serde(rename = "inputHitToken")]
    pub input_hit_token: Option<i64>,
    #[serde(rename = "inputMissToken")]
    pub input_miss_token: Option<i64>,
    #[serde(rename = "outputToken")]
    pub output_token: Option<i64>,
    #[serde(rename = "requestCount")]
    pub request_count: Option<i64>,
    #[serde(rename = "inputAudioDuration")]
    pub input_audio_duration: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiMonthlyBillResponse {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<Vec<MimoMonthBill>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoMonthBill {
    #[serde(rename = "reportMonth")]
    pub report_month: Option<i64>,
    #[serde(rename = "consumptionAmount")]
    pub consumption_amount: Option<String>,
    #[serde(rename = "giftConsumption")]
    pub gift_consumption: Option<String>,
    #[serde(rename = "cashConsumption")]
    pub cash_consumption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiTokenPlanResponse {
    pub code: i32,
    pub data: Option<MimoTokenPlanData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoTokenPlanData {
    #[serde(rename = "planCode")]
    pub plan_code: Option<String>,
    #[serde(rename = "planName")]
    pub plan_name: Option<String>,
    #[serde(rename = "currentPeriodEnd")]
    pub current_period_end: Option<String>,
    pub expired: Option<bool>,
    #[serde(rename = "enableAutoRenew")]
    pub enable_auto_renew: Option<bool>,
    #[serde(rename = "autoRenewDiscount")]
    pub auto_renew_discount: Option<serde_json::Value>,
    #[serde(rename = "hasAutoRenewSubscribed")]
    pub has_auto_renew_subscribed: Option<bool>,
    #[serde(rename = "clawEnabled")]
    pub claw_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiTokenPlanUsageResponse {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<MimoTokenPlanUsageData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoTokenPlanUsageData {
    #[serde(rename = "monthUsage")]
    pub month_usage: Option<MimoMonthUsage>,
    pub usage: Option<MimoUsageData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoMonthUsage {
    pub percent: Option<f64>,
    pub items: Option<serde_json::Value>,
}

// tokenPlan/usage 接口中 usage.items 数组的单个条目
// percent 为 0~1 小数 (官网 JS 用 d*100 转百分比)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoUsageItem {
    pub name: Option<String>,
    pub used: Option<i64>,
    pub limit: Option<i64>,
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoUsageData {
    pub items: Option<Vec<MimoUsageItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoApiTokenDetailListResponse {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<Vec<MimoTokenDetailItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimoTokenDetailItem {
    pub date: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    pub currency: Option<String>,
    #[serde(rename = "consumedAmount")]
    pub consumed_amount: Option<String>,
    #[serde(rename = "inputHitAmount")]
    pub input_hit_amount: Option<String>,
    #[serde(rename = "inputMissAmount")]
    pub input_miss_amount: Option<String>,
    #[serde(rename = "outputAmount")]
    pub output_amount: Option<String>,
    #[serde(rename = "totalToken")]
    pub total_token: Option<i64>,
    #[serde(rename = "inputHitToken")]
    pub input_hit_token: Option<i64>,
    #[serde(rename = "inputMissToken")]
    pub input_miss_token: Option<i64>,
    #[serde(rename = "outputToken")]
    pub output_token: Option<i64>,
    #[serde(rename = "requestCount")]
    pub request_count: Option<i64>,
    #[serde(rename = "inputAudioDuration")]
    pub input_audio_duration: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageSummary {
    pub model: MimoModel,
    pub total_tokens: i64,
    pub cost_in_cents: i64,
    pub total_tokens_formatted: String,
    pub cost_formatted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDailyUsagePoint {
    pub date: String,
    pub label: String,
    pub total_tokens: i64,
    pub input_cache_hit_tokens: i64,
    pub input_cache_miss_tokens: i64,
    pub output_tokens: i64,
    pub request_count: i64,
    pub cost_in_cents: i64,
    pub input_audio_duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub is_account_available: bool,
    pub total_balance: f64,
    pub granted_balance: f64,
    pub topped_up_balance: f64,

    pub flash_usage: Option<ModelUsageSummary>,
    pub pro_usage: Option<ModelUsageSummary>,
    pub flash_daily_usage: Vec<ModelDailyUsagePoint>,
    pub pro_daily_usage: Vec<ModelDailyUsagePoint>,

    pub current_day_cost: f64,
    pub current_month_cost: f64,
    pub current_day_requests: i64,
    pub current_day_flash_tokens: i64,
    pub current_day_pro_tokens: i64,
    pub current_day_audio_duration: i64,

    pub plan_name: Option<String>,
    pub plan_expired: Option<bool>,
    pub plan_period_end: Option<String>,
    pub plan_usage_percent: Option<f64>,
    // 套餐主进度条用量与总量 (来自 usage.items 主套餐条目, 非补偿积分)
    pub plan_used: Option<i64>,
    pub plan_limit: Option<i64>,

    pub has_platform_session: bool,
    pub is_first_launch: bool,
    pub last_updated: String,
    pub error_message: Option<String>,
    pub warning_message: Option<String>,
    pub payment_mode: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_display_names() {
        assert_eq!(MimoModel::Standard.display_name(), "mimo-v2.5");
        assert_eq!(MimoModel::Pro.display_name(), "mimo-v2.5-pro");
        assert_eq!(MimoModel::Standard.short_name(), "Standard");
        assert_eq!(MimoModel::Pro.short_name(), "Pro");
    }

    #[test]
    fn parses_balance_response() {
        let json = r#"{"code":0,"data":{"balance":"1.23","cashBalance":"1.00","giftBalance":"0.23","frozenBalance":"0.00","currency":"CNY"}}"#;
        let resp: MimoApiBalanceResponse = serde_json::from_str(json).unwrap();
        let data = resp.data.unwrap();
        assert_eq!(data.balance, "1.23");
        assert_eq!(data.cash_balance, "1.00");
        assert_eq!(data.gift_balance, "0.23");
        assert_eq!(data.currency, "CNY");
    }

    #[test]
    fn parses_token_detail_list() {
        let json = r#"{"code":0,"message":"ok","data":[{"date":"2026-09-01","model":"mimo-v2.5","totalToken":100,"inputHitToken":10,"inputMissToken":20,"outputToken":70,"requestCount":3,"inputAudioDuration":5}]}"#;
        let resp: MimoApiTokenDetailListResponse = serde_json::from_str(json).unwrap();
        let item = &resp.data.unwrap()[0];
        assert_eq!(item.model.as_deref(), Some("mimo-v2.5"));
        assert_eq!(item.total_token, Some(100));
        assert_eq!(item.input_hit_token, Some(10));
        assert_eq!(item.output_token, Some(70));
        assert_eq!(item.input_audio_duration, Some(5));
    }

    #[test]
    fn parses_token_plan_and_usage() {
        let plan = r#"{"code":0,"data":{"planCode":"P1","planName":"套餐","currentPeriodEnd":"2026-10-01","expired":false,"enableAutoRenew":true}}"#;
        let parsed: MimoApiTokenPlanResponse = serde_json::from_str(plan).unwrap();
        let plan_data = parsed.data.unwrap();
        assert_eq!(plan_data.plan_code.as_deref(), Some("P1"));
        assert_eq!(plan_data.plan_name.as_deref(), Some("套餐"));
        assert_eq!(plan_data.expired, Some(false));

        let usage = r#"{"code":0,"message":"ok","data":{"monthUsage":{"percent":0.42},"usage":{"items":[{"name":"主套餐","used":42,"limit":100,"percent":0.42}]}}}"#;
        let parsed: MimoApiTokenPlanUsageResponse = serde_json::from_str(usage).unwrap();
        let usage_data = parsed.data.unwrap();
        assert_eq!(usage_data.month_usage.as_ref().unwrap().percent, Some(0.42));
        let items = usage_data.usage.unwrap().items.unwrap();
        assert_eq!(items[0].name.as_deref(), Some("主套餐"));
        assert_eq!(items[0].used, Some(42));
        assert_eq!(items[0].limit, Some(100));
    }

    #[test]
    fn parses_monthly_bill() {
        let json = r#"{"code":0,"data":[{"reportMonth":202608,"consumptionAmount":"12.30","giftConsumption":"2.30","cashConsumption":"10.00"}]}"#;
        let resp: MimoApiMonthlyBillResponse = serde_json::from_str(json).unwrap();
        let bill = &resp.data.unwrap()[0];
        assert_eq!(bill.report_month, Some(202608));
        assert_eq!(bill.consumption_amount.as_deref(), Some("12.30"));
        assert_eq!(bill.cash_consumption.as_deref(), Some("10.00"));
    }

    #[test]
    fn parses_token_plan_usage_with_items_as_raw_value() {
        // monthUsage.items 为 JSON 透传字段，不应影响反序列化
        let json = r#"{"code":0,"data":{"monthUsage":{"percent":0.5,"items":{"a":1}}}}"#;
        let resp: MimoApiTokenPlanUsageResponse = serde_json::from_str(json).unwrap();
        let month = resp.data.unwrap().month_usage.unwrap();
        assert_eq!(month.percent, Some(0.5));
        assert!(month.items.is_some());
    }
}
