use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::storage::{
    data_dir, normalize_optional, normalize_optional_ref, now_ts, write_string_atomic,
};

const API_CHANNELS_FILE_NAME: &str = "api-channels.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiChannel {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiChannelStore {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_channel_id: Option<String>,
    pub channels: Vec<ApiChannel>,
}

impl Default for ApiChannelStore {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            current_channel_id: None,
            channels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiChannelView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub key_preview: String,
    pub has_api_key: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used: Option<i64>,
}

impl From<&ApiChannel> for ApiChannelView {
    fn from(channel: &ApiChannel) -> Self {
        Self {
            id: channel.id.clone(),
            name: channel.name.clone(),
            base_url: channel.base_url.clone(),
            key_preview: key_preview(&channel.api_key),
            has_api_key: !channel.api_key.trim().is_empty(),
            created_at: channel.created_at,
            updated_at: channel.updated_at,
            last_used: channel.last_used,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddApiChannelParams {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiChannelParams {
    pub channel_id: String,
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub fn api_channels_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(API_CHANNELS_FILE_NAME))
}

pub fn load_api_channel_store() -> Result<ApiChannelStore, String> {
    let path = api_channels_path()?;
    if !path.exists() {
        return Ok(ApiChannelStore::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|err| format!("读取 API 中转数据失败: {}, {}", path.display(), err))?;
    if content.trim().is_empty() {
        return Ok(ApiChannelStore::default());
    }
    serde_json::from_str(&content).map_err(|err| format!("解析 API 中转数据失败: {}", err))
}

pub fn save_api_channel_store(store: &ApiChannelStore) -> Result<(), String> {
    let path = api_channels_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|err| format!("序列化 API 中转数据失败: {}", err))?;
    write_string_atomic(&path, &content)
}

pub fn add_api_channel(
    store: &mut ApiChannelStore,
    params: AddApiChannelParams,
) -> Result<String, String> {
    let name = normalize_required(&params.name, "名称")?;
    let base_url = normalize_base_url(params.base_url)?;
    let api_key = normalize_required(&params.api_key, "API Key")?;
    let now = now_ts();
    let id = format!("channel_{}", Uuid::new_v4().simple());
    store.channels.push(ApiChannel {
        id: id.clone(),
        name,
        base_url,
        api_key,
        created_at: now,
        updated_at: now,
        last_used: None,
    });
    Ok(id)
}

pub fn update_api_channel(
    store: &mut ApiChannelStore,
    params: UpdateApiChannelParams,
) -> Result<(), String> {
    let channel = store
        .channels
        .iter_mut()
        .find(|item| item.id == params.channel_id)
        .ok_or_else(|| format!("API 中转不存在: {}", params.channel_id))?;

    if let Some(name) = params.name {
        channel.name = normalize_required(&name, "名称")?;
    }
    if let Some(base_url) = params.base_url {
        channel.base_url = normalize_base_url(base_url)?;
    }
    if let Some(api_key) = params.api_key {
        if let Some(value) = normalize_optional_ref(Some(&api_key)) {
            channel.api_key = value;
        }
    }
    channel.updated_at = now_ts();
    Ok(())
}

pub fn delete_api_channel(store: &mut ApiChannelStore, channel_id: &str) -> Result<(), String> {
    let before = store.channels.len();
    store.channels.retain(|item| item.id != channel_id);
    if before == store.channels.len() {
        return Err(format!("API 中转不存在: {}", channel_id));
    }
    if store.current_channel_id.as_deref() == Some(channel_id) {
        store.current_channel_id = None;
    }
    Ok(())
}

pub fn get_api_channel(store: &ApiChannelStore, channel_id: &str) -> Result<ApiChannel, String> {
    store
        .channels
        .iter()
        .find(|item| item.id == channel_id)
        .cloned()
        .ok_or_else(|| format!("API 中转不存在: {}", channel_id))
}

pub fn mark_api_channel_used(store: &mut ApiChannelStore, channel_id: &str) -> Result<(), String> {
    let now = now_ts();
    let channel = store
        .channels
        .iter_mut()
        .find(|item| item.id == channel_id)
        .ok_or_else(|| format!("API 中转不存在: {}", channel_id))?;
    channel.last_used = Some(now);
    channel.updated_at = now;
    store.current_channel_id = Some(channel_id.to_string());
    Ok(())
}

pub fn clear_current_api_channel(store: &mut ApiChannelStore) {
    store.current_channel_id = None;
}

fn normalize_required(input: &str, label: &str) -> Result<String, String> {
    normalize_optional_ref(Some(input)).ok_or_else(|| format!("{}不能为空", label))
}

fn normalize_base_url(input: String) -> Result<String, String> {
    normalize_optional(Some(input))
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Base URL 不能为空".to_string())
}

fn key_preview(api_key: &str) -> String {
    let value = api_key.trim();
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "****".to_string();
    }
    let head = chars.iter().take(4).collect::<String>();
    let tail = chars.iter().rev().take(4).collect::<Vec<_>>();
    let tail = tail.into_iter().rev().collect::<String>();
    format!("{}****{}", head, tail)
}
