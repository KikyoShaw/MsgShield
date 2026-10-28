<script setup lang="ts">
defineProps<{
  busy: boolean;
  canPatch: boolean;
  canRestore: boolean;
}>();

const emit = defineEmits<{ patch: []; restore: [] }>();
</script>

<template>
  <div class="action-bar">
    <button
      class="primary-btn"
      :disabled="busy || !canPatch"
      @click="emit('patch')"
    >
      补丁安装
    </button>
    <button
      class="ghost-btn"
      :disabled="busy || !canRestore"
      title="从 .h.bak 备份还原文件"
      @click="emit('restore')"
    >
      恢复原文件
    </button>
  </div>
</template>

<style scoped>
.action-bar {
  display: flex;
  gap: 12px;
}
.primary-btn,
.ghost-btn {
  flex: 1;
  padding: 9px 0;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s;
}
.primary-btn {
  border: 1px solid var(--accent);
  background: var(--accent);
  color: #fff;
}
.primary-btn:hover:not(:disabled) {
  filter: brightness(1.08);
}
.ghost-btn {
  border: 1px solid var(--border);
  background: var(--bg-soft);
  color: var(--text);
}
.ghost-btn:hover:not(:disabled) {
  border-color: var(--accent);
}
.primary-btn:disabled,
.ghost-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
