use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use sysinfo::{Pid, System};
use toml_edit::{value, Document};

use crate::models::{
    CodexAccount, CodexAuthMode, CodexCliStatus, CodexInstance, CodexInstanceView, CodexTokens,
    Store, DEFAULT_INSTANCE_ID,
};
use crate::storage::{
    default_codex_home, normalize_optional, normalize_optional_ref, now_ts, sha256_short,
    write_string_atomic,
};

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_OAUTH_TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAuthFile {
    #[serde(default)]
    auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    #[serde(default)]
    openai_api_key: Option<Value>,
    #[serde(default, alias = "api_base_url", alias = "apiBaseUrl")]
    base_url: Option<String>,
    #[serde(default)]
    tokens: Option<CodexAuthTokens>,
}

#[derive(Debug, Deserialize)]
struct CodexAuthTokens {
    id_token: String,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
}

pub fn normalize_base_url(input: Option<String>) -> Option<String> {
    normalize_optional(input).map(|value| value.trim_end_matches('/').to_string())
}

fn is_api_key_mode(input: Option<&str>) -> bool {
    input
        .map(|value| value.trim().eq_ignore_ascii_case("apikey"))
        .unwrap_or(false)
}

fn extract_api_key(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
}

fn parse_jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn auth_payload_value<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get("https://api.openai.com/auth")
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
}

fn normalize_timestamp(raw: i64) -> Option<i64> {
    if raw <= 0 {
        return None;
    }
    if raw > 10_000_000_000 {
        return Some(raw / 1000);
    }
    Some(raw)
}

fn parse_timestamp_text(raw: &str) -> Option<i64> {
    let value = normalize_optional_ref(Some(raw))?;
    if let Ok(timestamp) = value.parse::<i64>() {
        return normalize_timestamp(timestamp);
    }
    DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|item| item.timestamp())
}

fn auth_payload_timestamp(payload: &Value, key: &str) -> Option<i64> {
    let value = payload
        .get("https://api.openai.com/auth")
        .and_then(|item| item.get(key))?;
    value
        .as_i64()
        .and_then(normalize_timestamp)
        .or_else(|| value.as_str().and_then(parse_timestamp_text))
}

pub fn extract_oauth_profile(
    id_token: &str,
    access_token: &str,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
) {
    let id_payload = parse_jwt_payload(id_token);
    let access_payload = parse_jwt_payload(access_token);

    let email = id_payload
        .as_ref()
        .and_then(|payload| payload.get("email"))
        .and_then(|value| value.as_str())
        .and_then(|value| normalize_optional_ref(Some(value)))
        .unwrap_or_else(|| "unknown-codex-account".to_string());

    let plan_type = id_payload
        .as_ref()
        .and_then(|payload| auth_payload_value(payload, "chatgpt_plan_type"))
        .and_then(|value| normalize_optional_ref(Some(value)));

    let account_id = id_payload
        .as_ref()
        .and_then(|payload| {
            auth_payload_value(payload, "account_id")
                .or_else(|| auth_payload_value(payload, "chatgpt_account_id"))
        })
        .or_else(|| {
            access_payload.as_ref().and_then(|payload| {
                auth_payload_value(payload, "account_id")
                    .or_else(|| auth_payload_value(payload, "chatgpt_account_id"))
            })
        })
        .and_then(|value| normalize_optional_ref(Some(value)));

    let organization_id = id_payload
        .as_ref()
        .and_then(|payload| {
            auth_payload_value(payload, "organization_id")
                .or_else(|| auth_payload_value(payload, "chatgpt_organization_id"))
                .or_else(|| auth_payload_value(payload, "org_id"))
        })
        .or_else(|| {
            access_payload.as_ref().and_then(|payload| {
                auth_payload_value(payload, "organization_id")
                    .or_else(|| auth_payload_value(payload, "chatgpt_organization_id"))
                    .or_else(|| auth_payload_value(payload, "org_id"))
            })
        })
        .and_then(|value| normalize_optional_ref(Some(value)));

    let subscription_active_until = id_payload
        .as_ref()
        .and_then(|payload| auth_payload_timestamp(payload, "chatgpt_subscription_active_until"))
        .or_else(|| {
            access_payload.as_ref().and_then(|payload| {
                auth_payload_timestamp(payload, "chatgpt_subscription_active_until")
            })
        });

    (
        email,
        plan_type,
        account_id,
        organization_id,
        subscription_active_until,
    )
}

