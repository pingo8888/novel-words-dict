<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";

type NameType = "both" | "surname" | "given";
type GenderType = "both" | "male" | "female";
type NameTypeFilter = "all" | NameType;
type GenderTypeFilter = "all" | GenderType;
type InitialFilter = "all" | string;

interface NameEntry {
  term: string;
  group: string;
  nameType: NameType;
  genderType: GenderType;
}

interface QueryResponse {
  items: NameEntry[];
  total: number;
  totalAll: number;
  page: number;
  pageCount: number;
}

interface QueryRequest {
  initial: InitialFilter;
  nameType: NameTypeFilter;
  genderType: GenderTypeFilter;
  keyword: string;
  page: number;
}

interface AppSettingsResponse {
  dictDir: string;
  hotkey: string;
  projectDataDir: string;
}

const filters = reactive<QueryRequest>({
  initial: "all",
  nameType: "all",
  genderType: "all",
  keyword: "",
  page: 1,
});

const loading = ref(false);
const errorMessage = ref("");
const openEditorError = ref("");
const copyTipMessage = ref("");
const activeHotkey = ref("Alt+Z");
const settingsVisible = ref(false);
const settingsSaving = ref(false);
const settingsError = ref("");
const projectDataDir = ref("");
const settingsForm = reactive({
  dictDir: "",
  hotkey: "Alt+Z",
});
const result = ref<QueryResponse>({
  items: [],
  total: 0,
  totalAll: 0,
  page: 1,
  pageCount: 1,
});

const initials = Array.from({ length: 26 }, (_, index) =>
  String.fromCharCode(65 + index),
);

const renderItems = computed<(NameEntry | null)[]>(() => {
  const filled: (NameEntry | null)[] = [...result.value.items];
  while (filled.length < 50) {
    filled.push(null);
  }
  return filled;
});

const pageDisplay = computed(() => `${result.value.page}/${result.value.pageCount}`);

let unlistenEntryUpdated: (() => void) | null = null;
let unlistenEditorOpenRequest: (() => void) | null = null;
let copyTipTimer: number | null = null;

async function createEditorWindow(): Promise<void> {
  const existing = await WebviewWindow.getByLabel("editor");
  if (existing) {
    await existing.close();
    await new Promise((resolve) => window.setTimeout(resolve, 30));
  }

  const editor = new WebviewWindow("editor", {
    url: "/editor.html",
    title: "编辑词条",
    width: 540,
    height: 450,
    minWidth: 540,
    minHeight: 450,
    resizable: false,
    center: true,
    focus: true,
    alwaysOnTop: true,
  });

  await new Promise<void>((resolve, reject) => {
    editor.once("tauri://created", () => resolve());
    editor.once("tauri://error", (event) =>
      reject(new Error(String(event.payload ?? "未知错误"))),
    );
  });

  try {
    await editor.setAlwaysOnTop(true);
  } catch {
    // Ignore optional z-order failures from OS focus policy.
  }
  try {
    await editor.show();
  } catch {
    // Window is usually visible already.
  }
  try {
    await editor.setFocus();
  } catch {
    // Ignore focus-steal restrictions on Windows.
  }
}

async function query(resetPage = false): Promise<void> {
  if (resetPage) {
    filters.page = 1;
  }

  loading.value = true;
  errorMessage.value = "";
  try {
    const response = await invoke<QueryResponse>("query_entries", {
      request: { ...filters },
    });
    result.value = response;
    filters.page = response.page;
  } catch (error) {
    errorMessage.value =
      typeof error === "string" ? error : "查询失败，请稍后重试";
  } finally {
    loading.value = false;
  }
}

async function prevPage(): Promise<void> {
  if (filters.page <= 1 || loading.value) {
    return;
  }
  filters.page -= 1;
  await query(false);
}

async function nextPage(): Promise<void> {
  if (filters.page >= result.value.pageCount || loading.value) {
    return;
  }
  filters.page += 1;
  await query(false);
}

async function openEditor(term: string): Promise<void> {
  openEditorError.value = "";
  try {
    await invoke("set_editor_seed", { term });
    await createEditorWindow();
  } catch (error) {
    openEditorError.value =
      typeof error === "string" ? error : "打开编辑窗口失败";
  }
}

