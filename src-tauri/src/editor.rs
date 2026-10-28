//! FileHexEditor：单目标文件的版本/SHA1/备份/打补丁/恢复
//! 移植 C# FileHexEditor.cs + FileUtil.EditMultiHex

use crate::error::{PatchError, PatchResult};
use crate::patch_data::{Change, CommonModifyInfo, ModifyInfo, TargetInfo};
use sha1::{Digest, Sha1};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// 备份文件后缀
pub const BAK_SUFFIX: &str = ".h.bak";

pub struct FileHexEditor {
    pub file_name: String,
    pub file_path: PathBuf,
    pub file_bak_path: PathBuf,
    pub target_info: TargetInfo,
    /// 通过 SHA1 匹配到的精确补丁信息
    pub file_modify_info: Option<ModifyInfo>,
    /// 通过版本区间匹配到的特征码补丁信息
    pub file_common_modify_info: Option<CommonModifyInfo>,
    /// 将要执行的修改
    pub target_changes: Vec<Change>,
}

impl FileHexEditor {
    pub fn new(install_path: &str, target: TargetInfo) -> PatchResult<Self> {
        let file_path = crate::path_util::target_absolute_path(install_path, &target)?;
        let file_bak_path = {
            let mut s = file_path.clone().into_os_string();
            s.push(BAK_SUFFIX);
            PathBuf::from(s)
        };
        Ok(Self {
            file_name: target.name.clone(),
            file_path,
            file_bak_path,
            target_info: target,
            file_modify_info: None,
            file_common_modify_info: None,
            target_changes: Vec::new(),
        })
    }

    /// 文件版本（VS_FIXEDFILEINFO 的 FileVersion），不存在返回空串
    pub fn file_version(&self) -> String {
        get_file_version(&self.file_path).unwrap_or_default()
    }

    /// 备份文件版本
    pub fn backup_file_version(&self) -> String {
        get_file_version(&self.file_bak_path).unwrap_or_default()
    }

    /// 全文件 SHA1（小写 hex）；计算前先把属性归 Normal（对齐 C#）
    pub fn file_sha1(&self) -> PatchResult<String> {
        let path = &self.file_path;
        if !path.is_file() {
            return Err(PatchError::business("file_not_found", format!("文件不存在：{}", path.display())));
        }
        // 归 Normal（去掉只读等属性），失败容忍
        let _ = clear_readonly(path);
        let mut file = std::fs::File::open(path)
            .map_err(|e| PatchError::io(&format!("打开 {}", path.display()), &e))?;
        let mut hasher = Sha1::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|e| PatchError::io(&format!("读取 {}", path.display()), &e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /// 备份：不覆盖同版本的备份文件（首次备份优先）
    pub fn backup(&self) -> PatchResult<()> {
        if self.file_bak_path.exists() {
            if self.file_version() != self.backup_file_version() {
                copy_file(&self.file_path, &self.file_bak_path)?;
            }
        } else {
            copy_file(&self.file_path, &self.file_bak_path)?;
        }
        Ok(())
    }

    /// 打补丁：就地写入 target_changes（0x3F = 保留原字节）
    pub fn patch(&self) -> PatchResult<()> {
        edit_multi_hex(&self.file_path, &self.target_changes)
    }

    /// 恢复：备份文件覆盖回原文件
    pub fn restore(&self) -> PatchResult<()> {
        copy_file(&self.file_bak_path, &self.file_path)
    }

    /// 备份存在且版本一致
    pub fn backup_exists(&self) -> bool {
        self.file_bak_path.is_file() && self.file_version() == self.backup_file_version()
    }
}

/// Windows 文件版本（GetFileVersionInfoExW + VS_FIXEDFILEINFO，等价 C# FileVersionInfo.FileVersion）
#[cfg(windows)]
fn get_file_version(path: &std::path::Path) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoExW, GetFileVersionInfoSizeExW, VerQueryValueW, FILE_VER_GET_LOCALISED,
    };

    if !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeExW(FILE_VER_GET_LOCALISED, wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoExW(
            FILE_VER_GET_LOCALISED,
            wide.as_ptr(),
            handle,
            size,
            data.as_mut_ptr() as *mut core::ffi::c_void,
        ) == 0
        {
            return None;
        }
        let mut fixed_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut fixed_len = 0u32;
        // 子块 "\\" = 根路径，返回 VS_FIXEDFILEINFO（"VS_VERSION_INFO" 是资源块名，不是子块路径）
        let subblock: Vec<u16> = "\\\0".encode_utf16().collect();
        if VerQueryValueW(
            data.as_ptr() as *const core::ffi::c_void,
            subblock.as_ptr(),
            &mut fixed_ptr,
            &mut fixed_len,
        ) == 0
        {
            return None;
        }
        if fixed_len < 52 || fixed_ptr.is_null() {
            return None;
        }
        // 校验 dwSignature == 0xFEEF04BD
        let sig_check = std::slice::from_raw_parts(fixed_ptr as *const u8, 4);
        if sig_check != [0xBD, 0x04, 0xEF, 0xFE] {
            return None;
        }
        // VS_FIXEDFILEINFO: dwSignature(0) dwStrucVersion(4) dwFileVersionMS(8) dwFileVersionLS(12)
        let bytes = std::slice::from_raw_parts(fixed_ptr as *const u8, fixed_len as usize);
        let read_u32 = |off: usize| {
            u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
        };
        let ms = read_u32(8);
        let ls = read_u32(12);
        Some(format!(
            "{}.{}.{}.{}",
            ms >> 16,
            ms & 0xFFFF,
            ls >> 16,
            ls & 0xFFFF
        ))
    }
}

