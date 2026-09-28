use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: String,
    pub topped_up_balance: String,
}

/// 官方 Usage 导出当前只含两类 flash 名称（上游 JayHome137/DeepSeekMonitor v1.6 口径）：
/// V4.1 Flash 是新模型，V4 Flash 是旧名称，二者不代表"新/旧两代在役模型"之外的关系。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeepSeekModel {
    Flash,
    V4Flash,
}

impl DeepSeekModel {
    pub fn api_model_name(&self) -> &str {
        match self {
            DeepSeekModel::Flash => "deepseek-flash",
            DeepSeekModel::V4Flash => "deepseek-v4-flash",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            DeepSeekModel::Flash => "V4.1 Flash",
            DeepSeekModel::V4Flash => "V4 Flash",
        }
    }

    pub fn short_name(&self) -> &str {
        match self {
            DeepSeekModel::Flash => "V4.1",
            DeepSeekModel::V4Flash => "V4",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageSummary {
    pub model: DeepSeekModel,
    pub total_tokens: i32,
    pub cost_in_cents: i32,
    pub total_tokens_formatted: String,
    pub cost_formatted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDailyUsagePoint {
    pub date: String,
    pub label: String,
    pub total_tokens: i32,
    pub input_cache_hit_tokens: i32,
    pub input_cache_miss_tokens: i32,
    pub output_tokens: i32,
    pub request_count: i32,
    pub cost_in_cents: i32,
}

// Platform API models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUsageEntry {
    #[serde(rename = "type")]
    pub usage_type: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformModelData {
    pub model: String,
    pub usage: Vec<PlatformUsageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformDayData {
    pub date: String,
    pub data: Vec<PlatformModelData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformBizDataContent {
    pub total: Vec<PlatformModelData>,
    pub days: Vec<PlatformDayData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCostResponse {
    pub code: i32,
    pub data: Option<PlatformCostInnerData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAmountResponse {
    pub code: i32,
    pub data: Option<PlatformAmountInnerData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCostInnerData {
    pub biz_code: i32,
    pub biz_data: Option<Vec<PlatformBizDataContent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAmountInnerData {
    pub biz_code: i32,
    pub biz_data: Option<PlatformBizDataContent>,
}

// User Summary API models (platform.deepseek.com/api/v0/users/get_user_summary)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummaryResponse {
    pub code: i32,
    pub data: Option<UserSummaryInnerData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummaryInnerData {
    pub biz_code: i32,
    pub biz_data: Option<UserSummaryBizData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummaryBizData {
    pub normal_wallets: Vec<UserSummaryWallet>,
    pub bonus_wallets: Vec<UserSummaryWallet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummaryWallet {
    pub currency: String,
    pub balance: String,
    pub token_estimation: String,
}

// Dashboard aggregate data sent to frontend

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    // Balance
    pub is_account_available: bool,
    pub total_balance: f64,
    pub granted_balance: f64,
    pub topped_up_balance: f64,
    pub balance_info: Option<BalanceInfo>,

    // Usage
    pub flash_usage: Option<ModelUsageSummary>,
    pub v4flash_usage: Option<ModelUsageSummary>,
    pub flash_daily_usage: Vec<ModelDailyUsagePoint>,
    pub v4flash_daily_usage: Vec<ModelDailyUsagePoint>,

    // Platform data
    pub current_day_cost: f64,
    pub current_month_cost: f64,
    pub current_day_requests: i32,
    pub current_day_flash_tokens: i64,
    pub current_day_v4flash_tokens: i64,

    // State
    pub has_platform_session: bool,
    pub is_first_launch: bool,
    pub last_updated: String,
    pub error_message: Option<String>,
    pub warning_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_metadata_matches_platform_api_names() {
        assert_eq!(DeepSeekModel::Flash.api_model_name(), "deepseek-flash");
        assert_eq!(DeepSeekModel::V4Flash.api_model_name(), "deepseek-v4-flash");
        assert_eq!(DeepSeekModel::Flash.display_name(), "V4.1 Flash");
        assert_eq!(DeepSeekModel::V4Flash.display_name(), "V4 Flash");
        assert_eq!(DeepSeekModel::Flash.short_name(), "V4.1");
        assert_eq!(DeepSeekModel::V4Flash.short_name(), "V4");
    }

    #[test]
    fn model_deserializes_from_lowercase() {
        let flash: DeepSeekModel = serde_json::from_str("\"flash\"").unwrap();
        assert_eq!(flash, DeepSeekModel::Flash);
        let v4flash: DeepSeekModel = serde_json::from_str("\"v4flash\"").unwrap();
        assert_eq!(v4flash, DeepSeekModel::V4Flash);
    }

    #[test]
    fn parses_platform_amount_response() {
        let json = r#"{
            "code": 0,
            "data": {
                "biz_code": 0,
                "biz_data": {
                    "total": [
                        {"model":"deepseek-v4-flash","usage":[{"type":"PROMPT_TOKEN","amount":"100"}]}
                    ],
                    "days": [
                        {"date":"2026-09-01","data":[{"model":"deepseek-v4-flash","usage":[{"type":"RESPONSE_TOKEN","amount":"50"}]}]}
                    ]
                }
            }
        }"#;
        let resp: PlatformAmountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 0);
        let biz = resp.data.unwrap().biz_data.unwrap();
        assert_eq!(biz.total[0].model, "deepseek-v4-flash");
        assert_eq!(biz.total[0].usage[0].usage_type, "PROMPT_TOKEN");
        assert_eq!(biz.days[0].data[0].usage[0].amount, "50");
    }

    #[test]
    fn parses_platform_cost_response_with_null_biz_data() {
        let json = r#"{"code":0,"data":{"biz_code":0,"biz_data":null}}"#;
        let resp: PlatformCostResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.unwrap().biz_data.is_none());
    }

    #[test]
    fn parses_user_summary_wallets() {
        let json = r#"{
            "code":0,
            "data":{"biz_code":0,"biz_data":{
                "normal_wallets":[{"currency":"CNY","balance":"12.5","token_estimation":"1000"}],
                "bonus_wallets":[]
            }}
        }"#;
        let resp: UserSummaryResponse = serde_json::from_str(json).unwrap();
        let biz = resp.data.unwrap().biz_data.unwrap();
        assert_eq!(biz.normal_wallets[0].currency, "CNY");
        assert_eq!(biz.normal_wallets[0].balance, "12.5");
        assert!(biz.bonus_wallets.is_empty());
    }
}