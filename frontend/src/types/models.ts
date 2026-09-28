export interface BalanceInfo {
  currency: string;
  total_balance: string;
  granted_balance: string;
  topped_up_balance: string;
}

export interface ModelUsageSummary {
  model: "flash" | "pro";
  total_tokens: number;
  cost_in_cents: number;
  total_tokens_formatted: string;
  cost_formatted: string;
}

export interface ModelDailyUsagePoint {
  date: string;
  label: string;
  total_tokens: number;
  input_cache_hit_tokens: number;
  input_cache_miss_tokens: number;
  output_tokens: number;
  request_count: number;
  cost_in_cents: number;
}

export interface DashboardData {
  is_account_available: boolean;
  total_balance: number;
  granted_balance: number;
  topped_up_balance: number;
  balance_info: BalanceInfo | null;

  flash_usage: ModelUsageSummary | null;
  pro_usage: ModelUsageSummary | null;
  flash_daily_usage: ModelDailyUsagePoint[];
  pro_daily_usage: ModelDailyUsagePoint[];

  current_day_cost: number;
  current_month_cost: number;
  current_day_requests: number;
  current_day_flash_tokens: number;
  current_day_pro_tokens: number;

  has_platform_session: boolean;
  is_first_launch: boolean;
  last_updated: string;
  error_message: string | null;
  warning_message: string | null;
}

export interface ChartDataPoint {
  label: string;
  tokens: number;
  cost: number;
  cacheHit: number;
  cacheMiss: number;
  output: number;
  audioDuration: number;
  date: string;
}

// ════════════════════════════════════════════════════════════════
// MiMo Types — 从 MimoDesktopAssistant 迁移
// ════════════════════════════════════════════════════════════════

export interface MimoModelUsageSummary {
  model: "standard" | "pro";
  total_tokens: number;
  cost_in_cents: number;
  total_tokens_formatted: string;
  cost_formatted: string;
}

export interface MimoModelDailyUsagePoint {
  date: string;
  label: string;
  total_tokens: number;
  input_cache_hit_tokens: number;
  input_cache_miss_tokens: number;
  output_tokens: number;
  request_count: number;
  cost_in_cents: number;
  input_audio_duration: number;
}

export interface MimoDashboardData {
  is_account_available: boolean;
  total_balance: number;
  granted_balance: number;
  topped_up_balance: number;

  flash_usage: MimoModelUsageSummary | null;
  pro_usage: MimoModelUsageSummary | null;
  flash_daily_usage: MimoModelDailyUsagePoint[];
  pro_daily_usage: MimoModelDailyUsagePoint[];

  current_day_cost: number;
  current_month_cost: number;
  current_day_requests: number;
  current_day_flash_tokens: number;
  current_day_pro_tokens: number;
  current_day_audio_duration: number;

  plan_name: string | null;
  plan_expired: boolean | null;
  plan_period_end: string | null;
  plan_usage_percent: number | null;
  plan_used: number | null;
  plan_limit: number | null;

  has_platform_session: boolean;
  is_first_launch: boolean;
  last_updated: string;
  error_message: string | null;
  warning_message: string | null;
  payment_mode: string;
}