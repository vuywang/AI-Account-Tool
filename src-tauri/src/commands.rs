use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use crate::codex::{
    account_id_for_api_key, account_id_for_oauth, api_key_label, codex_cli_status,
    default_instance, extract_oauth_profile, instance_view, normalize_base_url,
    parse_auth_file_from_path, resolve_launch_instance, slugify, spawn_codex_terminal,
    upsert_account, windows_terminal_available, write_account_to_codex_home,
};
use crate::models::{
    AddApiKeyParams, AddTokenParams, AppState, CodexAccount, CodexAccountView, CodexAuthMode,
    CodexInstance, CodexQuotaErrorInfo, CreateInstanceParams, LaunchResult, UpdateInstanceParams,
    DEFAULT_INSTANCE_ID,
};
use crate::quota;
use crate::storage::{
    data_dir, default_codex_home, default_instances_root, load_store, normalize_optional,
    normalize_optional_ref, now_ts, save_store, store_path,
};

fn account_by_id<'a>(
    store: &'a crate::models::Store,
    account_id: &str,
) -> Result<&'a CodexAccount, String> {
    store
        .accounts
        .iter()
        .find(|item| item.id == account_id)
        .ok_or_else(|| format!("账号不存在: {}", account_id))
}

fn build_state() -> Result<AppState, String> {
    let data_dir = data_dir()?;
    fs::create_dir_all(&data_dir)
        .map_err(|err| format!("创建数据目录失败: {}, {}", data_dir.display(), err))?;
    let store = load_store()?;
    let mut instances = Vec::new();
    let default = default_instance(&store)?;
    instances.push(instance_view(&default, true));
    instances.extend(
        store
            .instances
            .iter()
            .map(|item| instance_view(item, false)),
    );

    Ok(AppState {
        data_dir: data_dir.to_string_lossy().to_string(),
        default_codex_home: default_codex_home()?.to_string_lossy().to_string(),
        store_path: store_path()?.to_string_lossy().to_string(),
        current_account_id: store.current_account_id.clone(),
        accounts: store.accounts.iter().map(CodexAccountView::from).collect(),
        instances,
        codex_cli: codex_cli_status(),
        windows_terminal_available: windows_terminal_available(),
    })
}

#[tauri::command]
pub fn get_app_state() -> Result<AppState, String> {
    build_state()
}