pub fn sync_account_metadata_from_tokens(account: &mut CodexAccount) -> bool {
    if account.auth_mode != CodexAuthMode::OAuth {
        return false;
    }
    let Some(tokens) = account.tokens.as_ref() else {
        return false;
    };
    let (email, plan_type, account_id, organization_id, subscription_active_until) =
        extract_oauth_profile(&tokens.id_token, &tokens.access_token);
    let mut changed = false;

    if account.email != email && email != "unknown-codex-account" {
        account.email = email;
        changed = true;
    }
    if account.plan_type != plan_type {
        account.plan_type = plan_type;
        changed = true;
    }
    if account.account_id != account_id {
        account.account_id = account_id.clone();
        changed = true;
    }
    if account.organization_id != organization_id {
        account.organization_id = organization_id;
        changed = true;
    }
    if account.subscription_active_until != subscription_active_until {
        account.subscription_active_until = subscription_active_until;
        changed = true;
    }
    if let Some(tokens) = account.tokens.as_mut() {
        if tokens.account_id != account_id {
            tokens.account_id = account_id;
            changed = true;
        }
    }

    changed
}

pub fn api_key_label(api_key: &str) -> String {
    let suffix = api_key.chars().rev().take(4).collect::<String>();
    let suffix = suffix.chars().rev().collect::<String>();
    format!("API Key {}", suffix)
}

pub fn account_id_for_oauth(
    email: &str,
    account_id: Option<&str>,
    organization_id: Option<&str>,
) -> String {
    format!(
        "oauth_{}",
        sha256_short(
            &format!(
                "{}|{}|{}",
                email.to_ascii_lowercase(),
                account_id.unwrap_or(""),
                organization_id.unwrap_or("")
            ),
            18
        )
    )
}

pub fn account_id_for_api_key(api_key: &str, base_url: Option<&str>) -> String {
    format!(
        "apikey_{}",
        sha256_short(
            &format!("{}|{}", api_key.trim(), base_url.unwrap_or("")),
            18
        )
    )
}

fn read_api_base_url_from_config(codex_home: &Path) -> Option<String> {
    let path = codex_home.join("config.toml");
    let content = fs::read_to_string(path).ok()?;
    let doc = content.parse::<Document>().ok()?;
    if let Some(url) = doc
        .get("openai_base_url")
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
    {
        return Some(url.trim_end_matches('/').to_string());
    }

    let provider_id = doc
        .get("model_provider")
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))?;

    doc.get("model_providers")
        .and_then(|item| item.get(provider_id.as_str()))
        .and_then(|item| item.get("base_url"))
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
        .map(|item| item.trim_end_matches('/').to_string())
}

fn write_openai_config(
    codex_home: &Path,
    base_url: Option<&str>,
    force_openai_provider: bool,
) -> Result<(), String> {
    let path = codex_home.join("config.toml");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut doc = if existing.trim().is_empty() {
        Document::new()
    } else {
        existing
            .parse::<Document>()
            .map_err(|err| format!("解析 config.toml 失败: {}", err))?
    };

    if force_openai_provider {
        doc["model_provider"] = value("openai");
    }

    match base_url
        .map(|item| item.trim().trim_end_matches('/').to_string())
        .filter(|item| !item.is_empty() && item != DEFAULT_OPENAI_BASE_URL)
    {
        Some(url) => {
            doc["openai_base_url"] = value(url);
        }
        None => {
            let _ = doc.remove("openai_base_url");
        }
    }

    if doc.to_string().trim().is_empty() && !path.exists() {
        return Ok(());
    }

    write_string_atomic(&path, &doc.to_string())
}

fn write_api_provider_to_config(codex_home: &Path, account: &CodexAccount) -> Result<(), String> {
    if account.auth_mode == CodexAuthMode::Apikey {
        let base_url = account
            .api_base_url
            .as_deref()
            .map(|item| item.trim_end_matches('/').to_string())
            .filter(|item| !item.is_empty() && item != DEFAULT_OPENAI_BASE_URL);
        return write_openai_config(codex_home, base_url.as_deref(), base_url.is_some());
    } else {
        return write_openai_config(codex_home, None, false);
    }
}

