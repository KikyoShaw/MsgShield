import { computed, reactive, readonly, ref, type Ref } from "vue";
import {
  applyPatch,
  detectApps,
  getAppStatus,
  getPatchDataInfo,
  refreshPatchData,
  restoreBackup,
  type AppStatus,
  type PatchError,
  type RestoreOutcome,
} from "../api/patcher";

/** 应用 key → 显示名（与 Rust apps.rs 对齐） */
const APP_LIST = [
  { key: "Wechat", label: "微信 3.x" },
  { key: "Weixin", label: "微信 4.x" },
  { key: "QQ", label: "QQ" },
  { key: "TIM", label: "TIM" },
  { key: "QQLite", label: "QQ轻聊版" },
  { key: "QQNT", label: "QQNT" },
] as const;

export type DialogKind = "alert" | "confirm" | null;

export interface DialogState {
  kind: DialogKind;
  title: string;
  message: string;
  /** confirm 确定回调 */
  onConfirm?: () => void | Promise<void>;
}

export function usePatcher() {
  const detectedPaths = reactive<Record<string, string | null>>({});
  const currentApp: Ref<string> = ref("Wechat");
  const path: Ref<string> = ref("");
  const status: Ref<AppStatus | null> = ref(null);
  const busy = ref(false);
  const toast = ref<{ ok: boolean; text: string } | null>(null);
  const dialog = reactive<DialogState>({ kind: null, title: "", message: "" });

  const appLabel = computed(
    () => APP_LIST.find((a) => a.key === currentApp.value)?.label ?? currentApp.value,
  );

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(ok: boolean, text: string) {
    toast.value = { ok, text };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast.value = null), 5000);
  }

  function showAlert(title: string, message: string) {
    dialog.kind = "alert";
    dialog.title = title;
    dialog.message = message;
  }

  function showConfirm(title: string, message: string, onConfirm: () => void | Promise<void>) {
    dialog.kind = "confirm";
    dialog.title = title;
    dialog.message = message;
    dialog.onConfirm = onConfirm;
  }

  function closeDialog() {
    dialog.kind = null;
    dialog.onConfirm = undefined;
  }

  async function confirmDialog() {
    const cb = dialog.onConfirm;
    closeDialog();
    await cb?.();
  }

  // ===== 状态查询 =====

  const checking = ref(false);

  async function refreshStatus() {
    if (!path.value) {
      status.value = null;
      return;
    }
    checking.value = true;
    try {
      status.value = await getAppStatus(currentApp.value, path.value);
    } catch (e) {
      status.value = null;
      const err = e as PatchError;
      showToast(false, err?.message ?? String(e));
    } finally {
      checking.value = false;
    }
  }

  async function init() {
    // 6 应用自动探测
    const detected = await detectApps();
    for (const d of detected) {
      detectedPaths[d.app_key] = d.installPath;
    }
    // 默认选微信，未装则选第一个探测到的
    if (!detectedPaths["Wechat"]) {
      const first = detected.find((d) => d.installPath);
      if (first) currentApp.value = first.app_key;
    }
    path.value = detectedPaths[currentApp.value] ?? "";
    await refreshStatus();
    // 后台刷新远程补丁数据（不阻塞 UI）
    refreshPatchData().catch(() => {
      /* refresh 内部已把状态记为 failed */
    });
  }

  async function switchApp(key: string) {
    currentApp.value = key;
    path.value = detectedPaths[key] ?? "";
    await refreshStatus();
  }

  async function setPath(p: string) {
    path.value = p;
    detectedPaths[currentApp.value] = p;
    await refreshStatus();
    if (status.value && !status.value.allFilesExist) {
      showAlert("提示", "无法找到此应用的关键文件，请选择正确的安装路径!");
    }
  }

  // ===== 补丁操作 =====

  async function doPatch(categories: string[] | null) {
    busy.value = true;
    try {
      await applyPatch(currentApp.value, path.value, categories);
      showToast(true, "补丁安装成功！");
    } catch (e) {
      const err = e as PatchError;
      // IO 类失败文案已由 Rust 侧追加管理员/关闭应用提示
      showAlert("错误", err?.message ?? String(e));
    } finally {
      busy.value = false;
      await refreshStatus();
    }
  }

  async function patch(selectedCategories: string[]) {
    if (!path.value) {
      showAlert("提示", "请选择正确的安装路径!");
      return;
    }
    if (status.value?.state === "fuzzy") {
      if (selectedCategories.length === 0) {
        showAlert("提示", "请至少选择一项功能");
        return;
      }
      // 防撤回(老) 与 防撤回带提示(新) 二选一
      if (
        selectedCategories.includes("防撤回(老)") &&
        selectedCategories.includes("防撤回带提示(新)")
      ) {
        showAlert(
          "提示",
          "防撤回(老) 和 防撤回带提示(新) 两个功能二选一即可！\r\n如果不确定怎么选，请取消本窗口后只勾选其一。",
        );
        return;
      }
      await doPatch(selectedCategories);
    } else {
      await doPatch(null);
    }
  }

  async function doRestore(force: boolean) {
    busy.value = true;
    try {
      const outcome: RestoreOutcome = await restoreBackup(
        currentApp.value,
        path.value,
        force,
      );
      if (outcome.kind === "need_confirm") {
        showConfirm(
          "版本不一致",
          `当前文件：${outcome.file}，版本：${outcome.currentVersion}；\r\n备份文件版本：${outcome.backupVersion}；\r\n两者版本不一致，是否继续还原？`,
          () => doRestore(true),
        );
      } else {
        showToast(true, "还原成功！");
      }
    } catch (e) {
      const err = e as PatchError;
      showAlert("错误", err?.message ?? String(e));
    } finally {
      busy.value = false;
      await refreshStatus();
    }
  }

  async function restore() {
    if (!path.value) {
      showAlert("提示", "请选择正确的安装路径!");
      return;
    }
    await doRestore(false);
  }

  return {
    appList: APP_LIST,
    detectedPaths: readonly(detectedPaths),
    currentApp,
    appLabel,
    path,
    status,
    busy,
    checking,
    toast,
    dialog: readonly(dialog),
    init,
    switchApp,
    setPath,
    patch,
    restore,
    closeDialog,
    confirmDialog,
    getPatchDataInfo,
    refreshPatchData,
  };
}
