//! 6 个应用的静态定义：注册表键名、回退磁盘路径、路径解析策略、主 DLL 文件名
//! 对齐 C# 版各 Modifier 的 FindInstallPath/GetVersion 逻辑

/// 应用在 patch.json 中的 key
pub const APP_KEYS: &[&str] = &["Wechat", "Weixin", "QQ", "TIM", "QQLite", "QQNT"];

/// 路径发现策略（决定 How 找安装目录）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStrategy {
    /// 注册表 InstallLocation → 磁盘扫描回退，直接当安装目录（QQ/TIM/QQLite/QQNT）
    Direct,
    /// Direct + 子目录按修改时间倒序探测（微信 3.5.0.4 目录结构变化：Wechat/Weixin）
    NewestSubfolder,
}

/// 单个应用的定位配置
pub struct AppDef {
    pub key: &'static str,
    /// HKLM 卸载信息键名（主）
    pub registry_key: &'static str,
    /// 是否从 WOW6432Node 读且取 UninstallString 的父目录（Weixin/QQNT 的 QQ 键）
    pub registry_wow6432_parent: bool,
    /// 次选注册表键名（QQNT：先 WOW6432Node QQ 再 QQNT）
    pub registry_key_secondary: Option<&'static str>,
    /// 磁盘扫描回退相对路径（Tencent\*）
    pub default_relative_path: &'static str,
    pub path_strategy: PathStrategy,
    /// GetVersion() 取哪个文件的版本（即主 DLL 文件名）
    pub version_file: &'static str,
}

pub const APP_DEFS: &[AppDef] = &[
    AppDef {
        key: "Wechat",
        registry_key: "Wechat",
        registry_wow6432_parent: false,
        registry_key_secondary: None,
        default_relative_path: r"Tencent\Wechat",
        path_strategy: PathStrategy::NewestSubfolder,
        version_file: "WeChatWin.dll",
    },
    AppDef {
        key: "Weixin",
        registry_key: "Weixin",
        registry_wow6432_parent: true,
        registry_key_secondary: None,
        default_relative_path: r"Tencent\Weixin",
        path_strategy: PathStrategy::NewestSubfolder,
        version_file: "Weixin.dll",
    },
    AppDef {
        key: "QQ",
        registry_key: "{052CFB79-9D62-42E3-8A15-DE66C2C97C3E}",
        registry_wow6432_parent: false,
        registry_key_secondary: None,
        default_relative_path: r"Tencent\QQ",
        path_strategy: PathStrategy::Direct,
        version_file: "IM.dll",
    },
    AppDef {
        key: "TIM",
        registry_key: "TIM",
        registry_wow6432_parent: false,
        registry_key_secondary: None,
        default_relative_path: r"Tencent\TIM",
        path_strategy: PathStrategy::Direct,
        version_file: "IM.dll",
    },
    AppDef {
        key: "QQLite",
        registry_key: "QQLite",
        registry_wow6432_parent: false,
        registry_key_secondary: None,
        default_relative_path: r"Tencent\QQLite",
        path_strategy: PathStrategy::Direct,
        version_file: "IM.dll",
    },
    AppDef {
        key: "QQNT",
        registry_key: "QQ",
        registry_wow6432_parent: true,
        // 先 WOW6432Node\Uninstall\QQ（UninstallString 父目录），再 Uninstall\QQNT
        registry_key_secondary: Some("QQNT"),
        default_relative_path: r"Tencent\QQNT",
        path_strategy: PathStrategy::Direct,
        version_file: "wrapper.node",
    },
];

pub fn app_def(key: &str) -> Option<&'static AppDef> {
    APP_DEFS.iter().find(|d| d.key == key)
}