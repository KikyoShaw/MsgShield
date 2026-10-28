<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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
      本程序仅供学习交流，严禁用于商业用途。十六进制编辑器使用的修改数据集收集自网络。
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
