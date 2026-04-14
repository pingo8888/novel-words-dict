<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Lock, Mars, Settings, Venus, VenusAndMars } from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";

type NameType = "both" | "surname" | "given" | "place";
type GenderType = "both" | "male" | "female";
type GenreType = "east" | "west";
type ToastTone = "info" | "error";
type NameTypeFilter = "all" | NameType;
type GenderTypeFilter = "all" | GenderType;
type GenreTypeFilter = "all" | GenreType;

interface NameEntry {
  term: string;
  group: string;
  nameType: NameType;
  genderType: GenderType;
  genre: GenreType;
  dictId: string;
  dictName: string;
  editable: boolean;
}

interface QueryResponse {
  items: NameEntry[];
  total: number;
  totalAll: number;
  page: number;
  pageCount: number;
}

interface QueryRequest {
  dictId: string;
  genreType: GenreTypeFilter;
  nameType: NameTypeFilter;
  genderType: GenderTypeFilter;
  keyword: string;
  page: number;
}

interface DictionaryOption {
  id: string;
  name: string;
  editable: boolean;
}

interface AppSettingsResponse {
  dictDir: string;
  hotkey: string;
  projectDataDir: string;
}

const filters = reactive<QueryRequest>({
  dictId: "all",
  genreType: "all",
  nameType: "all",
  genderType: "all",
  keyword: "",
  page: 1,
});

const loading = ref(false);
const queryButtonLoading = ref(false);
const toastMessage = ref("");
const toastTone = ref<ToastTone>("info");
const activeHotkey = ref("Alt+Z");
const dictionaries = ref<DictionaryOption[]>([
  { id: "all", name: "所有词库", editable: false },
  { id: "custom", name: "自定词库", editable: true },
]);
const settingsVisible = ref(false);
const settingsSaving = ref(false);
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

const renderItems = computed<(NameEntry | null)[]>(() => {
  const filled: (NameEntry | null)[] = [...result.value.items];
  while (filled.length < 40) {
    filled.push(null);
  }
  return filled;
});

const pageDisplay = computed(() => `${result.value.page}/${result.value.pageCount}`);
const isGenderFilterEditable = computed(
  () => filters.nameType === "surname" || filters.nameType === "given",
);

let unlistenEntryUpdated: (() => void) | null = null;
let unlistenEditorOpenRequest: (() => void) | null = null;
let toastTimer: number | null = null;

function resolveErrorMessage(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : fallback;
}

