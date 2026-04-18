<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { focusFirstElement, trapTabKey } from "../utils/a11y";

const props = defineProps<{
  visible: boolean;
  settingsSaving: boolean;
  projectDataDir: string;
  dictDir: string;
  hotkey: string;
  searchEngine: "google" | "bing" | "baidu";
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:dictDir": [value: string];
  "update:hotkey": [value: string];
  "update:searchEngine": [value: "google" | "bing" | "baidu"];
}>();

const dialogRef = ref<HTMLElement | null>(null);
let restoreFocusTarget: HTMLElement | null = null;
let backdropPointerDown = false;

const dictDirModel = computed({
  get: () => props.dictDir,
  set: (value: string) => emit("update:dictDir", value),
});

const hotkeyModel = computed({
  get: () => props.hotkey,
  set: (value: string) => emit("update:hotkey", value),
});

const searchEngineModel = computed({
  get: () => props.searchEngine,
  set: (value: "google" | "bing" | "baidu") => emit("update:searchEngine", value),
});

function closeDialog(): void {
  emit("close");
}

function onMaskPointerDown(event: PointerEvent): void {
  backdropPointerDown = event.target === event.currentTarget;
}

function onMaskClick(event: MouseEvent): void {
  if (event.target !== event.currentTarget) {
    backdropPointerDown = false;
    return;
  }
  if (backdropPointerDown) {
    closeDialog();
  }
  backdropPointerDown = false;
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

function normalizeHotkeyFromKeyboardEvent(event: KeyboardEvent): string | null {
  if (event.metaKey) {
    return null;
  }

  const keyToken = extractHotkeyKeyToken(event);
  if (!keyToken) {
    return null;
  }

  const ctrl = event.ctrlKey;
  const alt = event.altKey;
  const shift = event.shiftKey;
  const validModifierCombo = (alt && !ctrl && !shift) || (ctrl && alt && !shift);
  if (!validModifierCombo) {
    return null;
  }

  const parts: string[] = [];
  if (ctrl) {
    parts.push("Ctrl");
  }
  if (alt) {
    parts.push("Alt");
  }
  if (shift) {
    parts.push("Shift");
  }
  parts.push(keyToken);
  return parts.join("+");
}

function extractHotkeyKeyToken(event: KeyboardEvent): string | null {
  const code = event.code ?? "";
  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3);
  }
  if (/^Digit[0-9]$/.test(code)) {
    return code.slice(5);
  }

  const rawKey = event.key;
  if (rawKey === "Control" || rawKey === "Alt" || rawKey === "Shift") {
    return null;
  }
  if (/^[a-z0-9]$/i.test(rawKey)) {
    return rawKey.toUpperCase();
  }
  return null;
}

function onHotkeyInputKeydown(event: KeyboardEvent): void {
  if (event.key === "Backspace" || event.key === "Delete") {
    hotkeyModel.value = "";
    event.preventDefault();
    return;
  }

  const normalized = normalizeHotkeyFromKeyboardEvent(event);
  if (!normalized) {
    return;
  }
  hotkeyModel.value = normalized;
  event.preventDefault();
}

async function setGlobalHotkeyEnabled(enabled: boolean): Promise<void> {
  try {
    await invoke("set_hotkey_enabled", { enabled });
  } catch {
    // Ignore bridge errors to keep settings input usable.
  }
}

function onHotkeyInputFocus(): void {
  void setGlobalHotkeyEnabled(false);
}

function onHotkeyInputBlur(): void {
  void setGlobalHotkeyEnabled(true);
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
    void setGlobalHotkeyEnabled(true);
    if (restoreFocusTarget && document.contains(restoreFocusTarget)) {
      restoreFocusTarget.focus();
    }
    restoreFocusTarget = null;
  },
);

onBeforeUnmount(() => {
  void setGlobalHotkeyEnabled(true);
});
</script>

<template>
  <div
    v-if="visible"
    class="settings-mask"
    @pointerdown="onMaskPointerDown"
    @click.self="onMaskClick"
  >
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
          placeholder="Ctrl+Alt+D"
          @keydown="onHotkeyInputKeydown"
          @pointerdown="onHotkeyInputFocus"
          @focus="onHotkeyInputFocus"
          @blur="onHotkeyInputBlur"
        />
        <small>支持：Alt+键、Ctrl+Alt+键（例如 Alt+D、Ctrl+Alt+D）。</small>
      </label>

      <label class="field">
        <span>Ctrl+左键搜索引擎</span>
        <select v-model="searchEngineModel">
          <option value="google">Google</option>
          <option value="bing">Bing</option>
          <option value="baidu">Baidu</option>
        </select>
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