async function copyTerm(term: string): Promise<void> {
  openEditorError.value = "";
  const text = term.trim();
  if (!text) {
    return;
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      showCopyTip(`复制：${text}`);
      return;
    }

    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    document.body.appendChild(textarea);
    textarea.select();
    const success = document.execCommand("copy");
    document.body.removeChild(textarea);
    if (!success) {
      throw new Error("copy failed");
    }
    showCopyTip(`复制：${text}`);
  } catch (error) {
    openEditorError.value =
      typeof error === "string" ? error : "复制词条失败";
  }
}

function showCopyTip(message: string): void {
  copyTipMessage.value = message;
  if (copyTipTimer !== null) {
    window.clearTimeout(copyTipTimer);
  }
  copyTipTimer = window.setTimeout(() => {
    copyTipMessage.value = "";
    copyTipTimer = null;
  }, 1800);
}

function getNameTypeIcons(nameType: NameType): string[] {
  if (nameType === "surname") {
    return ["姓"];
  }
  if (nameType === "given") {
    return ["名"];
  }
  return ["姓", "名"];
}

function getGenderIcon(genderType: GenderType): string {
  if (genderType === "male") {
    return "♂";
  }
  if (genderType === "female") {
    return "♀";
  }
  return "⚥";
}

function getGenderIconClass(genderType: GenderType): string {
  if (genderType === "male") {
    return "gender-male";
  }
  if (genderType === "female") {
    return "gender-female";
  }
  return "gender-both";
}

function formatGroupLabel(group: string): string {
  const text = group.trim();
  if (!text) {
    return "〔未分组〕";
  }
  return `〔${text}〕`;
}

async function loadSettings(): Promise<void> {
  const settings = await invoke<AppSettingsResponse>("get_app_settings");
  projectDataDir.value = settings.projectDataDir;
  settingsForm.dictDir = settings.dictDir;
  settingsForm.hotkey = settings.hotkey;
  activeHotkey.value = settings.hotkey;
}

async function openSettings(): Promise<void> {
  settingsError.value = "";
  try {
    await loadSettings();
    settingsVisible.value = true;
  } catch (error) {
    openEditorError.value =
      typeof error === "string" ? error : "读取设置失败";
  }
}

function closeSettings(): void {
  if (settingsSaving.value) {
    return;
  }
  settingsVisible.value = false;
  settingsError.value = "";
}

async function saveSettings(): Promise<void> {
  settingsSaving.value = true;
  settingsError.value = "";
  try {
    const saved = await invoke<AppSettingsResponse>("save_app_settings", {
      request: {
        dictDir: settingsForm.dictDir.trim(),
        hotkey: settingsForm.hotkey.trim(),
      },
    });
    projectDataDir.value = saved.projectDataDir;
    settingsForm.dictDir = saved.dictDir;
    settingsForm.hotkey = saved.hotkey;
    activeHotkey.value = saved.hotkey;
    settingsVisible.value = false;
    await query(true);
  } catch (error) {
    settingsError.value =
      typeof error === "string" ? error : "保存设置失败";
  } finally {
    settingsSaving.value = false;
  }
}

onMounted(async () => {
  try {
    await loadSettings();
  } catch (error) {
    openEditorError.value =
      typeof error === "string" ? error : "读取设置失败";
  }
  await query(true);
  unlistenEntryUpdated = await listen("entry-updated", async () => {
    await query(false);
  });
  unlistenEditorOpenRequest = await listen<string>("editor-open-request", async (event) => {
    try {
      await invoke("set_editor_seed", { term: event.payload ?? "" });
      await createEditorWindow();
    } catch (error) {
      openEditorError.value =
        typeof error === "string" ? error : "打开编辑窗口失败";
    }
  });
});

onBeforeUnmount(() => {
  if (unlistenEntryUpdated) {
    unlistenEntryUpdated();
    unlistenEntryUpdated = null;
  }
  if (unlistenEditorOpenRequest) {
    unlistenEditorOpenRequest();
    unlistenEditorOpenRequest = null;
  }
  if (copyTipTimer !== null) {
    window.clearTimeout(copyTipTimer);
    copyTipTimer = null;
  }
});
</script>

