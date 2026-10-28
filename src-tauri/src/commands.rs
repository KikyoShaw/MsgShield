//! Tauri commands：前端 ↔ Rust 引擎的边界

use crate::apps::{self, APP_KEYS};
use tauri::Manager;
use crate::error::{PatchError, PatchResult};
use crate::modifier::{AppModifier, SupportState};
use crate::patch_data::{Bag, BUNDLED_PATCH_JSON, REMOTE_URL_TEMPLATES};
use crate::path_util;
use serde::Serialize;
use std::sync::{Arc, RwLock};

/// 全局状态：当前生效的补丁数据（内置基线，可被远程热替换）
/// 用 Arc 以便克隆进 spawn_blocking 线程池执行重 IO
pub struct PatcherState {
    pub bag: Arc<RwLock<Bag>>,
    /// 远程刷新状态（四态，对齐原版 lblUpdatePachJson）
    pub remote_status: Arc<RwLock<RemoteStatus>>,
}

/// 远程补丁数据状态
#[derive(Debug, Clone, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteStatus {
    /// 尚未刷新
    #[default]
    Pending,
    /// 全部镜像失败
    Failed,
    /// 存在更新的补丁工具版本（红字提示）
    NewVersionAvailable(String),
    /// 补丁数据已热更新
    Updated,
    /// 内置已是最新
    UpToDate,
}

#[derive(Debug, Serialize)]
pub struct DetectedApp {
    pub app_key: String,
    #[serde(rename = "installPath")]
    pub install_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub version: String,
    /// "exact" | "fuzzy" | "unsupported"
    pub state: String,
    /// 特征码模式下的功能列表（精确/不支持为空）
    pub categories: Vec<CategoryDto>,
    #[serde(rename = "backupExists")]
    pub backup_exists: bool,
    #[serde(rename = "allFilesExist")]
    pub all_files_exist: bool,
}

#[derive(Debug, Serialize)]
pub struct CategoryDto {
    pub name: String,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct SupportedVersions {
    #[serde(rename = "appKey")]
    pub app_key: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PatchDataInfo {
    #[serde(rename = "patchVersion")]
    pub patch_version: i64,
    #[serde(rename = "latestVersion")]
    pub latest_version: String,
    pub notice: String,
    #[serde(rename = "noticeUrl")]
    pub notice_url: String,
    pub remote: RemoteStatus,
}

/// 恢复结果：版本不一致需要前端确认后 force 重调
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestoreOutcome {
    Restored,
    NeedConfirm {
        file: String,
        #[serde(rename = "currentVersion")]
        current_version: String,
        #[serde(rename = "backupVersion")]
        backup_version: String,
    },
}

fn with_state<R>(app: &tauri::AppHandle, f: impl FnOnce(&PatcherState) -> R) -> R {
    f(app.state::<PatcherState>().inner())
}

/// 把闭包丢到线程池执行（重 IO：磁盘扫描/整文件读入/特征搜索），避免阻塞主线程卡 UI
async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> PatchResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| PatchError::business("join", format!("任务执行失败：{e}")))
}

