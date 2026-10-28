# MsgShield 消息防撤回

聊天客户端消息增强工具。基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 的 Windows 桌面应用。

## 功能

- 支持 6 个应用：微信 3.x（Wechat）、微信 4.x（Weixin）、QQ、TIM、QQ轻聊版（QQLite）、QQNT
- 消息防撤回 / 多开等补丁（按功能勾选，已安装功能自动灰显）
- 两种补丁策略：
  - **精确补丁**：SHA1 钉定版本 + 绝对偏移写入
  - **特征码补丁**：文件版本区间 + Boyer-Moore 通配符（`0x3F`）特征搜索
- 备份/恢复：`.h.bak` 侧车备份，同版本备份不覆盖，恢复前版本一致性确认
- 安装失败自动回滚（先备份全部 → 逐文件打补丁 → 失败恢复已打文件）
- 补丁数据：内置基线（`src-tauri/patch-data/patch.json`），启动时远程刷新，`PatchVersion` 单调热替换

## 运行

```bash
npm install
npm run tauri dev    # 开发（会弹 UAC 提权：补丁需写 Program Files 下的 DLL）
npm run tauri build  # 发布构建
```

> 注意：应用清单要求管理员权限（`src-tauri/app.manifest` 中 `requireAdministrator`）。

## 架构

```
src/                        # Vue 3 前端
├── api/patcher.ts          # invoke 封装 + 与 Rust 对齐的 TS 类型
├── composables/usePatcher  # 核心状态：应用切换/路径/三态/补丁操作
└── components/             # AppSelector / PathBar / VersionLabel /
                            # FeaturePanel / ActionBar / NoticeBanner

src-tauri/src/
├── patch_data.rs           # patch.json serde 模型（PascalCase）+ 内置数据
├── apps.rs                 # 6 应用静态定义（注册表键/回退路径/主 DLL）
├── path_util.rs            # 注册表 + 磁盘扫描 + {version} 目录解析 + 版本比较
├── matcher/
│   ├── boyer_moore.rs      # Boyer-Moore（坏字符 + 好后缀）
│   └── fuzzy.rs            # 0x3F 通配匹配 + FindChanges + 已装功能检测
├── editor.rs               # FileHexEditor：SHA1/版本/备份/就地写入（0x3F 跳写）
├── modifier.rs             # AppModifier：三态/验证/打补丁/恢复编排
├── commands.rs             # Tauri commands + 全局 Bag 状态 + 远程刷新
└── error.rs                # PatchError { code, message }
```

### 关键语义

| 语义 | 实现 |
|---|---|
| `0x3F` 通配符 | Search 中匹配任意字节；Replace/Changes 中跳过写入保留原字节 |
| 特征模式约束 | 模式不得以通配符开头（head 为 Boyer-Moore 锚点） |
| 版本区间 | `(StartVersion, EndVersion]`，空 EndVersion = ∞，逐段数字比较 |
| QQNT `{version}` | 取最大版本号子目录（≥2 个点、`-`→`.` 可解析） |
| 错误码 | `installed` / `not_support` / `maybe_modified` / `match_already_replace` / `match_inconformity` / `match_part_replace` |
| 版本标签三态 | 绿（已支持，精确）/ 橙绿（支持特征防撤回）/ 红（不支持） |

## 测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # 18 个单测：Boyer-Moore 对拍、通配语义、0x3F 跳写、备份规则、patch.json 解析
npm run build                                      # vue-tsc 类型检查 + vite 构建
```

## 免责声明

本程序仅供学习交流，严禁用于商业用途。
