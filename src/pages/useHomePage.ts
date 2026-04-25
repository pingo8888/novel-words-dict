import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { computed, onBeforeUnmount, onMounted, reactive, ref, toRef, watch } from "vue";
import {
  formatGroupLabel,
  getGenderIconClass,
  getNameTypeIcons,
  shouldShowGenderIcon,
} from "./homeDisplay";
import { useEntryActions } from "./useEntryActions";
import { useGroupSuggest } from "./useGroupSuggest";
import { useHomeSettings } from "./useHomeSettings";
import { useHomeUpdates } from "./useHomeUpdates";
import { useToast } from "../composables/useToast";
import type {
  GenderTypeFilter,
  GenreTypeFilter,
  NameTypeFilter,
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
  const appVersion = ref("");
  const { showToast, toastMessage, toastTone } = useToast();
  const {
    acceptUpdateConfirm,
    cancelUpdateConfirm,
    checkForUpdates,
    cleanupUpdateConfirm,
    updateChecking,
    updateConfirmCancelText,
    updateConfirmConfirmText,
    updateConfirmMessage,
    updateConfirmTitle,
    updateConfirmVisible,
  } = useHomeUpdates({ showToast });
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
  let wheelPageLocked = false;
  let wheelDeltaAccumulator = 0;

  async function query(resetPage = false): Promise<void> {
    if (loading.value) {
      return;
    }

    if (resetPage) {
      closeGroupSuggest();
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

  const {
    clearKeyword,
    closeGroupSuggest,
    groupSuggestPage,
    groupSuggestPageCount,
    groupSuggestPageItems,
    groupSuggestSelectedIndex,
    groupSuggestVisible,
    handleKeywordBlur,
    handleKeywordClick,
    handleKeywordInput,
    handleKeywordKeydown,
    handleKeywordKeyup,
    keywordInputRef,
    nextGroupSuggestPage,
    prevGroupSuggestPage,
    selectGroupSuggestion,
  } = useGroupSuggest({
    filters,
    settingsVisible,
    updateConfirmVisible,
    showToast,
    query,
  });
  const { createEditorWindow, handleEntryClick, openEditor } = useEntryActions({
    searchEngine: toRef(settingsForm, "searchEngine"),
    showToast,
  });

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

  function handlePageShortcut(event: KeyboardEvent): void {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }
    if (settingsVisible.value || updateConfirmVisible.value) {
      return;
    }

    event.preventDefault();
    if (event.key === "ArrowLeft") {
      void prevPage();
      return;
    }
    void nextPage();
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

  function notifyOpenDirFailed(message: string): void {
    showToast(message || "打开目录失败", "error");
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
    window.addEventListener("keydown", handlePageShortcut);
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
    window.removeEventListener("keydown", handlePageShortcut);
    cleanupUpdateConfirm();
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
    clearKeyword,
    closeSettings,
    dictionaries,
    filters,
    formatGroupLabel,
    getGenderIconClass,
    getNameTypeIcons,
    groupSuggestPage,
    groupSuggestPageCount,
    groupSuggestPageItems,
    groupSuggestSelectedIndex,
    groupSuggestVisible,
    handleKeywordClick,
    handleKeywordBlur,
    handleKeywordInput,
    handleKeywordKeydown,
    handleKeywordKeyup,
    handleEntryClick,
    handleResultWheel,
    keywordInputRef,
    loading,
    nextPage,
    nextGroupSuggestPage,
    notifyOpenDirFailed,
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
    prevGroupSuggestPage,
    saveSettings,
    selectGroupSuggestion,
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