async function createEditorWindow(): Promise<void> {
  const existing = await WebviewWindow.getByLabel("editor");
  if (existing) {
    try {
      await existing.emit("editor-seed-updated");
      await existing.setAlwaysOnTop(true);
      await existing.show();
      await existing.setFocus();
      return;
    } catch {
      try {
        await existing.close();
      } catch {
        // Ignore stale window close errors.
      }
    }
  }

  const editor = new WebviewWindow("editor", {
    url: "/editor.html",
    title: "编辑词条",
    width: 540,
    height: 400,
    minWidth: 540,
    minHeight: 400,
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
  if (loading.value) {
    return;
  }

  if (resetPage) {
    filters.page = 1;
  }

  loading.value = true;
  queryButtonLoading.value = resetPage;
  try {
    const response = await invoke<QueryResponse>("query_entries", {
      request: { ...filters },
    });
    result.value = response;
    filters.page = response.page;
  } catch (error) {
    showToast(resolveErrorMessage(error, "查询失败，请稍后重试"), "error");
  } finally {
    loading.value = false;
    queryButtonLoading.value = false;
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

async function openEditor(entry: NameEntry): Promise<void> {
  if (!entry.editable) {
    showToast("内置词库词条不可编辑", "error");
    return;
  }
  try {
    await invoke("set_editor_seed", { term: entry.term });
    await createEditorWindow();
  } catch (error) {
    showToast(resolveErrorMessage(error, "打开编辑窗口失败"), "error");
  }
}

async function copyTerm(term: string): Promise<void> {
  const text = term.trim();
  if (!text) {
    return;
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      showToast(`复制：${text}`);
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
    showToast(`复制：${text}`);
  } catch (error) {
    showToast(resolveErrorMessage(error, "复制词条失败"), "error");
  }
}

function showToast(message: string, tone: ToastTone = "info"): void {
  toastMessage.value = message;
  toastTone.value = tone;
  if (toastTimer !== null) {
    window.clearTimeout(toastTimer);
  }
  toastTimer = window.setTimeout(() => {
    toastMessage.value = "";
    toastTimer = null;
  }, 1800);
}

function getNameTypeIcons(nameType: NameType): string[] {
  if (nameType === "surname") {
    return ["姓"];
  }
  if (nameType === "given") {
    return ["名"];
  }
  if (nameType === "place") {
    return ["地"];
  }
  return ["姓", "名"];
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

async function loadDictionaries(): Promise<void> {
  const items = await invoke<DictionaryOption[]>("list_dictionaries");
  const normalized = items.length
    ? items
    : [
        { id: "all", name: "所有词库", editable: false },
        { id: "custom", name: "自定词库", editable: true },
      ];
  dictionaries.value = normalized;
  if (!normalized.some((item) => item.id === filters.dictId)) {
    filters.dictId = normalized[0].id;
  }
}

async function openSettings(): Promise<void> {
  try {
    await loadSettings();
    settingsVisible.value = true;
  } catch (error) {
    showToast(resolveErrorMessage(error, "读取设置失败"), "error");
  }
}

function closeSettings(): void {
  if (settingsSaving.value) {
    return;
  }
  settingsVisible.value = false;
}

async function saveSettings(): Promise<void> {
  settingsSaving.value = true;
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
    await loadDictionaries();
    await query(true);
  } catch (error) {
    showToast(resolveErrorMessage(error, "保存设置失败"), "error");
  } finally {
    settingsSaving.value = false;
  }
}

onMounted(async () => {
  try {
    await loadSettings();
    await loadDictionaries();
  } catch (error) {
    showToast(resolveErrorMessage(error, "读取设置失败"), "error");
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
      showToast(resolveErrorMessage(error, "打开编辑窗口失败"), "error");
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
  if (toastTimer !== null) {
    window.clearTimeout(toastTimer);
    toastTimer = null;
  }
});

watch(
  () => filters.nameType,
  () => {
    if (!isGenderFilterEditable.value && filters.genderType !== "all") {
      filters.genderType = "all";
    }
  },
  { immediate: true },
);
</script>

<template>
  <main class="home-page">
    <div class="top-row">
      <p class="description-inline">
        当前词条数{{ result.totalAll }}，当前取词快捷键{{ activeHotkey }}
      </p>
      <button class="settings-icon-btn" type="button" title="设置" @click="openSettings">
        <Settings :size="16" :stroke-width="2" />
      </button>
    </div>

    <section class="filters">
      <label class="field">
        <span>词库</span>
        <select v-model="filters.dictId">
          <option v-for="item in dictionaries" :key="item.id" :value="item.id">
            {{ item.name }}
          </option>
        </select>
      </label>

      <label class="field">
        <span>风格</span>
        <select v-model="filters.genreType">
          <option value="all">所有</option>
          <option value="east">东方</option>
          <option value="west">西方</option>
        </select>
      </label>

      <label class="field">
        <span>名词类型</span>
        <select v-model="filters.nameType">
          <option value="all">所有</option>
          <option value="surname">姓氏</option>
          <option value="given">名字</option>
          <option value="place">地名</option>
        </select>
      </label>

      <label class="field">
        <span>性别</span>
        <select v-model="filters.genderType" :disabled="!isGenderFilterEditable">
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

      <button class="query-btn" type="button" :disabled="queryButtonLoading" @click="query(true)">
        {{ queryButtonLoading ? "查询中..." : "查询" }}
      </button>
    </section>

    <section class="result-panel">
      <div class="result-summary">
        <span>命中词条：{{ result.total }}</span>
      </div>

      <div class="entry-grid">
        <button
          v-for="(entry, index) in renderItems"
          :key="entry ? `${entry.dictId}-${entry.term}-${index}` : `empty-${index}`"
          class="entry-item"
          :class="{ placeholder: !entry }"
          type="button"
          :disabled="!entry"
          @click="entry && copyTerm(entry.term)"
          @contextmenu.prevent="entry && openEditor(entry)"
        >
          <template v-if="entry">
            <span v-if="!entry.editable" class="entry-lock-corner" title="内置词条不可编辑">
              <Lock class="entry-lucide" :size="12" :stroke-width="2" />
            </span>
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
                <Mars
                  v-if="entry.genderType === 'male'"
                  class="entry-lucide"
                  :size="12"
                  :stroke-width="2"
                />
                <Venus
                  v-else-if="entry.genderType === 'female'"
                  class="entry-lucide"
                  :size="12"
                  :stroke-width="2"
                />
                <VenusAndMars
                  v-else
                  class="entry-lucide"
                  :size="12"
                  :stroke-width="2"
                />
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

    <p v-if="toastMessage" class="system-tip" :class="`tone-${toastTone}`">{{ toastMessage }}</p>

    <div
      v-if="settingsVisible"
      class="settings-mask"
      @click.self="closeSettings"
    >
      <section class="settings-dialog">
        <h2>设置</h2>
        <label class="field">
          <span>自定词库保存目录</span>
          <input
            v-model="settingsForm.dictDir"
            type="text"
            placeholder="请输入目录路径"
          />
          <small>默认值：{{ projectDataDir }}</small>
        </label>

        <label class="field">
          <span>界面取词快捷键</span>
          <input
            v-model="settingsForm.hotkey"
            type="text"
            maxlength="16"
            placeholder="Alt+Z"
          />
          <small>当前仅支持 Alt + 单个英文字母，例如 Alt+Z。</small>
        </label>

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
