<script setup lang="ts">
import { watch, ref } from "vue";
import type { AppStatus } from "../api/patcher";

const props = defineProps<{ status: AppStatus | null }>();

const selected = ref<string[]>([]);

// 状态变化时重置勾选：默认勾选所有未安装的功能
watch(
  () => props.status,
  (s) => {
    if (s?.state === "fuzzy") {
      selected.value = s.categories.filter((c) => !c.installed).map((c) => c.name);
    } else {
      selected.value = [];
    }
  },
);

function toggle(name: string) {
  const i = selected.value.indexOf(name);
  if (i >= 0) selected.value.splice(i, 1);
  else selected.value.push(name);
}

defineExpose({ selected });
</script>

<template>
  <div class="feature-panel">
    <!-- 精确模式：无勾选框 -->
    <p v-if="status?.state === 'exact'" class="hint">
      只有基于特征的补丁才能选择功能
    </p>
    <!-- 路径无效/不支持 -->
    <p v-else-if="status?.state !== 'fuzzy'" class="hint dim">无功能选项</p>
    <!-- 特征码模式：功能勾选 -->
    <template v-else>
      <label
        v-for="cat in status.categories"
        :key="cat.name"
        class="feature-item"
        :class="{ installed: cat.installed }"
      >
        <input
          type="checkbox"
          :checked="selected.includes(cat.name)"
          :disabled="cat.installed"
          @change="toggle(cat.name)"
        />
        <span>{{ cat.name }}{{ cat.installed ? "（已安装）" : "" }}</span>
      </label>
    </template>
  </div>
</template>

<style scoped>
.feature-panel {
  min-height: 90px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-soft);
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: 10px 18px;
}
.hint {
  margin: 0;
  font-size: 13px;
  color: var(--text-dim);
}
.hint.dim {
  opacity: 0.7;
}
.feature-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  cursor: pointer;
  user-select: none;
}
.feature-item.installed {
  color: var(--text-dim);
  cursor: default;
}
.feature-item input {
  accent-color: var(--accent);
}
</style>
