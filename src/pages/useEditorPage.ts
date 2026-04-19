import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useToast } from "../composables/useToast";
import type { NameEntry } from "../types/dict";
import { resolveErrorMessage } from "../utils/error";
import { isGenderEditableByNameType } from "../utils/nameType";

export function useEditorPage() {
  const saving = ref(false);
  const deleting = ref(false);
  const deleteConfirmVisible = ref(false);
  const { showToast, toastMessage, toastTone } = useToast();
  const editorTitle = ref("添加词条");
  const editingTerm = ref("");
  const bundledExistsDictName = ref("");
  const form = reactive<NameEntry>({
    term: "",
    genre: "west",
    group: "",
    nameType: "surname",
    genderType: "both",
  });
  const isGenderTypeEditable = computed(() => isGenderEditableByNameType(form.nameType));
  let unlistenSeedUpdated: (() => void) | null = null;
  let bundledLookupSeq = 0;
  let bundledLookupTimer: ReturnType<typeof setTimeout> | null = null;

  type TakeEditorSeedResult =
    | { ok: true; seed: string | null }
    | { ok: false; error: string };

  async function setEditorWindowTitle(title: string): Promise<void> {
    try {
      await invoke("set_editor_window_title", { title });
    } catch {
      // Ignore window title sync failures.
    }
  }

  function applyEditorTitle(editing: boolean): void {
    const nextTitle = editing ? "编辑词条" : "添加词条";
    if (editorTitle.value !== nextTitle) {
      editorTitle.value = nextTitle;
    }
    void setEditorWindowTitle(nextTitle);
  }

  async function takeEditorSeed(): Promise<TakeEditorSeedResult> {
    try {
      const seed = await invoke<string | null>("take_editor_seed");
      return { ok: true, seed };
    } catch (error) {
      return {
        ok: false,
        error: resolveErrorMessage(error, "读取编辑词条失败"),
      };
    }
  }

  function resetFormWithTerm(term: string): void {
    form.term = term.trim();
    form.genre = "west";
    form.group = "";
    form.nameType = "surname";
    form.genderType = "both";
    editingTerm.value = "";
    applyEditorTitle(false);
  }

  function applyEntryToForm(entry: NameEntry, preserveTerm: boolean): void {
    if (!preserveTerm) {
      form.term = entry.term;
    }
    form.genre = entry.genre;
    form.group = entry.group ?? "";
    form.nameType = entry.nameType === "both" ? "surname" : entry.nameType;
    form.genderType = isGenderEditableByNameType(form.nameType) ? entry.genderType : "both";
  }

  async function refreshBundledLookup(
    term: string,
    options: { autofill: boolean; preserveTerm: boolean },
  ): Promise<NameEntry | null> {
    const normalizedTerm = term.trim();
    if (!normalizedTerm || editingTerm.value) {
      bundledExistsDictName.value = "";
      return null;
    }

    const seq = ++bundledLookupSeq;
    try {
      const [dictName, bundledEntry] = await Promise.all([
        invoke<string | null>("get_bundled_entry_dict_name", {
          term: normalizedTerm,
        }),
        invoke<NameEntry | null>("get_bundled_entry", {
          term: normalizedTerm,
        }),
      ]);
      if (seq !== bundledLookupSeq) {
        return bundledEntry;
      }
      bundledExistsDictName.value = dictName ?? "";
      if (
        options.autofill &&
        bundledEntry &&
        !editingTerm.value &&
        form.term.trim() === normalizedTerm
      ) {
        applyEntryToForm(bundledEntry, options.preserveTerm);
      }
      return bundledEntry;
    } catch {
      if (seq !== bundledLookupSeq) {
        return null;
      }
      bundledExistsDictName.value = "";
      return null;
    }
  }

  async function loadEntryByTerm(term: string): Promise<void> {
    const normalizedTerm = term.trim();
    if (!normalizedTerm) {
      resetFormWithTerm("");
      bundledExistsDictName.value = "";
      return;
    }

    const existing = await invoke<NameEntry | null>("get_entry", { term: normalizedTerm });
    if (existing) {
      applyEntryToForm(existing, false);
      editingTerm.value = existing.term;
      applyEditorTitle(true);
      bundledExistsDictName.value = "";
      bundledLookupSeq += 1;
    } else {
      resetFormWithTerm(normalizedTerm);
      await refreshBundledLookup(normalizedTerm, { autofill: true, preserveTerm: false });
    }
  }

  async function refreshEditorFromSeed(): Promise<void> {
    const nextSeedResult = await takeEditorSeed();
    if (!nextSeedResult.ok) {
      showToast(nextSeedResult.error, "error");
      return;
    }
    await loadEntryByTerm(nextSeedResult.seed ?? "");
  }

  async function saveEntry(): Promise<void> {
    const trimmedTerm = form.term.trim();
    if (!trimmedTerm) {
      showToast("词条不能为空", "error");
      return;
    }

    saving.value = true;

    try {
      await invoke("upsert_entry", {
        entry: {
          term: trimmedTerm,
          genre: form.genre,
          group: form.group.trim(),
          nameType: form.nameType,
          genderType: isGenderTypeEditable.value ? form.genderType : "both",
        },
        originalTerm: editingTerm.value || null,
      });
      await invoke("close_editor_window");
    } catch (error) {
      showToast(resolveErrorMessage(error, "保存失败，请稍后重试"), "error");
    } finally {
      saving.value = false;
    }
  }

  async function closeWindow(): Promise<void> {
    await invoke("close_editor_window");
  }

  function requestDeleteEntry(): void {
    if (!editingTerm.value || deleting.value) {
      return;
    }
    deleteConfirmVisible.value = true;
  }

  function closeDeleteConfirm(): void {
    if (deleting.value) {
      return;
    }
    deleteConfirmVisible.value = false;
  }

  async function deleteEntry(): Promise<void> {
    if (!editingTerm.value || deleting.value) {
      return;
    }

    deleting.value = true;

    try {
      await invoke("delete_entry", { term: editingTerm.value });
      await invoke("close_editor_window");
    } catch (error) {
      showToast(resolveErrorMessage(error, "删除失败，请稍后重试"), "error");
    } finally {
      deleting.value = false;
      deleteConfirmVisible.value = false;
    }
  }

  onMounted(async () => {
    try {
      await refreshEditorFromSeed();
      unlistenSeedUpdated = await listen("editor-seed-updated", () => {
        void refreshEditorFromSeed().catch((error) => {
          showToast(resolveErrorMessage(error, "刷新词条失败，请稍后重试"), "error");
        });
      });
    } catch (error) {
      showToast(resolveErrorMessage(error, "初始化词条失败，请关闭后重试"), "error");
    }
  });

  onBeforeUnmount(() => {
    if (bundledLookupTimer) {
      clearTimeout(bundledLookupTimer);
      bundledLookupTimer = null;
    }
    if (unlistenSeedUpdated) {
      unlistenSeedUpdated();
      unlistenSeedUpdated = null;
    }
  });

  watch(
    () => form.nameType,
    () => {
      if (!isGenderTypeEditable.value && form.genderType !== "both") {
        form.genderType = "both";
      }
    },
    { immediate: true },
  );

  watch(
    () => form.term,
    (value) => {
      if (bundledLookupTimer) {
        clearTimeout(bundledLookupTimer);
      }
      bundledLookupTimer = setTimeout(() => {
        void refreshBundledLookup(value, { autofill: true, preserveTerm: true });
      }, 220);
    },
  );

  return {
    bundledExistsDictName,
    closeDeleteConfirm,
    closeWindow,
    deleteConfirmVisible,
    deleteEntry,
    deleting,
    editorTitle,
    editingTerm,
    form,
    isGenderTypeEditable,
    requestDeleteEntry,
    saveEntry,
    saving,
    toastMessage,
    toastTone,
  };
}
