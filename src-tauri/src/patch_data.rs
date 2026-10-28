//! 补丁数据模型（patch.json，PascalCase 字段）
//! serde rename_all = "PascalCase"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 内置补丁数据基线（PatchVersion=20261028）
pub const BUNDLED_PATCH_JSON: &str = include_str!("../patch-data/patch.json");

/// 远程补丁数据源镜像链，按序尝试（{ver} 替换为数据的 major.minor，如 "2.1"）
/// TODO: 部署自有数据源后替换为自有镜像地址
pub const REMOTE_URL_TEMPLATES: &[&str] = &[
    "https://hui-config.oss-cn-hangzhou.aliyuncs.com/{ver}/patch.json",
    "https://mirror.ghproxy.com/https://raw.githubusercontent.com/huiyadanli/RevokeMsgPatcher/master/RevokeMsgPatcher.Assistant/Data/{ver}/patch.json",
    "https://raw.gitmirror.com/huiyadanli/RevokeMsgPatcher/master/RevokeMsgPatcher.Assistant/Data/{ver}/patch.json",
    "https://cdn.jsdelivr.net/gh/huiyadanli/RevokeMsgPatcher@master/RevokeMsgPatcher.Assistant/Data/{ver}/patch.json",
    "https://raw.githubusercontent.com/huiyadanli/RevokeMsgPatcher/master/RevokeMsgPatcher.Assistant/Data/{ver}/patch.json",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Bag {
    #[serde(default)]
    pub apps: HashMap<String, App>,
    #[serde(default)]
    pub latest_version: String,
    #[serde(default)]
    pub notice: String,
    #[serde(default)]
    pub notice_url: String,
    #[serde(default)]
    pub patch_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct App {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file_target_infos: HashMap<String, TargetInfo>,
    /// JSON 中可能为 null（Weixin/QQNT 无精确补丁）
    #[serde(default, deserialize_with = "null_to_default")]
    pub file_modify_infos: HashMap<String, Vec<ModifyInfo>>,
    /// JSON 中可能为 null（QQLite 无特征补丁）
    #[serde(default, deserialize_with = "null_to_default")]
    pub file_common_modify_infos: HashMap<String, Vec<CommonModifyInfo>>,
}

/// JSON null → Default（C# 序列化 null 字段的兼容）
fn null_to_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let opt: Option<T> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TargetInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub relative_path: String,
    #[serde(default)]
    pub memo: Option<String>,
    /// 目标文件仅在该文件版本区间内生效（如 WeChat.exe 仅 3.7.0.x）
    #[serde(default)]
    pub start_version: Option<String>,
    #[serde(default)]
    pub end_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ModifyInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sha1_before: String,
    #[serde(default)]
    pub sha1_after: String,
    #[serde(default)]
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Change {
    /// 绝对文件偏移
    pub position: i64,
    /// 写入字节；0x3F 为通配符，表示保留原字节
    #[serde(default)]
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CommonModifyInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub start_version: String,
    #[serde(default)]
    pub end_version: Option<String>,
    #[serde(default)]
    pub replace_patterns: Vec<ReplacePattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ReplacePattern {
    /// 搜索特征码；0x3F 为通配符，且不得以通配符开头
    #[serde(default)]
    pub search: Vec<u8>,
    /// 替换内容；0x3F 表示保留原字节
    #[serde(default)]
    pub replace: Vec<u8>,
    #[serde(default)]
    pub category: String,
}

impl Bag {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl CommonModifyInfo {
    pub fn categories(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .replace_patterns
            .iter()
            .map(|p| p.category.clone())
            .filter(|c| !c.is_empty())
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_patch_json_parses() {
        let bag = Bag::parse(BUNDLED_PATCH_JSON).expect("内置 patch.json 解析失败");
        assert_eq!(bag.patch_version, 20261028);
        assert_eq!(bag.latest_version, "2.1");
        assert!(bag.apps.contains_key("Wechat"));
        assert!(bag.apps.contains_key("Weixin"));
        assert!(bag.apps.contains_key("QQ"));
        assert!(bag.apps.contains_key("TIM"));
        assert!(bag.apps.contains_key("QQLite"));
        assert!(bag.apps.contains_key("QQNT"));

        let wechat = &bag.apps["Wechat"];
        let targets: Vec<&String> = wechat.file_target_infos.keys().collect();
        assert!(targets.contains(&&"WeChatWin.dll".to_string()));
        assert!(targets.contains(&&"WeChat.exe".to_string()));

        // QQNT 的 wrapper.node 相对路径带 {version}
        let qqnt = &bag.apps["QQNT"];
        assert!(qqnt
            .file_target_infos
            .values()
            .any(|t| t.relative_path.contains("{version}")));

        // 精确补丁：WeChatWin.dll 有 SHA1 钉定的版本
        assert!(!wechat.file_modify_infos["WeChatWin.dll"].is_empty());

        // 模糊补丁：QQ 有特征码
        assert!(!bag.apps["QQ"].file_common_modify_infos.is_empty());
    }
}