#[cfg(not(windows))]
fn get_file_version(_path: &std::path::Path) -> Option<String> {
    None
}

/// 清除只读属性（对齐 C# File.SetAttributes Normal）
#[cfg(windows)]
fn clear_readonly(path: &std::path::Path) -> std::io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_NORMAL,
    };
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs != u32::MAX {
            SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn clear_readonly(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn copy_file(from: &std::path::Path, to: &std::path::Path) -> PatchResult<()> {
    let _ = clear_readonly(to);
    std::fs::copy(from, to).map_err(|e| PatchError::io(&format!("复制文件 {} → {}", from.display(), to.display()), &e))?;
    Ok(())
}

/// EditMultiHex：打开 ReadWrite，逐 Change Seek(Position) 写入；0x3F 跳过保留原字节
pub fn edit_multi_hex(path: &std::path::Path, changes: &[Change]) -> PatchResult<()> {
    let _ = clear_readonly(path);
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| PatchError::io(&format!("打开 {}", path.display()), &e))?;

    for change in changes {
        file.seek(SeekFrom::Start(change.position as u64))
            .map_err(|e| PatchError::io("定位偏移", &e))?;
        for &b in &change.content {
            if b == 0x3F {
                // 通配符：读一个字节前进，不写入
                file.read(&mut [0u8; 1]).map_err(|e| PatchError::io("读取", &e))?;
            } else {
                file.write_all(&[b]).map_err(|e| PatchError::io("写入", &e))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(content: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rmp_test_{}_{:x}",
            std::process::id(),
            content.len() ^ (content.first().copied().unwrap_or(0) as usize)
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    fn cleanup(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("h.bak.x"));
    }

    #[test]
    fn edit_multi_hex_writes_and_skips_wildcard() {
        // 原始 [1,2,3,4,5]；修改：位置1 写 [0xAA, 0x3F, 0xBB] → [1,AA,3,BB,5]
        let path = temp_file(&[1, 2, 3, 4, 5]);
        let changes = vec![Change {
            position: 1,
            content: vec![0xAA, 0x3F, 0xBB],
        }];
        edit_multi_hex(&path, &changes).unwrap();
        let result = std::fs::read(&path).unwrap();
        assert_eq!(result, vec![1, 0xAA, 3, 0xBB, 5]);
        cleanup(&path);
    }

    #[test]
    fn edit_multi_hex_multiple_changes() {
        let path = temp_file(&[0; 8]);
        let changes = vec![
            Change { position: 0, content: vec![1] },
            Change { position: 7, content: vec![2] },
        ];
        edit_multi_hex(&path, &changes).unwrap();
        let result = std::fs::read(&path).unwrap();
        assert_eq!(result, vec![1, 0, 0, 0, 0, 0, 0, 2]);
        cleanup(&path);
    }

    #[test]
    fn sha1_of_known_content() {
        // "abc" 的 SHA1
        let path = temp_file(b"abc");
        let editor = FileHexEditor {
            file_name: "t".into(),
            file_path: path.clone(),
            file_bak_path: path.clone(),
            target_info: TargetInfo::default(),
            file_modify_info: None,
            file_common_modify_info: None,
            target_changes: vec![],
        };
        assert_eq!(
            editor.file_sha1().unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        cleanup(&path);
    }

    #[test]
    #[cfg(windows)]
    fn file_version_of_system_dll() {
        // 用系统自带 DLL 做真实回归：必须读出非空版本号
        let sys_dir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let candidates = [
            std::path::Path::new(&sys_dir).join("System32").join("kernel32.dll"),
            std::path::Path::new(&sys_dir).join("System32").join("user32.dll"),
        ];
        let path = candidates.iter().find(|p| p.is_file()).expect("找不到系统 DLL");
        let v = get_file_version(path).expect("读取系统 DLL 版本失败");
        assert!(
            !v.is_empty() && v.split('.').count() >= 3,
            "版本号格式异常：{v}"
        );
        println!("{:?} -> {}", path, v);
    }
}