/// 6 应用自动探测安装路径（扫盘 + 注册表，重 IO）
#[tauri::command]
pub async fn detect_apps(app: tauri::AppHandle) -> Vec<DetectedApp> {
    let bag = app.state::<PatcherState>().inner().bag.clone();
    run_blocking(move || {
        let bag = bag.read().unwrap();
        APP_KEYS
            .iter()
            .map(|&key| {
                let def = apps::app_def(key).unwrap();
                let install_path = bag
                    .apps
                    .get(key)
                    .and_then(|config| path_util::find_install_path(def, config));
                DetectedApp {
                    app_key: key.to_string(),
                    install_path,
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// 应用状态：版本三态 + 功能列表 + 备份状态（整文件读入 + 特征码扫描，重 IO）
#[tauri::command]
pub async fn get_app_status(
    app: tauri::AppHandle,
    app_key: String,
    path: String,
) -> PatchResult<AppStatus> {
    let bag = app.state::<PatcherState>().inner().bag.clone();
    run_blocking(move || {
        let bag = bag.read().unwrap();
        let def = apps::app_def(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("未知应用：{app_key}")))?;
        let config = bag
            .apps
            .get(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("补丁数据中无应用：{app_key}")))?;

        let all_files_exist = path_util::is_all_files_exist(config, &path);
        if !all_files_exist {
            return Ok(AppStatus {
                version: String::new(),
                state: "no_files".into(),
                categories: vec![],
                backup_exists: false,
                all_files_exist: false,
            });
        }

        let modifier = AppModifier::new(def, config, &path)?;
        let (support, categories) = modifier.support_state();
        Ok(AppStatus {
            version: modifier.version(),
            state: match support {
                SupportState::Exact => "exact",
                SupportState::Fuzzy => "fuzzy",
                SupportState::Unsupported => "unsupported",
            }
            .into(),
            categories: categories
                .into_iter()
                .map(|c| CategoryDto {
                    name: c.name,
                    installed: c.installed,
                })
                .collect(),
            backup_exists: modifier.backup_exists(),
            all_files_exist: true,
        })
    })
    .await?
}

/// 安装补丁。categories=None 表示精确模式；Some(空) 由前端拦截
#[tauri::command]
pub async fn apply_patch(
    app: tauri::AppHandle,
    app_key: String,
    path: String,
    categories: Option<Vec<String>>,
) -> PatchResult<()> {
    let bag = app.state::<PatcherState>().inner().bag.clone();
    run_blocking(move || {
        let bag = bag.read().unwrap();
        let def = apps::app_def(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("未知应用：{app_key}")))?;
        let config = bag
            .apps
            .get(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("补丁数据中无应用：{app_key}")))?;

        if !path_util::is_all_files_exist(config, &path) {
            return Err(PatchError::business("bad_path", "请选择正确的安装路径!"));
        }

        let mut modifier = AppModifier::new(def, config, &path)?;
        modifier.validate_and_find_modify_info(categories.as_deref())?;
        modifier.patch()
    })
    .await?
}

/// 恢复备份。版本不一致时返回 NeedConfirm，前端确认后 force=true 重调（文件复制，重 IO）
#[tauri::command]
pub async fn restore_backup(
    app: tauri::AppHandle,
    app_key: String,
    path: String,
    force: bool,
) -> PatchResult<RestoreOutcome> {
    let bag = app.state::<PatcherState>().inner().bag.clone();
    run_blocking(move || {
        let bag = bag.read().unwrap();
        let def = apps::app_def(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("未知应用：{app_key}")))?;
        let config = bag
            .apps
            .get(&app_key)
            .ok_or_else(|| PatchError::business("bad_app", format!("补丁数据中无应用：{app_key}")))?;

        if !path_util::is_all_files_exist(config, &path) {
            return Err(PatchError::business("bad_path", "请选择正确的安装路径!"));
        }

        let modifier = AppModifier::new(def, config, &path)?;

        // 版本不一致检查（非 force 时逐文件请求确认——返回第一个不一致项）
        if !force {
            for editor in &modifier.editors {
                let current = editor.file_version();
                let backup = editor.backup_file_version();
                if editor.file_bak_path.is_file() && current != backup {
                    return Ok(RestoreOutcome::NeedConfirm {
                        file: editor.file_path.to_string_lossy().to_string(),
                        current_version: current,
                        backup_version: backup,
                    });
                }
            }
        }

        for editor in &modifier.editors {
            if editor.file_bak_path.is_file() {
                editor.restore()?;
            }
        }
        Ok(RestoreOutcome::Restored)
    })
    .await?
}

/// 各应用支持的版本列表（精确 ∪ 特征码区间）
#[tauri::command]
pub fn get_supported_versions(app: tauri::AppHandle) -> Vec<SupportedVersions> {
    with_state(&app, |state| {
        let bag = state.bag.read().unwrap();
        APP_KEYS
            .iter()
            .map(|&key| {
                let mut versions: Vec<String> = Vec::new();
                if let Some(config) = bag.apps.get(key) {
                    for infos in config.file_modify_infos.values() {
                        for info in infos {
                            if !versions.contains(&info.version) {
                                versions.push(info.version.clone());
                            }
                        }
                    }
                    for commons in config.file_common_modify_infos.values() {
                        for common in commons {
                            let label = if common.end_version.as_deref().unwrap_or("").is_empty() {
                                format!("{} 以上（特征）", common.start_version)
                            } else {
                                format!(
                                    "{} ~ {}（特征）",
                                    common.start_version,
                                    common.end_version.clone().unwrap_or_default()
                                )
                            };
                            if !versions.contains(&label) {
                                versions.push(label);
                            }
                        }
                    }
                }
                versions.sort_by(|a, b| match path_util::version_compare(a, b) {
                    1 => std::cmp::Ordering::Greater,
                    -1 => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                });
                SupportedVersions {
                    app_key: key.to_string(),
                    versions,
                }
            })
            .collect()
    })
}

/// 当前补丁数据信息（含远程刷新状态）
#[tauri::command]
pub fn get_patch_data_info(app: tauri::AppHandle) -> PatchDataInfo {
    with_state(&app, |state| {
        let bag = state.bag.read().unwrap();
        let remote = state.remote_status.read().unwrap().clone();
        PatchDataInfo {
            patch_version: bag.patch_version,
            latest_version: bag.latest_version.clone(),
            notice: bag.notice.clone(),
            notice_url: bag.notice_url.clone(),
            remote,
        }
    })
}

/// 远程刷新补丁数据：5 URL 镜像链 + PatchVersion 单调热替换
#[tauri::command]
pub async fn refresh_patch_data(app: tauri::AppHandle) -> Result<PatchDataInfo, String> {
    let (major_minor, current_patch_version) = {
        let state = app.state::<PatcherState>();
        let state = state.inner();
        let bag = state.bag.read().unwrap();
        let mm = bag
            .latest_version
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        (mm, bag.patch_version)
    };

    // 依次尝试镜像链
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))?;

    let mut fetched: Option<Bag> = None;
    for template in REMOTE_URL_TEMPLATES {
        let url = template.replace("{ver}", &major_minor);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.text().await {
                    if json.contains("LatestVersion") {
                        if let Ok(bag) = Bag::parse(&json) {
                            fetched = Some(bag);
                            break;
                        }
                    }
                }
            }
            _ => continue,
        }
    }

    let state = app.state::<PatcherState>();
    let state = state.inner();
    let mut bag_lock = state.bag.write().unwrap();
    let mut remote_lock = state.remote_status.write().unwrap();

    let Some(new_bag) = fetched else {
        *remote_lock = RemoteStatus::Failed;
        return Ok(current_info(&bag_lock, &remote_lock));
    };

    // 存在更新的补丁工具版本（红字提示，不替换数据）
    let new_tool_version = version_gt(&new_bag.latest_version, &bag_lock.latest_version);
    if new_tool_version {
        *remote_lock = RemoteStatus::NewVersionAvailable(new_bag.latest_version.clone());
        return Ok(current_info(&bag_lock, &remote_lock));
    }

    // PatchVersion 更大 → 热替换
    if new_bag.patch_version > current_patch_version {
        *bag_lock = new_bag;
        *remote_lock = RemoteStatus::Updated;
    } else {
        *remote_lock = RemoteStatus::UpToDate;
    }
    Ok(current_info(&bag_lock, &remote_lock))
}

fn current_info(bag: &Bag, remote: &RemoteStatus) -> PatchDataInfo {
    PatchDataInfo {
        patch_version: bag.patch_version,
        latest_version: bag.latest_version.clone(),
        notice: bag.notice.clone(),
        notice_url: bag.notice_url.clone(),
        remote: remote.clone(),
    }
}

/// 简单版本大小比较（点分段数字）
fn version_gt(a: &str, b: &str) -> bool {
    let pa: Vec<i64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<i64> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 载入内置补丁数据（启动时）
pub fn load_bundled_bag() -> Bag {
    Bag::parse(BUNDLED_PATCH_JSON).expect("内置 patch.json 必须可解析")
}