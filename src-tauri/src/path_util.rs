//! 路径发现：注册表 + 磁盘扫描 + 版本比较 + {version} 目录解析
//! 移植 C# PathUtil.cs / TargetInfo.GetAbsolutePath / VersionUtil.cs

use crate::apps::{AppDef, PathStrategy};
use crate::error::PatchResult;
use crate::patch_data::{App, TargetInfo};
use std::path::{Path, PathBuf};

/// HKLM 卸载信息注册表根
const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

/// 读 HKLM 卸载键的 InstallLocation
fn install_location(key_name: &str) -> Option<String> {
    let root = winreg::enums::HKEY_LOCAL_MACHINE;
    let key = winreg::RegKey::predef(root)
        .open_subkey(format!(r"{UNINSTALL}\{key_name}"))
        .ok()?;
    let value: String = key.get_value("InstallLocation").ok()?;
    let trimmed = value.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 读 HKLM WOW6432Node 卸载键的 UninstallString（去引号，返回其父目录）
fn uninstall_string_parent(key_name: &str) -> Option<String> {
    let root = winreg::enums::HKEY_LOCAL_MACHINE;
    let key = winreg::RegKey::predef(root)
        .open_subkey(format!(r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{key_name}"))
        .ok()?;
    let value: String = key.get_value("UninstallString").ok()?;
    let trimmed = value.replace('"', "");
    let parent = Path::new(&trimmed).parent()?;
    let s = parent.to_str()?;
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// 扫描所有逻辑盘的默认安装路径（Program Files (x86) 优先，其次 Program Files）
fn default_install_paths(relative_path: &str) -> Vec<PathBuf> {
    let mut list = Vec::new();
    for drive in logical_drives() {
        let p86 = drive.join("Program Files (x86)").join(relative_path);
        if p86.is_dir() {
            list.push(p86);
        } else {
            let p = drive.join("Program Files").join(relative_path);
            if p.is_dir() {
                list.push(p);
            }
        }
    }
    list
}

/// 所有逻辑盘盘符（A:\ ~ Z:\ 中存在的）
fn logical_drives() -> Vec<PathBuf> {
    let mut drives = Vec::new();
    for c in b'A'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", c as char));
        if root.is_dir() {
            drives.push(root);
        }
    }
    drives
}

/// 子目录按修改时间倒序
fn subdirs_by_mtime_desc(base: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    dirs.push((mtime, entry.path()));
                }
            }
        }
    }
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    dirs.into_iter().map(|(_, p)| p).collect()
}

/// 版本比较（VersionUtil.Compare）：逐段数字比较；空 = 最大
/// 返回 1 / 0 / -1
pub fn version_compare(v1: &str, v2: &str) -> i32 {
    if v1.is_empty() {
        return 1;
    }
    if v2.is_empty() {
        return -1;
    }
    let s1: Vec<&str> = v1.split('.').collect();
    let s2: Vec<&str> = v2.split('.').collect();
    let len = s1.len().max(s2.len());
    for i in 0..len {
        let n1 = s1.get(i).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let n2 = s2.get(i).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        // 解析失败按 0 处理（C# Convert.ToInt32 会抛异常被上层吞掉，这里更宽松但语义等价于放行）
        match n1.cmp(&n2) {
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Less => return -1,
        }
    }
    0
}

/// 版本区间判定：start < version <= end（end 空为不限制）
pub fn is_in_version_range(version: &str, start: &str, end: Option<&str>) -> bool {
    let end = end.unwrap_or("");
    version_compare(version, start) == 1 && version_compare(version, end) <= 0
}

/// TargetInfo.GetAbsolutePath：解析 {version} 为最大版本号子目录
pub fn target_absolute_path(install_path: &str, target: &TargetInfo) -> PatchResult<PathBuf> {
    if !target.relative_path.contains("{version}") {
        return Ok(Path::new(install_path).join(&target.relative_path));
    }

    let rel = &target.relative_path;
    let version_index = rel.find("{version}").unwrap();
    let prefix = &rel[..version_index];
    let suffix = &rel[version_index + "{version}".len()..];

    let search_dir = Path::new(install_path).join(prefix);
    if !search_dir.is_dir() {
        // C# 返回 ""（后续按文件不存在处理）
        return Ok(PathBuf::from(""));
    }

    let mut valid_versions: Vec<(Vec<i64>, String)> = Vec::new();
    for entry in std::fs::read_dir(&search_dir).map_err(|e| crate::error::PatchError::io("读取版本目录", &e))? {
        let entry = entry.map_err(|e| crate::error::PatchError::io("读取版本目录", &e))?;
        let dir_name = entry.file_name().to_string_lossy().to_string();
        // 至少两个点且可解析为版本（'-' → '.'）
        if dir_name.matches('.').count() >= 2 {
            let normalized = dir_name.replace('-', ".");
            if let Some(segments) = parse_version_segments(&normalized) {
                valid_versions.push((segments, dir_name));
            }
        }
    }

    let Some(max_dir) = valid_versions
        .into_iter()
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, name)| name)
    else {
        return Err(crate::error::PatchError::business(
            "no_version_dir",
            format!("在 {} 下未找到有效的版本目录", search_dir.display()),
        ));
    };

    let full_rel = rel.replace("{version}", &max_dir);
    Ok(Path::new(install_path).join(full_rel).join(suffix.trim_start_matches('\\').trim_start_matches('/')))
}

