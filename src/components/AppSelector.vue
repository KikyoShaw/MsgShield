<script setup lang="ts">
defineProps<{
  appList: readonly { key: string; label: string }[];
  current: string;
  detected: Record<string, string | null>;
}>();

const emit = defineEmits<{ select: [key: string] }>();
</script>

<template>
  <div class="app-selector">
    <button
      v-for="app in appList"
      :key="app.key"
      class="app-btn"
      :class="{ active: app.key === current, detected: detected[app.key] }"
      :title="detected[app.key] ?? '未检测到安装路径'"
      @click="emit('select', app.key)"
    >
      {{ app.label }}
      <span v-if="detected[app.key]" class="dot" />
    </button>
  </div>
</template>

<style scoped>
.app-selector {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.app-btn {
  position: relative;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-soft);
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;
}
.app-btn:hover {
  border-color: var(--accent);
}
.app-btn.active {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}
.dot {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #4caf50;
}
</style>