#[tauri::command]
pub fn import_current_codex_account(
    codex_home: Option<String>,
    label: Option<String>,
) -> Result<AppState, String> {
    let home = normalize_optional(codex_home)
        .map(PathBuf::from)
        .unwrap_or(default_codex_home()?);
    let account = parse_auth_file_from_path(&home.join("auth.json"), &home, label)?;
    let mut store = load_store()?;
    let account = upsert_account(&mut store, account);
    store.current_account_id = Some(account.id);
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn add_api_key_account(params: AddApiKeyParams) -> Result<AppState, String> {
    let api_key = normalize_optional_ref(Some(&params.api_key))
        .ok_or_else(|| "API Key 不能为空".to_string())?;
    let api_base_url = normalize_base_url(params.api_base_url);
    let label = normalize_optional(params.label).unwrap_or_else(|| api_key_label(&api_key));
    let email = normalize_optional(params.email).unwrap_or_else(|| label.clone());
    let now = now_ts();
    let account = CodexAccount {
        id: account_id_for_api_key(&api_key, api_base_url.as_deref()),
        label,
        email,
        auth_mode: CodexAuthMode::Apikey,
        openai_api_key: Some(api_key),
        api_base_url,
        account_id: None,
        organization_id: None,
        plan_type: Some("API_KEY".to_string()),
        tokens: None,
        quota: None,
        quota_error: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    };
    let mut store = load_store()?;
    let account = upsert_account(&mut store, account);
    store.current_account_id = Some(account.id);
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn add_token_account(params: AddTokenParams) -> Result<AppState, String> {
    let id_token = normalize_optional_ref(Some(&params.id_token))
        .ok_or_else(|| "id_token 不能为空".to_string())?;
    let access_token = normalize_optional_ref(Some(&params.access_token))
        .ok_or_else(|| "access_token 不能为空".to_string())?;
    let (email, plan_type, extracted_account_id, organization_id) =
        extract_oauth_profile(&id_token, &access_token);
    let account_id = normalize_optional(params.account_id).or(extracted_account_id);
    let now = now_ts();
    let account = CodexAccount {
        id: account_id_for_oauth(&email, account_id.as_deref(), organization_id.as_deref()),
        label: normalize_optional(params.label).unwrap_or_else(|| email.clone()),
        email,
        auth_mode: CodexAuthMode::OAuth,
        openai_api_key: None,
        api_base_url: None,
        account_id: account_id.clone(),
        organization_id,
        plan_type,
        tokens: Some(crate::models::CodexTokens {
            id_token,
            access_token,
            refresh_token: normalize_optional(params.refresh_token),
            account_id,
        }),
        quota: None,
        quota_error: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    };
    let mut store = load_store()?;
    let account = upsert_account(&mut store, account);
    store.current_account_id = Some(account.id);
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn switch_account(account_id: String, codex_home: Option<String>) -> Result<AppState, String> {
    let mut store = load_store()?;
    let home = normalize_optional(codex_home)
        .map(PathBuf::from)
        .unwrap_or(default_codex_home()?);
    let account = account_by_id(&store, &account_id)?.clone();
    write_account_to_codex_home(&home, &account)?;
    if let Some(stored) = store.accounts.iter_mut().find(|item| item.id == account_id) {
        stored.last_used = now_ts();
    }
    store.current_account_id = Some(account_id);
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn delete_account(account_id: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    store.accounts.retain(|item| item.id != account_id);
    for instance in &mut store.instances {
        if instance.bind_account_id.as_deref() == Some(&account_id) {
            instance.bind_account_id = None;
        }
    }
    if store.current_account_id.as_deref() == Some(&account_id) {
        store.current_account_id = None;
    }
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn update_account_label(account_id: String, label: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let label =
        normalize_optional_ref(Some(&label)).ok_or_else(|| "账号名称不能为空".to_string())?;
    let account = store
        .accounts
        .iter_mut()
        .find(|item| item.id == account_id)
        .ok_or_else(|| format!("账号不存在: {}", account_id))?;
    account.label = label;
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn create_instance(params: CreateInstanceParams) -> Result<AppState, String> {
    let name =
        normalize_optional_ref(Some(&params.name)).ok_or_else(|| "实例名称不能为空".to_string())?;
    let mut store = load_store()?;
    let id = format!("inst_{}", Uuid::new_v4().simple());
    let codex_home = normalize_optional(params.codex_home)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            default_instances_root()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(format!("{}-{}", slugify(&name), &id[5..13]))
        });
    fs::create_dir_all(&codex_home)
        .map_err(|err| format!("创建实例目录失败: {}, {}", codex_home.display(), err))?;

    if let Some(ref account_id) = params.bind_account_id {
        let account = account_by_id(&store, account_id)?.clone();
        write_account_to_codex_home(&codex_home, &account)?;
    }

    store.instances.push(CodexInstance {
        id,
        name,
        codex_home: codex_home.to_string_lossy().to_string(),
        working_dir: normalize_optional(params.working_dir),
        extra_args: normalize_optional(params.extra_args).unwrap_or_default(),
        bind_account_id: params.bind_account_id,
        created_at: now_ts(),
        last_launched_at: None,
        last_pid: None,
    });
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn update_instance(params: UpdateInstanceParams) -> Result<AppState, String> {
    if params.instance_id == DEFAULT_INSTANCE_ID {
        return Err("默认实例暂不支持编辑，请直接切换账号或启动".to_string());
    }

    let mut store = load_store()?;
    if let Some(ref bind) = params.bind_account_id {
        if let Some(ref account_id) = bind {
            let _ = account_by_id(&store, account_id)?;
        }
    }

    let instance = store
        .instances
        .iter_mut()
        .find(|item| item.id == params.instance_id)
        .ok_or_else(|| format!("实例不存在: {}", params.instance_id))?;

    if let Some(name) = params
        .name
        .and_then(|item| normalize_optional_ref(Some(&item)))
    {
        instance.name = name;
    }
    if let Some(codex_home) = params
        .codex_home
        .and_then(|item| normalize_optional_ref(Some(&item)))
    {
        fs::create_dir_all(&codex_home)
            .map_err(|err| format!("创建实例目录失败: {}, {}", codex_home, err))?;
        instance.codex_home = codex_home;
    }
    if let Some(working_dir) = params.working_dir {
        instance.working_dir = normalize_optional(working_dir);
    }
    if let Some(extra_args) = params.extra_args {
        instance.extra_args = extra_args.trim().to_string();
    }
    if let Some(bind_account_id) = params.bind_account_id {
        instance.bind_account_id = bind_account_id;
    }

    let codex_home = instance.codex_home.clone();
    let bind_account_id = instance.bind_account_id.clone();
    let _ = instance;

    if let Some(account_id) = bind_account_id {
        let account = account_by_id(&store, &account_id)?.clone();
        write_account_to_codex_home(Path::new(&codex_home), &account)?;
    }

    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn delete_instance(instance_id: String) -> Result<AppState, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        return Err("默认实例不能删除".to_string());
    }
    let mut store = load_store()?;
    store.instances.retain(|item| item.id != instance_id);
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn launch_instance(instance_id: String) -> Result<LaunchResult, String> {
    let mut store = load_store()?;
    let instance = resolve_launch_instance(&store, &instance_id)?;
    if let Some(ref account_id) = instance.bind_account_id {
        let account = account_by_id(&store, account_id)?.clone();
        write_account_to_codex_home(Path::new(&instance.codex_home), &account)?;
    }
    fs::create_dir_all(&instance.codex_home)
        .map_err(|err| format!("创建 CODEX_HOME 失败: {}, {}", instance.codex_home, err))?;
    let (pid, command) = spawn_codex_terminal(&instance)?;

    if instance_id != DEFAULT_INSTANCE_ID {
        if let Some(target) = store
            .instances
            .iter_mut()
            .find(|item| item.id == instance_id)
        {
            target.last_pid = pid;
            target.last_launched_at = Some(now_ts());
        }
        save_store(&store)?;
    }

    Ok(LaunchResult {
        instance_id,
        pid,
        command,
    })
}

#[tauri::command]
pub fn stop_instance(instance_id: String) -> Result<AppState, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        return Err("默认实例没有可跟踪的进程，请在终端窗口中退出".to_string());
    }

    let mut store = load_store()?;
    let instance = store
        .instances
        .iter_mut()
        .find(|item| item.id == instance_id)
        .ok_or_else(|| format!("实例不存在: {}", instance_id))?;
    if let Some(pid) = instance.last_pid {
        let _ = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .output();
    }
    instance.last_pid = None;
    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    let path = normalize_optional_ref(Some(&path)).ok_or_else(|| "路径不能为空".to_string())?;
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|err| format!("打开资源管理器失败: {}", err))?;
    Ok(())
}

#[tauri::command]
pub fn get_instance_launch_command(instance_id: String) -> Result<String, String> {
    let store = load_store()?;
    let instance = resolve_launch_instance(&store, &instance_id)?;
    Ok(crate::codex::build_launch_command(
        &instance.codex_home,
        instance.working_dir.as_deref(),
        &instance.extra_args,
    ))
}

#[tauri::command]
pub async fn refresh_account_quota(account_id: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let account = store
        .accounts
        .iter()
        .find(|item| item.id == account_id)
        .cloned()
        .ok_or_else(|| format!("账号不存在: {}", account_id))?;

    match quota::fetch_quota(&account).await {
        Ok(result) => {
            if let Some(target) = store.accounts.iter_mut().find(|item| item.id == account_id) {
                target.quota = Some(result.quota);
                target.quota_error = None;
                target.usage_updated_at = Some(now_ts());
                if result.plan_type.is_some() {
                    target.plan_type = result.plan_type;
                }
            }
        }
        Err(err) => {
            if let Some(target) = store.accounts.iter_mut().find(|item| item.id == account_id) {
                target.quota_error = Some(CodexQuotaErrorInfo {
                    message: err.clone(),
                    timestamp: now_ts(),
                });
                target.usage_updated_at = Some(now_ts());
            }
            save_store(&store)?;
            return build_state();
        }
    }

    save_store(&store)?;
    build_state()
}

#[tauri::command]
pub async fn refresh_all_quotas() -> Result<AppState, String> {
    let mut store = load_store()?;
    for index in 0..store.accounts.len() {
        let account = store.accounts[index].clone();
        if account.auth_mode == CodexAuthMode::Apikey {
            continue;
        }
        match quota::fetch_quota(&account).await {
            Ok(result) => {
                store.accounts[index].quota = Some(result.quota);
                store.accounts[index].quota_error = None;
                store.accounts[index].usage_updated_at = Some(now_ts());
                if result.plan_type.is_some() {
                    store.accounts[index].plan_type = result.plan_type;
                }
            }
            Err(err) => {
                store.accounts[index].quota_error = Some(CodexQuotaErrorInfo {
                    message: err.clone(),
                    timestamp: now_ts(),
                });
                store.accounts[index].usage_updated_at = Some(now_ts());
            }
        }
    }

    save_store(&store)?;
    build_state()
}
