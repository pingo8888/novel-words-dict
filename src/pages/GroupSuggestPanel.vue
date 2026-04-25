<script setup lang="ts">
defineProps<{
  items: string[];
  page: number;
  pageCount: number;
  selectedIndex: number;
}>();

const emit = defineEmits<{
  select: [value: string];
  prevPage: [];
  nextPage: [];
}>();
</script>

<template>
  <div class="group-suggest-panel" role="listbox" aria-label="分组预选">
    <div v-if="items.length > 0" class="group-suggest-list">
      <button
        v-for="(item, index) in items"
        :key="item"
        type="button"
        class="group-suggest-item"
        :class="{ active: index === selectedIndex }"
        role="option"
        :aria-selected="index === selectedIndex"
        @mousedown.prevent="emit('select', item)"
      >
        {{ item }}
      </button>
    </div>
    <div v-else class="group-suggest-empty">无匹配分组</div>

    <div class="group-suggest-hints">
      <span>第 {{ page }}/{{ pageCount }} 页</span>
      <button type="button" :disabled="page <= 1" @mousedown.prevent="emit('prevPage')">← 上页</button>
      <button type="button" :disabled="page >= pageCount" @mousedown.prevent="emit('nextPage')">→ 下页</button>
      <span>↑/↓ 选择，Enter 确认</span>
    </div>
  </div>
</template>

<style scoped src="./GroupSuggestPanel.scoped.css"></style>
