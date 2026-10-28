//! AppModifier：单应用的补丁编排（初始化 editors、三态状态、验证、打补丁、恢复）
//! 移植 C# AppModifier.cs

use crate::apps::AppDef;
use crate::editor::FileHexEditor;
use crate::error::{PatchError, PatchResult};
use crate::matcher::fuzzy;
use crate::patch_data::{App, CommonModifyInfo};
use crate::path_util::is_in_version_range;
use std::io::Read;

/// 应用当前状态（对应 C# 版本标签三态）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportState {
    /// 精确补丁（绿：已支持）
    Exact,
    /// 特征码补丁（橙绿：支持特征防撤回）
    Fuzzy,
    /// 不支持（红）
    Unsupported,
}

/// UI 展示的功能项
#[derive(Debug, Clone)]
pub struct CategoryItem {
    pub name: String,
    pub installed: bool,
}

pub struct AppModifier<'a> {
    pub def: &'a AppDef,
    pub config: &'a App,
    pub editors: Vec<FileHexEditor>,
}

impl<'a> AppModifier<'a> {
    /// 初始化文件修改器：按 TargetInfo 版本区间过滤（InitEditors）
    pub fn new(def: &'a AppDef, config: &'a App, install_path: &str) -> PatchResult<Self> {
        let mut editors = Vec::new();
        // 按 key 排序保证确定性（C# HashMap 顺序不定，这里更稳）
        let mut targets: Vec<_> = config.file_target_infos.values().cloned().collect();
        targets.sort_by(|a, b| a.name.cmp(&b.name));
        for target in targets {
            let editor = FileHexEditor::new(install_path, target)?;
            let start = editor.target_info.start_version.clone().unwrap_or_default();
            let end = editor.target_info.end_version.clone().unwrap_or_default();
            let version = editor.file_version();
            let in_range = (start.is_empty() && end.is_empty())
                || is_in_version_range(&version, &start, if end.is_empty() { None } else { Some(&end) });
            if in_range {
                editors.push(editor);
            }
        }
        if editors.is_empty() {
            return Err(PatchError::business(
                "no_editor",
                "当前版本没有对应的文件修改信息，请确认补丁信息是否正常！",
            ));
        }
        Ok(Self {
            def,
            config,
            editors,
        })
    }

    /// 应用当前版本：取主 DLL（def.version_file）的文件版本
    pub fn version(&self) -> String {
        for editor in &self.editors {
            if editor.file_name == self.def.version_file {
                return editor.file_version();
            }
        }
        String::new()
    }

    /// 三态状态 + 功能列表（SetVersionLabelAndCategoryCategories）
    pub fn support_state(&self) -> (SupportState, Vec<CategoryItem>) {
        // 精确版本匹配：每个 editor 的文件版本都在 FileModifyInfos 中有对应
        if !self.config.file_modify_infos.is_empty() {
            let mut exact_count = 0;
            for editor in &self.editors {
                let infos = self.config.file_modify_infos.get(&editor.file_name);
                if let Some(infos) = infos {
                    if infos.iter().any(|m| m.version == editor.file_version()) {
                        exact_count += 1;
                    }
                }
            }
            if exact_count == self.editors.len() && !self.editors.is_empty() {
                return (SupportState::Exact, Vec::new());
            }
        }

        // 特征码版本区间匹配
        let mut categories: Vec<String> = Vec::new();
        let mut installed: Vec<String> = Vec::new();
        let mut fuzzy_count = 0;
        for editor in &self.editors {
            if let Some(common) = self.find_common_modify_info(editor) {
                for c in common.categories() {
                    if !c.is_empty() && !categories.contains(&c) {
                        categories.push(c);
                    }
                }
                if let Ok(content) = read_file_bytes(&editor.file_path) {
                    for c in fuzzy::find_replaced_function(&content, &common.replace_patterns) {
                        if !installed.contains(&c) {
                            installed.push(c);
                        }
                    }
                }
                fuzzy_count += 1;
            }
        }
        categories.sort();

        if fuzzy_count == self.editors.len() && !self.editors.is_empty() {
            let items = categories
                .into_iter()
                .map(|name| CategoryItem {
                    installed: installed.contains(&name),
                    name,
                })
                .collect();
            (SupportState::Fuzzy, items)
        } else {
            (SupportState::Unsupported, Vec::new())
        }
    }

    /// 备份存在且每个 editor 版本一致（BackupExists）
    pub fn backup_exists(&self) -> bool {
        self.editors.iter().all(|e| e.backup_exists())
    }