<template>
  <main class="home-page">
    <header class="header">
      <div class="header-main">
        <h1>外国人名词库</h1>
        <p class="description">
          管理和筛选外国人名词条目，支持系统级快捷键 {{ activeHotkey }} 快速取词。
          当前词条数：<strong>{{ result.totalAll }}</strong>
        </p>
      </div>
      <button class="settings-btn" type="button" @click="openSettings">设置</button>
    </header>

    <section class="filters">
      <label class="field">
        <span>首字母</span>
        <select v-model="filters.initial">
          <option value="all">所有</option>
          <option v-for="letter in initials" :key="letter" :value="letter">
            {{ letter }}
          </option>
        </select>
      </label>

      <label class="field">
        <span>姓氏类型</span>
        <select v-model="filters.nameType">
          <option value="all">所有</option>
          <option value="surname">姓氏</option>
          <option value="given">名字</option>
        </select>
      </label>

      <label class="field">
        <span>性别</span>
        <select v-model="filters.genderType">
          <option value="all">所有</option>
          <option value="male">男性</option>
          <option value="female">女性</option>
        </select>
      </label>

      <label class="field keyword">
        <span>关键字</span>
        <input
          v-model="filters.keyword"
          type="text"
          maxlength="120"
          placeholder="输入词条或分组关键字"
          @keyup.enter="query(true)"
        />
      </label>

      <button class="query-btn" type="button" :disabled="loading" @click="query(true)">
        {{ loading ? "查询中..." : "查询" }}
      </button>
    </section>

    <section class="result-panel">
      <div class="result-summary">
        <span>命中词条：{{ result.total }}</span>
        <span v-if="errorMessage" class="error-message">{{ errorMessage }}</span>
      </div>
      <p v-if="openEditorError" class="error-message open-editor-error">{{ openEditorError }}</p>

      <div class="entry-grid">
        <button
          v-for="(entry, index) in renderItems"
          :key="entry ? entry.term : `empty-${index}`"
          class="entry-item"
          :class="{ placeholder: !entry }"
          type="button"
          :disabled="!entry"
          @click="entry && copyTerm(entry.term)"
          @contextmenu.prevent="entry && openEditor(entry.term)"
        >
          <template v-if="entry">
            <div class="entry-icons">
              <span
                v-for="icon in getNameTypeIcons(entry.nameType)"
                :key="`name-${entry.term}-${icon}`"
                class="entry-icon name-type"
              >
                {{ icon }}
              </span>
              <span
                class="entry-icon"
                :class="getGenderIconClass(entry.genderType)"
              >
                {{ getGenderIcon(entry.genderType) }}
              </span>
            </div>
            <div class="entry-main">
              <span class="term">{{ entry.term }}</span>
              <span class="group">{{ formatGroupLabel(entry.group) }}</span>
            </div>
          </template>
        </button>
      </div>

      <div class="pagination">
        <button type="button" :disabled="loading || filters.page <= 1" @click="prevPage">
          上一页
        </button>
        <span>{{ pageDisplay }}</span>
        <button
          type="button"
          :disabled="loading || filters.page >= result.pageCount"
          @click="nextPage"
        >
          下一页
        </button>
      </div>
    </section>

    <p v-if="copyTipMessage" class="copy-tip">{{ copyTipMessage }}</p>

    <div
      v-if="settingsVisible"
      class="settings-mask"
      @click.self="closeSettings"
    >
      <section class="settings-dialog">
        <h2>设置</h2>
        <label class="field">
          <span>词库保存目录</span>
          <input
            v-model="settingsForm.dictDir"
            type="text"
            placeholder="请输入目录路径"
          />
          <small>默认值：{{ projectDataDir }}</small>
        </label>

        <label class="field">
          <span>快捷键</span>
          <input
            v-model="settingsForm.hotkey"
            type="text"
            maxlength="16"
            placeholder="Alt+Z"
          />
          <small>当前仅支持 Alt + 单个英文字母，例如 Alt+Z。</small>
        </label>

        <p v-if="settingsError" class="error-message">{{ settingsError }}</p>

        <div class="settings-actions">
          <button type="button" class="secondary" :disabled="settingsSaving" @click="closeSettings">
            取消
          </button>
          <button type="button" class="primary" :disabled="settingsSaving" @click="saveSettings">
            {{ settingsSaving ? "保存中..." : "保存" }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped src="./HomePage.scoped.css"></style>
