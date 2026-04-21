import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useHomeSettings } from "./useHomeSettings";
import { useToast } from "../composables/useToast";
import type {
  GenderType,
  GenderTypeFilter,
  GenreTypeFilter,
  NameTypeFilter,
  NameType,
  QueryNameEntry,
} from "../types/dict";
import { resolveErrorMessage } from "../utils/error";
import { isGenderEditableByNameType } from "../utils/nameType";

interface QueryResponse {
  items: QueryNameEntry[];
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

type SearchEngine = "google" | "bing" | "baidu";

interface UpdateConfirmOptions {
  title: string;
  message: string;
  confirmText: string;
  cancelText: string;
}

export function useHomePage() {
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
  const updateChecking = ref(false);
  const updateConfirmVisible = ref(false);
  const updateConfirmTitle = ref("");
  const updateConfirmMessage = ref("");
  const updateConfirmConfirmText = ref("确认");
  const updateConfirmCancelText = ref("取消");
  const appVersion = ref("");
  const { showToast, toastMessage, toastTone } = useToast();
  const dictionaries = ref<DictionaryOption[]>([
    { id: "all", name: "所有词库", editable: false },
    { id: "custom", name: "自定词库", editable: true },
  ]);
  const {
    activeHotkey,
    closeSettings,
    initializeSettings,
    openSettings,
    projectDataDir,
    saveSettings,
    settingsForm,
    settingsSaving,
    settingsVisible,
  } = useHomeSettings({
    onError: (message) => showToast(message, "error"),
    onAfterSave: async () => {
      await loadDictionaries();
      await query(true);
    },
  });
  const result = ref<QueryResponse>({
    items: [],
    total: 0,
    totalAll: 0,
    page: 1,
    pageCount: 1,
  });

  const renderItems = computed<QueryNameEntry[]>(() => result.value.items);
  const pageDisplay = computed(() => `${result.value.page}/${result.value.pageCount}`);
  const isGenderFilterEditable = computed(() =>
    filters.nameType === "all" ? false : isGenderEditableByNameType(filters.nameType),
  );

  let unlistenEntryUpdated: (() => void) | null = null;
  let unlistenEditorOpenRequest: (() => void) | null = null;
  let startupUpdateTimer: ReturnType<typeof setTimeout> | null = null;
  let wheelPageCooldownTimer: ReturnType<typeof setTimeout> | null = null;
  let resolveUpdateConfirm: ((confirmed: boolean) => void) | null = null;
  let wheelPageLocked = false;
  let wheelDeltaAccumulator = 0;

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

  function handleResultWheel(event: WheelEvent): void {
    if (event.ctrlKey) {
      return;
    }
    if (loading.value || result.value.pageCount <= 1) {
      return;
    }
    if (settingsVisible.value) {
      return;
    }

    wheelDeltaAccumulator += event.deltaY;
    const threshold = 60;
    if (Math.abs(wheelDeltaAccumulator) < threshold) {
      return;
    }

    const direction = wheelDeltaAccumulator > 0 ? 1 : -1;
    wheelDeltaAccumulator = 0;

    if (wheelPageLocked) {
      return;
    }

    if (direction > 0 && filters.page >= result.value.pageCount) {
      return;
    }
    if (direction < 0 && filters.page <= 1) {
      return;
    }

    wheelPageLocked = true;
    event.preventDefault();
    if (wheelPageCooldownTimer) {
      clearTimeout(wheelPageCooldownTimer);
    }
    wheelPageCooldownTimer = setTimeout(() => {
      wheelPageLocked = false;
    }, 240);

    if (direction > 0) {
      void nextPage();
      return;
    }
    void prevPage();
  }

  async function openEditor(entry: QueryNameEntry): Promise<void> {
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

  function buildSearchText(entry: QueryNameEntry): string {
    const term = entry.term.trim();
    const group = entry.group.trim();
    if (!term) {
      return "";
    }
    return group ? `${group} ${term}` : term;
  }

  function buildSearchUrl(text: string, searchEngine: SearchEngine): URL {
    if (searchEngine === "bing") {
      const url = new URL("https://www.bing.com/search");
      url.searchParams.set("q", text);
      return url;
    }
    if (searchEngine === "baidu") {
      const url = new URL("https://www.baidu.com/s");
      url.searchParams.set("wd", text);
      return url;
    }
    const url = new URL("https://www.google.com/search");
    url.searchParams.set("q", text);
    return url;
  }

  async function searchTermInBrowser(entry: QueryNameEntry): Promise<void> {
    const text = buildSearchText(entry);
    if (!text) {
      return;
    }
    const url = buildSearchUrl(text, settingsForm.searchEngine);
    try {
      await openUrl(url);
      showToast(`搜索：${text}`);
    } catch (error) {
      showToast(resolveErrorMessage(error, "打开浏览器搜索失败"), "error");
    }
  }

  async function handleEntryClick(event: MouseEvent, entry: QueryNameEntry): Promise<void> {
    if (event.ctrlKey && event.button === 0) {
      event.preventDefault();
      await searchTermInBrowser(entry);
      return;
    }
    await copyTerm(entry.term);
  }

  function buildUpdateConfirmText(update: Update): string {
    const notes = (update.body ?? "").trim();
    const notesPreview = notes.length > 400 ? `${notes.slice(0, 400)}...` : notes;
    const notesText = notesPreview ? `\n\n更新说明：\n${notesPreview}` : "";
    return `发现新版本 ${update.version}（当前 ${update.currentVersion}）。是否立即下载并安装？${notesText}`;
  }

  function resolveAndCloseUpdateConfirm(confirmed: boolean): void {
    const resolver = resolveUpdateConfirm;
    resolveUpdateConfirm = null;
    updateConfirmVisible.value = false;
    if (resolver) {
      resolver(confirmed);
    }
  }

  function acceptUpdateConfirm(): void {
    resolveAndCloseUpdateConfirm(true);
  }

  function cancelUpdateConfirm(): void {
    resolveAndCloseUpdateConfirm(false);
  }

  function requestUpdateConfirm(options: UpdateConfirmOptions): Promise<boolean> {
    if (resolveUpdateConfirm) {
      resolveUpdateConfirm(false);
      resolveUpdateConfirm = null;
    }
    updateConfirmTitle.value = options.title;
    updateConfirmMessage.value = options.message;
    updateConfirmConfirmText.value = options.confirmText;
    updateConfirmCancelText.value = options.cancelText;
    updateConfirmVisible.value = true;

    return new Promise<boolean>((resolve) => {
      resolveUpdateConfirm = resolve;
    });
  }

  async function installUpdate(update: Update): Promise<void> {
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        showToast("开始下载更新...");
        return;
      }
      if (event.event === "Finished") {
        showToast("下载完成，正在安装...");
      }
    });
    const shouldRelaunch = await requestUpdateConfirm({
      title: "更新安装完成",
      message: "是否立即重启应用？",
      confirmText: "立即重启",
      cancelText: "稍后重启",
    });
    if (!shouldRelaunch) {
      showToast("更新已安装，请稍后重启应用生效");
      return;
    }
    await relaunch();
  }

  async function checkForUpdates(manual = false): Promise<void> {
    if (!manual && import.meta.env.DEV) {
      return;
    }

    if (updateChecking.value) {
      if (manual) {
        showToast("正在检查更新，请稍候");
      }
      return;
    }

    updateChecking.value = true;
    try {
      const update = await check();
      if (!update) {
        if (manual) {
          showToast("当前已是最新版本");
        }
        return;
      }

      const shouldInstall = await requestUpdateConfirm({
        title: "发现新版本",
        message: buildUpdateConfirmText(update),
        confirmText: "下载并安装",
        cancelText: "暂不更新",
      });
      if (!shouldInstall) {
        if (manual) {
          showToast("已取消更新");
        }
        return;
      }

      await installUpdate(update);
    } catch (error) {
      const fallbackMessage = manual ? "检查更新失败" : "自动检查更新失败";
      showToast(resolveErrorMessage(error, fallbackMessage), "error");
    } finally {
      updateChecking.value = false;
    }
  }

  function getNameTypeIcons(nameType: NameType): string[] {
    switch (nameType) {
      case "surname":
        return ["姓"];
      case "given":
        return ["名"];
      case "place":
        return ["地"];
      case "creature":
        return ["生"];
      case "monster":
        return ["怪"];
      case "gear":
        return ["装"];
      case "food":
        return ["食"];
      case "item":
        return ["物"];
      case "skill":
        return ["技"];
      case "faction":
        return ["势"];
      case "title":
        return ["衔"];
      case "nickname":
        return ["绰"];
      case "others":
        return [];
      case "both":
        return ["姓", "名"];
      default:
        if (import.meta.env.DEV) {
          console.warn("Unknown nameType icon mapping:", nameType);
        }
        return [];
    }
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

  function shouldShowGenderIcon(nameType: NameType): boolean {
    return isGenderEditableByNameType(nameType);
  }

  function formatGroupLabel(group: string): string {
    const text = group.trim();
    if (!text) {
      return "〔未分组〕";
    }
    return `〔${text}〕`;
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

  onMounted(async () => {
    await initializeSettings();
    try {
      appVersion.value = await getVersion();
    } catch {
      appVersion.value = "";
    }
    try {
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
    startupUpdateTimer = setTimeout(() => {
      void checkForUpdates(false);
    }, 1200);
  });

  onBeforeUnmount(() => {
    if (resolveUpdateConfirm) {
      resolveUpdateConfirm(false);
      resolveUpdateConfirm = null;
    }
    if (startupUpdateTimer) {
      clearTimeout(startupUpdateTimer);
      startupUpdateTimer = null;
    }
    if (wheelPageCooldownTimer) {
      clearTimeout(wheelPageCooldownTimer);
      wheelPageCooldownTimer = null;
    }
    if (unlistenEntryUpdated) {
      unlistenEntryUpdated();
      unlistenEntryUpdated = null;
    }
    if (unlistenEditorOpenRequest) {
      unlistenEditorOpenRequest();
      unlistenEditorOpenRequest = null;
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

  return {
    activeHotkey,
    appVersion,
    closeSettings,
    dictionaries,
    filters,
    formatGroupLabel,
    getGenderIconClass,
    getNameTypeIcons,
    handleEntryClick,
    handleResultWheel,
    loading,
    nextPage,
    openEditor,
    openSettings,
    pageDisplay,
    prevPage,
    projectDataDir,
    checkForUpdates,
    query,
    queryButtonLoading,
    renderItems,
    result,
    isGenderFilterEditable,
    saveSettings,
    settingsForm,
    settingsSaving,
    settingsVisible,
    updateChecking,
    updateConfirmVisible,
    updateConfirmTitle,
    updateConfirmMessage,
    updateConfirmConfirmText,
    updateConfirmCancelText,
    acceptUpdateConfirm,
    cancelUpdateConfirm,
    shouldShowGenderIcon,
    toastMessage,
    toastTone,
  };
}
