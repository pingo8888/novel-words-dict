<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { focusFirstElement, trapTabKey } from "../utils/a11y";

const props = defineProps<{
  visible: boolean;
  settingsSaving: boolean;
  projectDataDir: string;
  dictDir: string;
  hotkey: string;
  searchEngine: "google" | "bing" | "baidu";
  updateChecking: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  checkUpdate: [];
  "open-dir-failed": [message: string];
  "update:hotkey": [value: string];
  "update:searchEngine": [value: "google" | "bing" | "baidu"];
}>();

const dialogRef = ref<HTMLElement | null>(null);
let restoreFocusTarget: HTMLElement | null = null;
let backdropPointerDown = false;

const hotkeyModel = computed({
  get: () => props.hotkey,
  set: (value: string) => emit("update:hotkey", value),
});

const searchEngineModel = computed({
  get: () => props.searchEngine,
  set: (value: "google" | "bing" | "baidu") => emit("update:searchEngine", value),
});
const dictDirDisplay = computed(() => {
  const dictDir = props.dictDir.trim();
  if (dictDir) {
    return dictDir;
  }
  return props.projectDataDir.trim();
});

async function openDictDir(): Promise<void> {
  const target = dictDirDisplay.value;
  if (!target) {
    return;
  }
  try {
    await openPath(target);
  } catch {
    emit("open-dir-failed", "打开目录失败");
  }
}

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
      <div class="settings-header">
        <h2 id="settings-dialog-title">设置</h2>
        <button
          type="button"
          class="settings-close-btn"
          aria-label="关闭设置"
          title="关闭"
          @click="closeDialog"
        >
          ×
        </button>
      </div>
      <p id="settings-dialog-desc" class="settings-desc">
        查看词库保存目录、修改快捷键和搜索设置，按 Esc 可关闭对话框。
      </p>
      <div class="field">
        <span>打开自定义词库目录</span>
        <div class="dict-dir-row">
          <input
            class="dict-dir-path"
            :value="dictDirDisplay"
            :title="dictDirDisplay"
            type="text"
            readonly
          />
          <button
            type="button"
            class="secondary dict-dir-open-btn"
            :disabled="!dictDirDisplay"
            @click="openDictDir"
          >
            打开
          </button>
        </div>
      </div>

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
        <button
          type="button"
          class="secondary check-update"
          :disabled="settingsSaving || updateChecking"
          @click="emit('checkUpdate')"
        >
          {{ updateChecking ? "检查中..." : "检查更新" }}
        </button>
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