fn build_auth_json(account: &CodexAccount) -> Result<Value, String> {
    match account.auth_mode {
        CodexAuthMode::Apikey => {
            let api_key = account
                .openai_api_key
                .as_deref()
                .and_then(|value| normalize_optional_ref(Some(value)))
                .ok_or_else(|| "API Key 账号缺少 OPENAI_API_KEY".to_string())?;
            Ok(json!({
                "auth_mode": "apikey",
                "OPENAI_API_KEY": api_key
            }))
        }
        CodexAuthMode::OAuth => {
            let tokens = account
                .tokens
                .as_ref()
                .ok_or_else(|| "OAuth 账号缺少 tokens".to_string())?;
            if tokens.id_token.trim().is_empty() || tokens.access_token.trim().is_empty() {
                return Err("OAuth 账号缺少 id_token/access_token".to_string());
            }
            Ok(json!({
                "OPENAI_API_KEY": Value::Null,
                "tokens": {
                    "id_token": tokens.id_token,
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "account_id": account.account_id.as_ref().or(tokens.account_id.as_ref())
                },
                "last_refresh": Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
            }))
        }
    }
}

pub fn write_account_to_codex_home(
    codex_home: &Path,
    account: &CodexAccount,
) -> Result<(), String> {
    fs::create_dir_all(codex_home)
        .map_err(|err| format!("创建 CODEX_HOME 失败: {}, {}", codex_home.display(), err))?;
    let auth_path = codex_home.join("auth.json");
    let auth_content = serde_json::to_string_pretty(&build_auth_json(account)?)
        .map_err(|err| format!("序列化 auth.json 失败: {}", err))?;
    write_string_atomic(&auth_path, &auth_content)?;
    write_api_provider_to_config(codex_home, account)
}

pub fn write_api_channel_to_codex_home(
    codex_home: &Path,
    api_key: &str,
    base_url: &str,
) -> Result<(), String> {
    fs::create_dir_all(codex_home)
        .map_err(|err| format!("创建 CODEX_HOME 失败: {}, {}", codex_home.display(), err))?;
    let api_key = normalize_optional_ref(Some(api_key))
        .ok_or_else(|| "API 中转缺少 OPENAI_API_KEY".to_string())?;
    let base_url = normalize_optional_ref(Some(base_url))
        .ok_or_else(|| "API 中转缺少 Base URL".to_string())?;
    let auth_content = serde_json::to_string_pretty(&json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": api_key,
    }))
    .map_err(|err| format!("序列化 auth.json 失败: {}", err))?;
    write_string_atomic(&codex_home.join("auth.json"), &auth_content)?;
    write_openai_config(codex_home, Some(&base_url), true)
}

pub fn parse_auth_file_from_path(
    path: &Path,
    codex_home: &Path,
    label: Option<String>,
) -> Result<CodexAccount, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("读取 auth.json 失败: {}, {}", path.display(), err))?;
    let auth_file: CodexAuthFile =
        serde_json::from_str(&content).map_err(|err| format!("解析 auth.json 失败: {}", err))?;
    let now = now_ts();

    if is_api_key_mode(auth_file.auth_mode.as_deref()) || auth_file.tokens.is_none() {
        let api_key = extract_api_key(auth_file.openai_api_key.as_ref())
            .ok_or_else(|| "auth.json 缺少 OPENAI_API_KEY 或 OAuth tokens".to_string())?;
        let base_url = normalize_base_url(auth_file.base_url)
            .or_else(|| read_api_base_url_from_config(codex_home));
        let fallback_label = api_key_label(&api_key);
        return Ok(CodexAccount {
            id: account_id_for_api_key(&api_key, base_url.as_deref()),
            label: normalize_optional(label).unwrap_or(fallback_label.clone()),
            email: fallback_label,
            auth_mode: CodexAuthMode::Apikey,
            openai_api_key: Some(api_key),
            api_base_url: base_url,
            account_id: None,
            organization_id: None,
            plan_type: Some("API_KEY".to_string()),
            subscription_active_until: None,
            tokens: None,
            quota: None,
            quota_error: None,
            usage_updated_at: None,
            created_at: now,
            last_used: now,
        });
    }

    let tokens = auth_file
        .tokens
        .ok_or_else(|| "auth.json 缺少 OAuth tokens".to_string())?;
    let (email, plan_type, extracted_account_id, organization_id, subscription_active_until) =
        extract_oauth_profile(&tokens.id_token, &tokens.access_token);
    let account_id = normalize_optional(tokens.account_id.clone()).or(extracted_account_id);
    let id = account_id_for_oauth(&email, account_id.as_deref(), organization_id.as_deref());
    Ok(CodexAccount {
        id,
        label: normalize_optional(label).unwrap_or_else(|| email.clone()),
        email,
        auth_mode: CodexAuthMode::OAuth,
        openai_api_key: None,
        api_base_url: None,
        account_id,
        organization_id,
        plan_type,
        subscription_active_until,
        tokens: Some(CodexTokens {
            id_token: tokens.id_token,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            account_id: tokens.account_id,
        }),
        quota: None,
        quota_error: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    })
}

