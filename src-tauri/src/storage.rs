use chrono::Utc;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::Store;

const APP_DIR_NAME: &str = "AI Account Tool";
const STORE_FILE_NAME: &str = "store.json";

pub fn now_ts() -> i64 {
    Utc::now().timestamp()
}

pub fn data_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or_else(|| "无法定位 AppData\\Local 目录".to_string())
        .map(|dir| dir.join(APP_DIR_NAME))
}

pub fn store_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(STORE_FILE_NAME))
}

pub fn default_codex_home() -> Result<PathBuf, String> {
    if let Ok(raw) = env::var("CODEX_HOME") {
        let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    dirs::home_dir()
        .ok_or_else(|| "无法定位用户目录".to_string())
        .map(|dir| dir.join(".codex"))
}

pub fn default_instances_root() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("codex-instances"))
}

pub fn write_string_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法定位父目录: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("创建目录失败: {}, {}", parent.display(), err))?;
    let tmp = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&tmp, content)
        .map_err(|err| format!("写入临时文件失败: {}, {}", tmp.display(), err))?;
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!("替换文件失败: {}, {}", path.display(), err)
    })
}

pub fn load_store() -> Result<Store, String> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(Store::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|err| format!("读取数据文件失败: {}, {}", path.display(), err))?;
    if content.trim().is_empty() {
        return Ok(Store::default());
    }
    serde_json::from_str(&content).map_err(|err| format!("解析数据文件失败: {}", err))
}

pub fn save_store(store: &Store) -> Result<(), String> {
    let path = store_path()?;
    let content =
        serde_json::to_string_pretty(store).map_err(|err| format!("序列化数据失败: {}", err))?;
    write_string_atomic(&path, &content)
}

pub fn sha256_short(input: &str, len: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{:x}", digest);
    hex.chars().take(len).collect()
}

pub fn normalize_optional(input: Option<String>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn normalize_optional_ref(input: Option<&str>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
