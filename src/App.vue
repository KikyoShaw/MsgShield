<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { usePatcher } from "./composables/usePatcher";
import AppSelector from "./components/AppSelector.vue";
import PathBar from "./components/PathBar.vue";
import VersionLabel from "./components/VersionLabel.vue";
import FeaturePanel from "./components/FeaturePanel.vue";
import ActionBar from "./components/ActionBar.vue";
import NoticeBanner from "./components/NoticeBanner.vue";

const {
  appList,
  detectedPaths,
  currentApp,
  appLabel,
  path,
  status,
  busy,
  checking,
  toast,
  dialog,
  init,
  switchApp,
  setPath,
  patch,
  restore,
  closeDialog,
  confirmDialog,
} = usePatcher();

const featurePanel = ref<InstanceType<typeof FeaturePanel> | null>(null);
const noticeBanner = ref<InstanceType<typeof NoticeBanner> | null>(null);

const canPatch = computed(() => !!status.value?.allFilesExist && status.value.state !== "unsupported");
const canRestore = computed(() => !!status.value?.backupExists);

async function onPatch() {
  await patch(featurePanel.value?.selected ?? []);
}

async function onRestore() {
  await restore();
}

async function openRepo() {
  await openUrl("https://github.com/KikyoShaw/MsgShield");
}

onMounted(() => {
  init().finally(() => noticeBanner.value?.refreshInfo());
});
</script>

<template>
  <main class="container">
    <NoticeBanner ref="noticeBanner" />

    <AppSelector
      :app-list="appList"
      :current="currentApp"
      :detected="detectedPaths"
      @select="switchApp"
    />

    <section class="group">
      <h2>{{ appLabel }} 安装路径</h2>
      <PathBar v-model="path" @update:model-value="setPath" />
      <VersionLabel :status="status" :checking="checking" />
    </section>

    <section class="group">
      <h2>功能选择</h2>
      <FeaturePanel ref="featurePanel" :status="status" />
    </section>

    <ActionBar
      :busy="busy"
      :can-patch="canPatch"
      :can-restore="canRestore"
      @patch="onPatch"
      @restore="onRestore"
    />

    <footer class="footnote">
      <p>本程序仅供学习交流，严禁用于商业用途。十六进制编辑器使用的修改数据集收集自网络。</p>
      <a class="repo-link" href="#" @click.prevent="openRepo">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path
            fill="currentColor"
            d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z"
          />
        </svg>
        <span>GitHub 仓库</span>
      </a>
    </footer>

    <!-- Toast -->
    <transition name="fade">
      <div v-if="toast" class="toast" :class="toast.ok ? 'ok' : 'err'">
        {{ toast.text }}
      </div>
    </transition>

    <!-- 对话框（alert / confirm） -->
    <div v-if="dialog.kind" class="modal-mask" @click.self="closeDialog">
      <div class="modal">
        <h3>{{ dialog.title }}</h3>
        <p class="dialog-message">{{ dialog.message }}</p>
        <div class="modal-actions">
          <template v-if="dialog.kind === 'confirm'">
            <button class="ghost" @click="closeDialog">取消</button>
            <button class="primary" @click="confirmDialog">确定</button>
          </template>
          <button v-else class="primary" @click="closeDialog">确定</button>
        </div>
      </div>
    </div>
  </main>
</template>

<style src="./styles.css"></style>