pub fn upsert_account(store: &mut Store, mut account: CodexAccount) -> CodexAccount {
    if let Some(existing) = store.accounts.iter_mut().find(|item| item.id == account.id) {
        account.created_at = existing.created_at;
        *existing = account.clone();
    } else {
        store.accounts.push(account.clone());
    }
    account
}

pub async fn refresh_oauth_tokens(
    refresh_token: &str,
    account_id: Option<String>,
) -> Result<CodexTokens, String> {
    let response = reqwest::Client::new()
        .post(CODEX_OAUTH_TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|err| format!("Token 刷新请求失败: {}", err))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("读取 Token 刷新响应失败: {}", err))?;

    if !status.is_success() {
        let preview = body.chars().take(240).collect::<String>();
        return Err(format!("Token 刷新失败: {}, {}", status, preview));
    }

    let value: Value =
        serde_json::from_str(&body).map_err(|err| format!("解析 Token 刷新响应失败: {}", err))?;
    let id_token = value
        .get("id_token")
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
        .ok_or_else(|| "Token 刷新响应缺少 id_token".to_string())?;
    let access_token = value
        .get("access_token")
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
        .ok_or_else(|| "Token 刷新响应缺少 access_token".to_string())?;
    let next_refresh_token = value
        .get("refresh_token")
        .and_then(|item| item.as_str())
        .and_then(|item| normalize_optional_ref(Some(item)))
        .or_else(|| Some(refresh_token.to_string()));

    Ok(CodexTokens {
        id_token,
        access_token,
        refresh_token: next_refresh_token,
        account_id,
    })
}

fn process_running(pid: Option<u32>) -> bool {
    let Some(pid) = pid else {
        return false;
    };
    let mut system = System::new_all();
    system.refresh_all();
    system.process(Pid::from_u32(pid)).is_some()
}

fn is_initialized(codex_home: &str) -> bool {
    let path = Path::new(codex_home);
    path.join("auth.json").exists()
        || path.join("config.toml").exists()
        || path.join("sessions").exists()
}

#[cfg(windows)]
fn hidden_output(command: &mut Command) -> std::io::Result<Output> {
    use std::os::windows::process::CommandExt;

    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    command.output()
}

#[cfg(not(windows))]
fn hidden_output(command: &mut Command) -> std::io::Result<Output> {
    command.output()
}

#[cfg(windows)]
fn hidden_spawn(command: &mut Command) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;

    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    command.spawn()
}

#[cfg(not(windows))]
fn hidden_spawn(command: &mut Command) -> std::io::Result<Child> {
    command.spawn()
}

pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let mut command = Command::new("where.exe");
    command.arg(name);
    let output = hidden_output(&mut command).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.exists())
}

pub fn codex_cli_status() -> CodexCliStatus {
    let appdata_path = env::var("APPDATA")
        .ok()
        .map(|appdata| PathBuf::from(appdata).join("npm").join("codex.cmd"))
        .filter(|path| path.exists());

    let (path, source) = if let Some(path) = appdata_path {
        (Some(path), "npm-global".to_string())
    } else {
        (
            find_on_path("codex.cmd").or_else(|| find_on_path("codex.exe")),
            "path".to_string(),
        )
    };

    let version = path.as_ref().and_then(|path| {
        let mut command = Command::new(path);
        command.arg("--version");
        hidden_output(&mut command)
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|output| !output.is_empty())
    });

    CodexCliStatus {
        path: path.map(|item| item.to_string_lossy().to_string()),
        version,
        source,
    }
}

pub fn windows_terminal_available() -> bool {
    find_on_path("wt.exe").is_some()
}

pub fn build_launch_script(
    codex_home: &str,
    working_dir: Option<&str>,
    extra_args: &str,
    cli_path: &str,
) -> String {
    let mut pieces = Vec::new();
    if let Some(dir) = working_dir.and_then(|value| normalize_optional_ref(Some(value))) {
        pieces.push(format!("cd /d \"{}\"", dir.replace('"', "")));
    }
    pieces.push(format!(
        "set \"CODEX_HOME={}\"",
        codex_home.replace('"', "")
    ));
    let mut codex = format!("\"{}\"", cli_path.replace('"', ""));
    let extra = extra_args.trim();
    if !extra.is_empty() {
        codex.push(' ');
        codex.push_str(extra);
    }
    pieces.push(codex);
    pieces.join(" && ")
}

