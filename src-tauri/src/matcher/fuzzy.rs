//! 通配符模糊匹配，移植 C# FuzzyMatcher.cs + ModifyFinder.cs
//! 0x3F ('?') 为通配符；模式不得以通配符开头（head 必须非空）

use crate::error::PatchError;
use crate::matcher::boyer_moore;
use crate::patch_data::{Change, ReplacePattern};

pub const WILDCARD: u8 = 0x3F;

/// 通配符匹配所有符合查找串的位置
pub fn match_all(content: &[u8], pattern: &[u8]) -> Result<Vec<usize>, PatchError> {
    let head = get_head(pattern)?;
    let indexes = boyer_moore::match_all(content, head);
    if head.len() == pattern.len() {
        Ok(indexes)
    } else {
        Ok(indexes
            .into_iter()
            .filter(|&i| is_equal(content, i, pattern))
            .collect())
    }
}

/// 确认从 start 开始的整个串是否匹配（通配符位任意匹配）
pub fn is_equal(content: &[u8], start: usize, whole: &[u8]) -> bool {
    if start + whole.len() > content.len() {
        return false;
    }
    for (i, &b) in whole.iter().enumerate() {
        if b == WILDCARD {
            continue;
        }
        if content[start + i] != b {
            return false;
        }
    }
    true
}

/// 头串：首个通配符之前的前缀（不可为空）
fn get_head(whole: &[u8]) -> Result<&[u8], PatchError> {
    let len = whole
        .iter()
        .position(|&b| b == WILDCARD)
        .unwrap_or(whole.len());
    if len == 0 {
        return Err(PatchError::business(
            "wildcard_position",
            "不正确的通配符位置!",
        ));
    }
    Ok(&whole[..len])
}

/// IsAllReplaced：返回 (是否全部已替换, 已安装功能集合)
/// C# 版第一个返回值恒为 false（matchNum 从未自增），保留该语义
fn is_all_replaced(content: &[u8], patterns: &[ReplacePattern]) -> (bool, Vec<String>) {
    let mut already_replaced: Vec<String> = Vec::new();
    for pattern in patterns {
        let search_matches = match_all(content, &pattern.search).unwrap_or_default();
        let replace_matches = match_all(content, &pattern.replace).unwrap_or_default();
        // 查找串没有但替换串存在 = 该功能已完成替换
        if search_matches.is_empty() && !replace_matches.is_empty() {
            if !already_replaced.contains(&pattern.category) {
                already_replaced.push(pattern.category.clone());
            }
        }
    }
    already_replaced.sort();
    (false, already_replaced)
}

/// 返回文件中已安装的功能集合（UI 灰显"（已安装）"）
pub fn find_replaced_function(content: &[u8], patterns: &[ReplacePattern]) -> Vec<String> {
    is_all_replaced(content, patterns).1
}

/// 特征码扫描生成修改点（ModifyFinder.FindChanges）
pub fn find_changes(content: &[u8], replace_patterns: &[ReplacePattern]) -> Result<Vec<Change>, PatchError> {
    let mut changes: Vec<Change> = Vec::new();
    let mut match_num = 0usize;

    for pattern in replace_patterns {
        let match_indexes = match_all(content, &pattern.search)?;
        for &index in &match_indexes {
            match_num += 1;
            // 与要替换的串不一样才需要替换
            if !is_equal(content, index, &pattern.replace) {
                changes.push(Change {
                    position: index as i64,
                    content: pattern.replace.clone(),
                });
            }
        }
    }

    if match_num < replace_patterns.len() {
        let (_, already_replaced) = is_all_replaced(content, replace_patterns);
        if !already_replaced.is_empty() {
            let joined = already_replaced.join("、");
            Err(PatchError::business(
                "match_inconformity",
                format!("特征比对：以下功能补丁已经安装，请取消勾选！\n已安装功能：【{joined}】"),
            ))
        } else {
            Err(PatchError::business(
                "match_inconformity",
                format!(
                    "特征比对：当前特征码匹配数[{match_num}]和期望的匹配数[{}]不一致。\n出现此种情况的一般有如下可能：\n1. 你可能已经安装了某个功能的补丁，请选择未安装功能进行安装。\n2. 如果当前版本为最新版本，特征码可能出现变化（可能性比较低），请联系作者处理。",
                    replace_patterns.len()
                ),
            ))
        }
    } else if match_num != changes.len() {
        if changes.is_empty() {
            Err(PatchError::business(
                "match_already_replace",
                "特征比对：当前应用已经安装了所选功能补丁！",
            ))
        } else {
            Err(PatchError::business(
                "match_part_replace",
                "特征比对：部分特征已经被替换，请确认是否有使用过其他防撤回/多开补丁！",
            ))
        }
    } else {
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(search: Vec<u8>, replace: Vec<u8>, category: &str) -> ReplacePattern {
        ReplacePattern {
            search,
            replace,
            category: category.to_string(),
        }
    }

    #[test]
    fn wildcard_matches_any_byte() {
        // 0x3F 匹配任意字节
        let content = [0x01, 0x02, 0xAA, 0x03, 0x02, 0xBB, 0x03];
        let pattern = [0x02, WILDCARD, 0x03];
        assert_eq!(match_all(&content, &pattern).unwrap(), vec![1, 4]);
    }

    #[test]
    fn leading_wildcard_rejected() {
        let content = [1, 2, 3];
        let pattern = [WILDCARD, 2, 3];
        let err = match_all(&content, &pattern).unwrap_err();
        assert_eq!(err.code, "wildcard_position");
    }

    #[test]
    fn no_wildcard_is_exact() {
        let content = b"abcabc";
        assert_eq!(match_all(content, b"abc").unwrap(), vec![0, 3]);
    }

    #[test]
    fn find_changes_basic() {
        let content = vec![0x10, 0x20, 0x30, 0x40, 0x10, 0x20, 0x30, 0x40];
        let p = pattern(
            vec![0x10, 0x20, 0x30, 0x40],
            vec![0x10, 0x20, 0x31, 0x40],
            "防撤回",
        );
        let changes = find_changes(&content, &[p.clone()]).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].position, 0);
        assert_eq!(changes[1].position, 4);
        assert_eq!(changes[0].content, vec![0x10, 0x20, 0x31, 0x40]);
    }

    #[test]
    fn find_changes_detects_installed() {
        // 已替换：Search 0 匹配、Replace 2 匹配
        let content = vec![0x10, 0x20, 0x31, 0x40, 0x10, 0x20, 0x31, 0x40];
        let p = pattern(
            vec![0x10, 0x20, 0x30, 0x40],
            vec![0x10, 0x20, 0x31, 0x40],
            "防撤回",
        );
        let err = find_changes(&content, &[p]).unwrap_err();
        assert_eq!(err.code, "match_inconformity");
        assert!(err.message.contains("【防撤回】"));
    }

    #[test]
    fn find_replaced_function_reports_category() {
        let content = vec![0x10, 0x20, 0x31, 0x40];
        let installed = pattern(
            vec![0x10, 0x20, 0x30, 0x40],
            vec![0x10, 0x20, 0x31, 0x40],
            "防撤回",
        );
        let not_installed = pattern(
            vec![0x50, 0x60],
            vec![0x50, 0x61],
            "多开",
        );
        assert_eq!(
            find_replaced_function(&content, &[installed, not_installed]),
            vec!["防撤回".to_string()]
        );
    }
}