<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  getPatchDataInfo,
  getSupportedVersions,
  type PatchDataInfo,
  type SupportedVersions,
} from "../api/patcher";

const info = ref<PatchDataInfo | null>(null);
const showVersions = ref(false);
const supported = ref<SupportedVersions[]>([]);

const statusText = computed(() => {
  const r = info.value?.remote;
  if (!r || r.kind === "pending") return "正在获取补丁信息...";
  switch (r.kind) {
    case "failed":
      return "[ 获取最新补丁信息失败 ]";
    case "new_version_available":
      return `[ 存在最新版本 ${r.data} ]`;
    case "updated":
      return "[ 补丁数据已更新，点击查看更多信息 ]";
    case "up_to_date":
      return "[ 软件内置补丁信息已经是最新 ]";
  }
});

const hasNewVersion = computed(() => info.value?.remote.kind === "new_version_available");

async function refreshInfo() {
  info.value = await getPatchDataInfo();
}

async function openNoticeUrl() {
  const url = info.value?.noticeUrl;
  if (url) await openUrl(url);
}

async function openVersions() {
  supported.value = await getSupportedVersions();
  showVersions.value = true;
}

onMounted(refreshInfo);
defineExpose({ refreshInfo });
</script>

<template>
  <div class="notice-banner">
    <p
      v-if="info?.notice"
      class="notice clickable"
      title="点击查看"
      @click="openNoticeUrl"
    >
      {{ info.notice }}
    </p>
    <p class="data-status" :class="{ 'has-new': hasNewVersion }">
      <span class="clickable" @click="openVersions">{{ statusText }}</span>
      <span v-if="info" class="patch-ver">数据版本：{{ info.patchVersion }}</span>
    </p>

    <teleport to="body">
      <div v-if="showVersions" class="modal-mask" @click.self="showVersions = false">
        <div class="modal">
          <h3>当前支持防撤回的版本</h3>
          <div class="versions">
            <section v-for="app in supported" :key="app.appKey">
              <h4>{{ app.appKey }}</h4>
              <p v-for="v in app.versions" :key="v">{{ v }}</p>
            </section>
          </div>
          <button class="close-btn" @click="showVersions = false">关闭</button>
        </div>
      </div>
    </teleport>
  </div>
</template>

<style scoped>
.notice-banner {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.notice {
  margin: 0;
  padding: 8px 10px;
  border: 1px solid #e6a23c66;
  border-radius: 6px;
  background: #fdf6ec;
  color: #b88230;
  font-size: 13px;
}
.data-status {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.clickable {
  cursor: pointer;
}
.clickable:hover {
  text-decoration: underline;
}
.has-new .clickable {
  color: #d94040;
  font-weight: 600;
}
.patch-ver {
  cursor: default;
}
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  background: var(--bg);
  border-radius: 10px;
  padding: 20px;
  width: 480px;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.modal h3 {
  margin: 0;
  font-size: 15px;
}
.versions {
  overflow-y: auto;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 20px;
}
.versions h4 {
  margin: 6px 0 2px;
  font-size: 13px;
  color: var(--accent);
}
.versions p {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
}
.close-btn {
  align-self: flex-end;
  padding: 6px 18px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-soft);
  color: var(--text);
  cursor: pointer;
}
</style>