pub fn build_launch_command(
    codex_home: &str,
    working_dir: Option<&str>,
    extra_args: &str,
) -> String {
    let cli = codex_cli_status()
        .path
        .unwrap_or_else(|| "%APPDATA%\\npm\\codex.cmd".to_string());
    format!(
        "cmd.exe /d /k {}",
        build_launch_script(codex_home, working_dir, extra_args, &cli)
    )
}

pub fn instance_view(instance: &CodexInstance, is_default: bool) -> CodexInstanceView {
    CodexInstanceView {
        id: instance.id.clone(),
        name: instance.name.clone(),
        codex_home: instance.codex_home.clone(),
        working_dir: instance.working_dir.clone(),
        extra_args: instance.extra_args.clone(),
        bind_account_id: instance.bind_account_id.clone(),
        created_at: instance.created_at,
        last_launched_at: instance.last_launched_at,
        last_pid: instance.last_pid,
        running: process_running(instance.last_pid),
        initialized: is_initialized(&instance.codex_home),
        is_default,
        launch_command: build_launch_command(
            &instance.codex_home,
            instance.working_dir.as_deref(),
            &instance.extra_args,
        ),
    }
}

pub fn default_instance(store: &Store) -> Result<CodexInstance, String> {
    Ok(CodexInstance {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: "默认 Codex".to_string(),
        codex_home: default_codex_home()?.to_string_lossy().to_string(),
        working_dir: None,
        extra_args: String::new(),
        bind_account_id: store.current_account_id.clone(),
        created_at: 0,
        last_launched_at: None,
        last_pid: None,
    })
}

pub fn resolve_launch_instance(store: &Store, instance_id: &str) -> Result<CodexInstance, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        return default_instance(store);
    }
    store
        .instances
        .iter()
        .find(|item| item.id == instance_id)
        .cloned()
        .ok_or_else(|| format!("实例不存在: {}", instance_id))
}

pub fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "codex-instance".to_string()
    } else {
        slug
    }
}

pub fn spawn_codex_terminal(instance: &CodexInstance) -> Result<(Option<u32>, String), String> {
    let cli = codex_cli_status()
        .path
        .ok_or_else(|| "未找到 Codex CLI，请先安装 npm 版 @openai/codex".to_string())?;
    let script = build_launch_script(
        &instance.codex_home,
        instance.working_dir.as_deref(),
        &instance.extra_args,
        &cli,
    );
    let title = format!("Codex - {}", instance.name);

    if windows_terminal_available() {
        let mut command = Command::new("wt.exe");
        command
            .arg("new-tab")
            .arg("--title")
            .arg(&title)
            .arg("cmd.exe")
            .arg("/d")
            .arg("/k")
            .arg(&script);
        if let Some(ref working_dir) = instance.working_dir {
            command.current_dir(working_dir);
        }
        let child = command
            .spawn()
            .map_err(|err| format!("启动 Windows Terminal 失败: {}", err))?;
        return Ok((
            Some(child.id()),
            format!(
                "wt.exe new-tab --title \"{}\" cmd.exe /d /k {}",
                title, script
            ),
        ));
    }

    let mut command = Command::new("cmd.exe");
    command.arg("/d").arg("/k").arg(&script);
    if let Some(ref working_dir) = instance.working_dir {
        command.current_dir(working_dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE);
    }
    let child = command
        .spawn()
        .map_err(|err| format!("启动 cmd.exe 失败: {}", err))?;
    Ok((Some(child.id()), format!("cmd.exe /d /k {}", script)))
}

pub fn spawn_codex_login(codex_home: &Path) -> Result<(), String> {
    let cli = codex_cli_status()
        .path
        .ok_or_else(|| "未找到 Codex CLI，请先安装 npm 版 @openai/codex".to_string())?;
    fs::create_dir_all(codex_home)
        .map_err(|err| format!("创建 CODEX_HOME 失败: {}, {}", codex_home.display(), err))?;

    let mut command = Command::new(&cli);
    command
        .arg("login")
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _child =
        hidden_spawn(&mut command).map_err(|err| format!("启动 Codex 登录失败: {}", err))?;
    Ok(())
}
