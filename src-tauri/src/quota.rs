use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};

use crate::models::{CodexAccount, CodexAuthMode, CodexQuota};
use crate::storage::now_ts;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WindowInfo {
    #[serde(rename = "used_percent")]
    used_percent: Option<i32>,
    #[serde(rename = "limit_window_seconds")]
    limit_window_seconds: Option<i64>,
    #[serde(rename = "reset_after_seconds")]
    reset_after_seconds: Option<i64>,
    #[serde(rename = "reset_at")]
    reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateLimitInfo {
    #[serde(rename = "primary_window")]
    primary_window: Option<WindowInfo>,
    #[serde(rename = "secondary_window")]
    secondary_window: Option<WindowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageResponse {
    #[serde(rename = "plan_type")]
    plan_type: Option<String>,
    #[serde(rename = "rate_limit")]
    rate_limit: Option<RateLimitInfo>,
    #[serde(rename = "code_review_rate_limit")]
    code_review_rate_limit: Option<RateLimitInfo>,
}

pub struct QuotaFetchResult {
    pub quota: CodexQuota,
    pub plan_type: Option<String>,
}

fn remaining_percentage(window: &WindowInfo) -> i32 {
    100 - window.used_percent.unwrap_or(0).clamp(0, 100)
}

fn window_minutes(window: &WindowInfo) -> Option<i64> {
    let seconds = window.limit_window_seconds?;
    if seconds <= 0 {
        return None;
    }
    Some((seconds + 59) / 60)
}

fn reset_time(window: &WindowInfo) -> Option<i64> {
    if let Some(reset_at) = window.reset_at {
        return Some(reset_at);
    }
    let reset_after_seconds = window.reset_after_seconds?;
    if reset_after_seconds < 0 {
        return None;
    }
    Some(now_ts() + reset_after_seconds)
}

fn parse_quota(usage: &UsageResponse) -> CodexQuota {
    let primary = usage
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.primary_window.as_ref());
    let secondary = usage
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.secondary_window.as_ref());
    let code_review = usage
        .code_review_rate_limit
        .as_ref()
        .and_then(|limit| limit.primary_window.as_ref());

    CodexQuota {
        hourly_percentage: primary.map(remaining_percentage).unwrap_or(100),
        hourly_reset_time: primary.and_then(reset_time),
        hourly_window_minutes: primary.and_then(window_minutes),
        hourly_window_present: Some(primary.is_some()),
        weekly_percentage: secondary.map(remaining_percentage).unwrap_or(100),
        weekly_reset_time: secondary.and_then(reset_time),
        weekly_window_minutes: secondary.and_then(window_minutes),
        weekly_window_present: Some(secondary.is_some()),
        code_review_percentage: code_review.map(remaining_percentage),
        code_review_reset_time: code_review.and_then(reset_time),
        code_review_window_minutes: code_review.and_then(window_minutes),
    }
}

pub async fn fetch_quota(account: &CodexAccount) -> Result<QuotaFetchResult, String> {
    if account.auth_mode == CodexAuthMode::Apikey {
        return Err("API Key 账号不支持刷新 ChatGPT/Codex 网页额度".to_string());
    }
    let tokens = account
        .tokens
        .as_ref()
        .ok_or_else(|| "OAuth 账号缺少 tokens".to_string())?;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", tokens.access_token))
            .map_err(|err| format!("构建 Authorization 头失败: {}", err))?,
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    if let Some(account_id) = account
        .account_id
        .as_deref()
        .filter(|item| !item.trim().is_empty())
    {
        headers.insert(
            "ChatGPT-Account-Id",
            HeaderValue::from_str(account_id)
                .map_err(|err| format!("构建 ChatGPT-Account-Id 头失败: {}", err))?,
        );
    }

    let response = reqwest::Client::new()
        .get(USAGE_URL)
        .headers(headers)
        .send()
        .await
        .map_err(|err| format!("额度请求失败: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("读取额度响应失败: {}", err))?;

    if !status.is_success() {
        let preview = if body.len() > 240 {
            &body[..240]
        } else {
            &body
        };
        return Err(format!("额度接口返回 {}: {}", status, preview));
    }

    let usage: UsageResponse =
        serde_json::from_str(&body).map_err(|err| format!("解析额度响应失败: {}", err))?;
    Ok(QuotaFetchResult {
        quota: parse_quota(&usage),
        plan_type: usage.plan_type,
    })
}