/// "9.9.9.12345" → [9,9,9,12345]（全段数字才有效）
fn parse_version_segments(s: &str) -> Option<Vec<i64>> {
    let mut parts = Vec::new();
    for seg in s.split('.') {
        parts.push(seg.parse::<i64>().ok()?);
    }
    Some(parts)
}

/// IsAllFilesExist：所有目标文件存在（排除 WeChat.exe 次要目标）
pub fn is_all_files_exist(app: &App, install_path: &str) -> bool {
    if install_path.is_empty() {
        return false;
    }
    let mut count = 0usize;
    let mut success = 0usize;
    for target in app.file_target_infos.values() {
        if target.name == "WeChat.exe" {
            continue;
        }
        count += 1;
        if let Ok(path) = target_absolute_path(install_path, target) {
            if path.is_file() {
                success += 1;
            }
        }
    }
    success == count && success >= 1
}

/// 单应用安装路径自动发现（对齐各 Modifier.FindInstallPath）
pub fn find_install_path(def: &AppDef, app: &App) -> Option<String> {
    let check = |p: &str| -> bool { is_all_files_exist(app, p) };

    match def.path_strategy {
        PathStrategy::Direct => {
            // QQNT：先 WOW6432Node UninstallString 父目录，再普通键 InstallLocation
            if def.registry_wow6432_parent {
                if let Some(parent) = uninstall_string_parent(def.registry_key) {
                    if check(&parent) {
                        return Some(parent);
                    }
                }
            }
            let secondary = def.registry_key_secondary;
            let mut candidate = if def.registry_wow6432_parent {
                None
            } else {
                install_location(def.registry_key)
            };
            if candidate.as_deref().map(check).unwrap_or(false) {
                return candidate;
            }
            if let Some(sec) = secondary {
                candidate = install_location(sec);
                if candidate.as_deref().map(check).unwrap_or(false) {
                    return candidate;
                }
            }
            // 磁盘扫描回退
            for dir in default_install_paths(def.default_relative_path) {
                if let Some(s) = dir.to_str() {
                    if check(s) {
                        return Some(s.to_string());
                    }
                }
            }
            None
        }
        PathStrategy::NewestSubfolder => {
            // Wechat/Weixin：注册表位置 → 本身可用？否则子目录按修改时间倒序探测
            let get_real = |base: &str| -> Option<String> {
                if base.is_empty() {
                    return None;
                }
                if check(base) {
                    return Some(base.to_string());
                }
                for sub in subdirs_by_mtime_desc(Path::new(base)) {
                    if let Some(s) = sub.to_str() {
                        if check(s) {
                            return Some(s.to_string());
                        }
                    }
                }
                None
            };

            let mut base = if def.registry_wow6432_parent {
                uninstall_string_parent(def.registry_key)
            } else {
                install_location(def.registry_key)
            };
            if let Some(b) = &base {
                if let Some(real) = get_real(b) {
                    return Some(real);
                }
            }
            // 磁盘扫描回退：注意 C# 此时返回 defaultPath 本身（非 realPath）
            for dir in default_install_paths(def.default_relative_path) {
                if let Some(s) = dir.to_str() {
                    base = get_real(s);
                    if base.is_some() {
                        return Some(s.to_string());
                    }
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_empty_is_max() {
        assert_eq!(version_compare("", "1.0.0"), 1);
        assert_eq!(version_compare("1.0.0", ""), -1);
        assert_eq!(version_compare("1.0.0", "1.0.0"), 0);
        assert_eq!(version_compare("3.9.11.0", "3.9.2.0"), 1);
        assert_eq!(version_compare("3.9.11.0", "3.9.11"), 0); // 缺段补 0
        assert_eq!(version_compare("4.0.0.0", "3.9.99.99"), 1);
    }

    #[test]
    fn version_range_semantics() {
        // (start, end]
        assert!(is_in_version_range("3.8.0.0", "3.7.0.0", Some("4.0.3.0")));
        assert!(!is_in_version_range("3.7.0.0", "3.7.0.0", Some("4.0.3.0"))); // start 不含
        assert!(is_in_version_range("4.0.3.0", "3.7.0.0", Some("4.0.3.0"))); // end 含
        assert!(is_in_version_range("4.1.99.0", "4.1.12.0", None)); // end 空 = ∞
        assert!(!is_in_version_range("4.0.0.0", "4.1.12.0", None));
    }

    #[test]
    fn version_dir_name_parsing() {
        assert_eq!(parse_version_segments("9.9.9.12345"), Some(vec![9, 9, 9, 12345]));
        assert_eq!(parse_version_segments("9.9.12-23456"), None); // 必须先归一化 '-'
        assert_eq!(parse_version_segments("9.9.12.23456x"), None);
    }
}