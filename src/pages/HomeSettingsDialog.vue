<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { focusFirstElement, trapTabKey } from "../utils/a11y";

const props = defineProps<{
  visible: boolean;
  settingsSaving: boolean;
  projectDataDir: string;
  dictDir: string;
  hotkey: string;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:dictDir": [value: string];
  "update:hotkey": [value: string];
}>();

const dialogRef = ref<HTMLElement | null>(null);
let restoreFocusTarget: HTMLElement | null = null;

const dictDirModel = computed({
  get: () => props.dictDir,
  set: (value: string) => emit("update:dictDir", value),
});

const hotkeyModel = computed({
  get: () => props.hotkey,
  set: (value: string) => emit("update:hotkey", value),
});

function closeDialog(): void {
  emit("close");
}

function onDialogKeydown(event: KeyboardEvent): void {
  if (!dialogRef.value) {
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    closeDialog();
    return;
  }
  trapTabKey(event, dialogRef.value);
}

watch(
  () => props.visible,
  async (visible) => {
    if (visible) {
      restoreFocusTarget = document.activeElement as HTMLElement | null;
      await nextTick();
      if (dialogRef.value) {
        focusFirstElement(dialogRef.value);
      }
      return;
    }
    if (restoreFocusTarget && document.contains(restoreFocusTarget)) {
      restoreFocusTarget.focus();
    }
    restoreFocusTarget = null;
  },
);
</script>

<template>
  <div v-if="visible" class="settings-mask" @click.self="closeDialog">
    <section
      ref="dialogRef"
      class="settings-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-dialog-title"
      aria-describedby="settings-dialog-desc"
      tabindex="-1"
      @keydown="onDialogKeydown"
    >
      <h2 id="settings-dialog-title">设置</h2>
      <p id="settings-dialog-desc" class="settings-desc">
        修改词库目录与快捷键，按 Esc 可关闭对话框。
      </p>
      <label class="field">
        <span>自定词库保存目录</span>
        <input
          v-model="dictDirModel"
          type="text"
          placeholder="请输入目录路径"
        />
        <small>默认值：{{ projectDataDir }}</small>
        <small>安全限制：仅允许项目数据目录及其子目录。</small>
      </label>

      <label class="field">
        <span>界面取词快捷键</span>
        <input
          v-model="hotkeyModel"
          type="text"
          maxlength="16"
          placeholder="Alt+Z"
        />
        <small>当前仅支持 Alt + 单个英文字母，例如 Alt+Z。</small>
      </label>

      <div class="settings-actions">
        <button type="button" class="secondary" @click="closeDialog">
          取消
        </button>
        <button type="button" class="primary" :disabled="settingsSaving" @click="emit('save')">
          {{ settingsSaving ? "保存中..." : "保存" }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped src="./HomeSettingsDialog.scoped.css"></style>