    /// b. 验证并计算补丁修改点（ValidateAndFindModifyInfo）
    pub fn validate_and_find_modify_info(&mut self, categories: Option<&[String]>) -> PatchResult<()> {
        for editor in &mut self.editors {
            let file_version = editor.file_version();
            let file_sha1 = editor.file_sha1()?;

            let mut matching_sha1_before: Option<&crate::patch_data::ModifyInfo> = None;
            let mut matching_sha1_after: Option<&crate::patch_data::ModifyInfo> = None;
            let mut matching_version: Option<&crate::patch_data::ModifyInfo> = None;

            if let Some(infos) = self.config.file_modify_infos.get(&editor.file_name) {
                for info in infos {
                    if file_sha1 == info.sha1_after && !info.sha1_after.is_empty() {
                        matching_sha1_after = Some(info);
                    } else if file_sha1 == info.sha1_before && !info.sha1_before.is_empty() {
                        matching_sha1_before = Some(info);
                    }
                    if file_version == info.version {
                        matching_version = Some(info);
                    }
                }
            }

            // 补丁前 SHA1 匹配 → 精确补丁
            if let Some(before) = matching_sha1_before {
                editor.target_changes = before.changes.clone();
                editor.file_modify_info = Some(before.clone());
                continue;
            }

            // 补丁后 SHA1 匹配 → 已安装
            if matching_sha1_after.is_some() {
                return Err(PatchError::business("installed", "你已经安装过此补丁！"));
            }

            // SHA1 都不匹配 → 尝试特征码（clone 检索依据，避免 &mut self 与 &self 借用冲突）
            let file_version_now = editor.file_version();
            let file_name_now = editor.file_name.clone();
            let common_info = self
                .config
                .file_common_modify_infos
                .get(&file_name_now)
                .and_then(|infos| {
                    infos
                        .iter()
                        .find(|info| {
                            is_in_version_range(&file_version_now, &info.start_version, info.end_version.as_deref())
                        })
                        .cloned()
                });
            if let Some(common) = &common_info {
                if !common.replace_patterns.is_empty() {
                    let replace_patterns: Vec<_> = match categories {
                        Some(cats) if !cats.is_empty() => common
                            .replace_patterns
                            .iter()
                            .filter(|p| cats.contains(&p.category))
                            .cloned()
                            .collect(),
                        _ => common.replace_patterns.clone(),
                    };
                    let content = read_file_bytes(&editor.file_path)?;
                    editor.target_changes = fuzzy::find_changes(&content, &replace_patterns)?;
                    editor.file_common_modify_info = Some(common.clone());
                    continue;
                }
            }

            // 无特征信息：按版本匹配与否区分错误
            if matching_version.is_none() {
                return Err(PatchError::business(
                    "not_support",
                    format!("不支持的文件：名称 {} 版本 {}！", editor.file_name, file_version),
                ));
            }
            return Err(PatchError::business(
                "maybe_modified",
                format!(
                    "程序支持此文件版本： {} - {}。但是文件校验不通过，请确认是否使用过其他补丁程序！",
                    editor.file_name, file_version
                ),
            ));
        }
        Ok(())
    }

    /// c. 安装补丁：先备份全部 → 逐个打 → 失败回滚已打的（Patch）
    pub fn patch(&mut self) -> PatchResult<()> {
        if self.editors.is_empty() {
            return Err(PatchError::business("no_editor", "补丁安装失败，原因：无对应的文件修改器"));
        }

        // 先备份所有文件
        for editor in &self.editors {
            editor.backup()?;
        }

        // 打补丁
        let mut done: Vec<usize> = Vec::new();
        let result: PatchResult<()> = (|| {
            for (i, editor) in self.editors.iter().enumerate() {
                editor.patch()?;
                done.push(i);
            }
            Ok(())
        })();

        if let Err(err) = result {
            // 恢复所有已打上补丁的文件
            for &i in &done {
                let _ = self.editors[i].restore();
            }
            return Err(err);
        }
        Ok(())
    }

    /// 寻找版本区间对应的特征码信息（FindCommonModifyInfo：取第一个命中的）
    fn find_common_modify_info(&self, editor: &FileHexEditor) -> Option<&CommonModifyInfo> {
        let infos = self.config.file_common_modify_infos.get(&editor.file_name)?;
        let version = editor.file_version();
        infos
            .iter()
            .find(|info| is_in_version_range(&version, &info.start_version, info.end_version.as_deref()))
    }
}

fn read_file_bytes(path: &std::path::Path) -> PatchResult<Vec<u8>> {
    let mut file =
        std::fs::File::open(path).map_err(|e| PatchError::io(&format!("打开 {}", path.display()), &e))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| PatchError::io(&format!("读取 {}", path.display()), &e))?;
    Ok(buf)
}