<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";

const path = defineModel<string>({ required: true });

async function browse() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    path.value = selected;
  }
}
</script>

<template>
  <div class="path-bar">
    <input
      v-model="path"
      class="path-input"
      type="text"
      placeholder="选择或输入安装路径..."
      spellcheck="false"
    />
    <button class="browse-btn" @click="browse">浏览...</button>
  </div>
</template>

<style scoped>
.path-bar {
  display: flex;
  gap: 8px;
}
.path-input {
  flex: 1;
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-soft);
  color: var(--text);
  font-size: 13px;
  min-width: 0;
}
.path-input:focus {
  outline: none;
  border-color: var(--accent);
}
.browse-btn {
  padding: 7px 14px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-soft);
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
  white-space: nowrap;
}
.browse-btn:hover {
  border-color: var(--accent);
}
</style>
