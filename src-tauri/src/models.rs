use serde::{Deserialize, Serialize};

use crate::api_channels::ApiChannelView;

pub const DEFAULT_INSTANCE_ID: &str = "__default__";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodexAuthMode {
    OAuth,
    Apikey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexTokens {
    pub id_token: String,
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccount {
    pub id: String,
    pub label: String,
    pub email: String,
    pub auth_mode: CodexAuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_active_until: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<CodexTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<CodexQuota>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_error: Option<CodexQuotaErrorInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuota {
    pub hourly_percentage: i32,
    pub hourly_reset_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_window_minutes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_window_present: Option<bool>,
    pub weekly_percentage: i32,
    pub weekly_reset_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekly_window_minutes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekly_window_present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_review_percentage: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_review_reset_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_review_window_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaErrorInfo {
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountView {
    pub id: String,
    pub label: String,
    pub email: String,
    pub auth_mode: CodexAuthMode,
    pub api_base_url: Option<String>,
    pub account_id: Option<String>,
    pub organization_id: Option<String>,
    pub plan_type: Option<String>,
    pub subscription_active_until: Option<i64>,
    pub has_api_key: bool,
    pub has_refresh_token: bool,
    pub quota: Option<CodexQuota>,
    pub quota_error: Option<CodexQuotaErrorInfo>,
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

impl From<&CodexAccount> for CodexAccountView {
    fn from(account: &CodexAccount) -> Self {
        Self {
            id: account.id.clone(),
            label: account.label.clone(),
            email: account.email.clone(),
            auth_mode: account.auth_mode.clone(),
            api_base_url: account.api_base_url.clone(),
            account_id: account.account_id.clone(),
            organization_id: account.organization_id.clone(),
            plan_type: account.plan_type.clone(),
            subscription_active_until: account.subscription_active_until,
            has_api_key: account
                .openai_api_key
                .as_deref()
                .map(|item| !item.trim().is_empty())
                .unwrap_or(false),
            has_refresh_token: account
                .tokens
                .as_ref()
                .and_then(|tokens| tokens.refresh_token.as_deref())
                .map(|item| !item.trim().is_empty())
                .unwrap_or(false),
            quota: account.quota.clone(),
            quota_error: account.quota_error.clone(),
            usage_updated_at: account.usage_updated_at,
            created_at: account.created_at,
            last_used: account.last_used,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexInstance {
    pub id: String,
    pub name: String,
    pub codex_home: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub extra_args: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind_account_id: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_launched_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexInstanceView {
    pub id: String,
    pub name: String,
    pub codex_home: String,
    pub working_dir: Option<String>,
    pub extra_args: String,
    pub bind_account_id: Option<String>,
    pub created_at: i64,
    pub last_launched_at: Option<i64>,
    pub last_pid: Option<u32>,
    pub running: bool,
    pub initialized: bool,
    pub is_default: bool,
    pub launch_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub version: String,
    pub current_account_id: Option<String>,
    pub accounts: Vec<CodexAccount>,
    pub instances: Vec<CodexInstance>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            current_account_id: None,
            accounts: Vec::new(),
            instances: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliStatus {
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub data_dir: String,
    pub default_codex_home: String,
    pub store_path: String,
    pub api_channels_path: String,
    pub current_account_id: Option<String>,
    pub current_api_channel_id: Option<String>,
    pub accounts: Vec<CodexAccountView>,
    pub api_channels: Vec<ApiChannelView>,
    pub instances: Vec<CodexInstanceView>,
    pub codex_cli: CodexCliStatus,
    pub windows_terminal_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddApiKeyParams {
    pub label: Option<String>,
    pub email: Option<String>,
    pub api_key: String,
    pub api_base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTokenParams {
    pub label: Option<String>,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceParams {
    pub name: String,
    pub codex_home: Option<String>,
    pub working_dir: Option<String>,
    pub extra_args: Option<String>,
    pub bind_account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceParams {
    pub instance_id: String,
    pub name: Option<String>,
    pub codex_home: Option<String>,
    pub working_dir: Option<Option<String>>,
    pub extra_args: Option<String>,
    pub bind_account_id: Option<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub instance_id: String,
    pub pid: Option<u32>,
    pub command: String,
}
