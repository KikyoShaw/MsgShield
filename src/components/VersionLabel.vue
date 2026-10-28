<script setup lang="ts">
import { computed } from "vue";
import type { AppStatus } from "../api/patcher";

const props = defineProps<{ status: AppStatus | null; checking?: boolean }>();

const text = computed(() => {
  if (props.checking) return "正在检测...";
  if (!props.status) return "";
  if (!props.status.allFilesExist) return "路径下未找到关键文件";
  const v = props.status.version;
  switch (props.status.state) {
    case "exact":
      return `${v}（已支持）`;
    case "fuzzy":
      return `${v}（支持特征防撤回）`;
    default:
      return `${v}（不支持）`;
  }
});

const colorClass = computed(() => {
  if (props.checking) return "dim";
  if (!props.status || !props.status.allFilesExist) return "red";
  switch (props.status.state) {
    case "exact":
      return "green";
    case "fuzzy":
      return "lime";
    default:
      return "red";
  }
});
</script>

<template>
  <div v-if="text" class="version-label" :class="colorClass">{{ text }}</div>
</template>

<style scoped>
.version-label {
  font-size: 14px;
  font-weight: 600;
}
.green {
  color: #2e9e44;
}
.lime {
  color: #4caf50;
}
.red {
  color: #d94040;
}
.dim {
  color: var(--text-dim);
  font-weight: 400;
}
</style>
