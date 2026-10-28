import { invoke } from "@tauri-apps/api/core";

// ===== 与 Rust 侧 commands.rs 对齐的类型 =====

export interface DetectedApp {
  app_key: string;
  installPath: string | null;
}

export type AppState = "exact" | "fuzzy" | "unsupported" | "no_files";

export interface CategoryItem {
  name: string;
  installed: boolean;
}

export interface AppStatus {
  version: string;
  state: AppState;
  categories: CategoryItem[];
  backupExists: boolean;
  allFilesExist: boolean;
}

export interface SupportedVersions {
  appKey: string;
  versions: string[];
}

export type RemoteStatus =
  | { kind: "pending" }
  | { kind: "failed" }
  | { kind: "new_version_available"; data: string }
  | { kind: "updated" }
  | { kind: "up_to_date" };

export interface PatchDataInfo {
  patchVersion: number;
  latestVersion: string;
  notice: string;
  noticeUrl: string;
  remote: RemoteStatus;
}

export type RestoreOutcome =
  | { kind: "restored" }
  | {
      kind: "need_confirm";
      file: string;
      currentVersion: string;
      backupVersion: string;
    };

/** Rust PatchError 序列化形（invoke reject 时收到的对象） */
export interface PatchError {
  code: string;
  message: string;
}

// ===== invoke 封装 =====

export function detectApps(): Promise<DetectedApp[]> {
  return invoke<DetectedApp[]>("detect_apps");
}

export function getAppStatus(appKey: string, path: string): Promise<AppStatus> {
  return invoke<AppStatus>("get_app_status", { appKey, path });
}

export function applyPatch(
  appKey: string,
  path: string,
  categories: string[] | null,
): Promise<void> {
  return invoke<void>("apply_patch", { appKey, path, categories });
}

export function restoreBackup(
  appKey: string,
  path: string,
  force: boolean,
): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("restore_backup", { appKey, path, force });
}

export function getSupportedVersions(): Promise<SupportedVersions[]> {
  return invoke<SupportedVersions[]>("get_supported_versions");
}

export function getPatchDataInfo(): Promise<PatchDataInfo> {
  return invoke<PatchDataInfo>("get_patch_data_info");
}

export function refreshPatchData(): Promise<PatchDataInfo> {
  return invoke<PatchDataInfo>("refresh_patch_data");
}
