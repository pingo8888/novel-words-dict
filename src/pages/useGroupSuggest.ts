import { invoke } from "@tauri-apps/api/core";
import { computed, nextTick, reactive, ref, watch, type Ref } from "vue";
import type { GenderTypeFilter, GenreTypeFilter, NameTypeFilter, ToastTone } from "../types/dict";
import { resolveErrorMessage } from "../utils/error";

interface GroupSuggestionFilters {
  dictId: string;
  genreType: GenreTypeFilter;
  nameType: NameTypeFilter;
  genderType: GenderTypeFilter;
  keyword: string;
}

interface GroupSuggestionRequest {
  dictId: string;
  genreType: GenreTypeFilter;
  nameType: NameTypeFilter;
  genderType: GenderTypeFilter;
  keyword: string;
}

interface UseGroupSuggestOptions {
  filters: GroupSuggestionFilters;
  settingsVisible: Ref<boolean>;
  updateConfirmVisible: Ref<boolean>;
  showToast: (message: string, tone?: ToastTone) => void;
  query: (resetPage?: boolean) => Promise<void>;
}

const GROUP_SUGGEST_PAGE_SIZE = 6;

export function useGroupSuggest({
  filters,
  settingsVisible,
  updateConfirmVisible,
  showToast,
  query,
}: UseGroupSuggestOptions) {
  const keywordInputRef = ref<HTMLInputElement | null>(null);
  const groupSuggest = reactive({
    visible: false,
    items: [] as string[],
    page: 1,
    selectedIndex: 0,
    segmentStart: 0,
    segmentEnd: 0,
    query: "",
  });

  const groupSuggestPageCount = computed(() =>
    Math.max(1, Math.ceil(groupSuggest.items.length / GROUP_SUGGEST_PAGE_SIZE)),
  );
  const groupSuggestPageItems = computed(() => {
    const start = (groupSuggest.page - 1) * GROUP_SUGGEST_PAGE_SIZE;
    return groupSuggest.items.slice(start, start + GROUP_SUGGEST_PAGE_SIZE);
  });
  const groupSuggestVisible = computed(() => groupSuggest.visible);
  const groupSuggestPage = computed(() => groupSuggest.page);
  const groupSuggestSelectedIndex = computed(() => groupSuggest.selectedIndex);

  let groupSuggestSeq = 0;

  function findGroupSegment(input: HTMLInputElement): {
    start: number;
    end: number;
    query: string;
  } | null {
    const value = input.value;
    const caret = input.selectionStart ?? value.length;
    let start = caret;
    while (start > 0 && !/\s/.test(value[start - 1])) {
      start -= 1;
    }
    let end = caret;
    while (end < value.length && !/\s/.test(value[end])) {
      end += 1;
    }
    if (value[start] !== "@" || caret <= start) {
      return null;
    }
    return {
      start,
      end,
      query: value.slice(start + 1, caret),
    };
  }

  function closeGroupSuggest(): void {
    groupSuggest.visible = false;
    groupSuggest.items = [];
    groupSuggest.page = 1;
    groupSuggest.selectedIndex = 0;
    groupSuggest.query = "";
    groupSuggestSeq += 1;
  }

  function clampGroupSuggestSelection(): void {
    if (groupSuggest.page > groupSuggestPageCount.value) {
      groupSuggest.page = groupSuggestPageCount.value;
    }
    if (groupSuggest.page < 1) {
      groupSuggest.page = 1;
    }
    const pageLen = groupSuggestPageItems.value.length;
    if (pageLen === 0) {
      groupSuggest.selectedIndex = 0;
      return;
    }
    groupSuggest.selectedIndex = Math.max(0, Math.min(groupSuggest.selectedIndex, pageLen - 1));
  }

  async function refreshGroupSuggestions(
    input: HTMLInputElement | null = keywordInputRef.value,
  ): Promise<void> {
    if (!input || settingsVisible.value || updateConfirmVisible.value) {
      closeGroupSuggest();
      return;
    }

    const segment = findGroupSegment(input);
    if (!segment) {
      closeGroupSuggest();
      return;
    }

    const previousQuery = groupSuggest.query;
    groupSuggest.visible = true;
    groupSuggest.segmentStart = segment.start;
    groupSuggest.segmentEnd = segment.end;
    groupSuggest.query = segment.query;
    if (previousQuery !== segment.query) {
      groupSuggest.page = 1;
      groupSuggest.selectedIndex = 0;
    }

    const seq = ++groupSuggestSeq;
    try {
      const request: GroupSuggestionRequest = {
        dictId: filters.dictId,
        genreType: filters.genreType,
        nameType: filters.nameType,
        genderType: filters.genderType,
        keyword: segment.query,
      };
      const items = await invoke<string[]>("query_group_suggestions", { request });
      if (seq !== groupSuggestSeq) {
        return;
      }
      groupSuggest.items = items;
      clampGroupSuggestSelection();
    } catch (error) {
      if (seq !== groupSuggestSeq) {
        return;
      }
      closeGroupSuggest();
      showToast(resolveErrorMessage(error, "读取分组失败，请稍后重试"), "error");
    }
  }

  function prevGroupSuggestPage(): void {
    if (!groupSuggest.visible || groupSuggest.page <= 1) {
      return;
    }
    groupSuggest.page -= 1;
    groupSuggest.selectedIndex = 0;
    clampGroupSuggestSelection();
  }

  function nextGroupSuggestPage(): void {
    if (!groupSuggest.visible || groupSuggest.page >= groupSuggestPageCount.value) {
      return;
    }
    groupSuggest.page += 1;
    groupSuggest.selectedIndex = 0;
    clampGroupSuggestSelection();
  }

  function moveGroupSuggestSelection(delta: number): void {
    if (!groupSuggest.visible || groupSuggestPageItems.value.length === 0) {
      return;
    }
    const maxIndex = groupSuggestPageItems.value.length - 1;
    groupSuggest.selectedIndex = Math.max(0, Math.min(groupSuggest.selectedIndex + delta, maxIndex));
  }

  async function selectGroupSuggestion(value: string): Promise<void> {
    const input = keywordInputRef.value;
    if (!input || !groupSuggest.visible) {
      return;
    }
    const before = filters.keyword.slice(0, groupSuggest.segmentStart);
    const after = filters.keyword.slice(groupSuggest.segmentEnd);
    const replacement = `@${value}`;
    const caret = before.length + replacement.length;
    filters.keyword = `${before}${replacement}${after}`;
    closeGroupSuggest();
    await nextTick();
    input.focus();
    input.setSelectionRange(caret, caret);
  }

  function handleKeywordInput(event: Event): void {
    void refreshGroupSuggestions(event.target as HTMLInputElement);
  }

  function handleKeywordClick(event: MouseEvent): void {
    void refreshGroupSuggestions(event.target as HTMLInputElement);
  }

  function handleKeywordKeyup(event: KeyboardEvent): void {
    if (
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      event.key === "Alt" ||
      event.key === "Control" ||
      event.key === "Meta" ||
      event.key === "Shift"
    ) {
      return;
    }
    if (
      event.key === "ArrowUp" ||
      event.key === "ArrowDown" ||
      event.key === "ArrowLeft" ||
      event.key === "ArrowRight"
    ) {
      if (!groupSuggest.visible) {
        void refreshGroupSuggestions(event.target as HTMLInputElement);
      }
      return;
    }
    if (event.key === "Enter" || event.key === "Escape") {
      return;
    }
    void refreshGroupSuggestions(event.target as HTMLInputElement);
  }

  function handleKeywordBlur(): void {
    closeGroupSuggest();
  }

  function handleKeywordKeydown(event: KeyboardEvent): void {
    if (!groupSuggest.visible) {
      if (event.key === "Enter") {
        void query(true);
      }
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeGroupSuggest();
      return;
    }
    if (event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      return;
    }
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      prevGroupSuggestPage();
      return;
    }
    if (event.key === "ArrowRight") {
      event.preventDefault();
      nextGroupSuggestPage();
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveGroupSuggestSelection(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveGroupSuggestSelection(1);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const selected = groupSuggestPageItems.value[groupSuggest.selectedIndex];
      if (selected) {
        void selectGroupSuggestion(selected);
      }
    }
  }

  function clearKeyword(): void {
    filters.keyword = "";
    closeGroupSuggest();
  }

  watch(
    () => [filters.dictId, filters.genreType, filters.nameType, filters.genderType],
    () => {
      if (groupSuggest.visible) {
        void refreshGroupSuggestions();
      }
    },
  );

  watch(settingsVisible, (visible) => {
    if (visible) {
      closeGroupSuggest();
    }
  });

  watch(updateConfirmVisible, (visible) => {
    if (visible) {
      closeGroupSuggest();
    }
  });

  return {
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
  };
}
