<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";

type NameType = "both" | "surname" | "given" | "place";
type GenderType = "both" | "male" | "female";
type GenreType = "east" | "west";
type ToastTone = "info" | "error";

interface NameEntry {
  term: string;
  genre: GenreType;
  group: string;
  nameType: NameType;
  genderType: GenderType;
}

const saving = ref(false);
const deleting = ref(false);
const deleteConfirmVisible = ref(false);
const toastMessage = ref("");
const toastTone = ref<ToastTone>("info");
const editorModeLabel = ref("[添加]");
const editingTerm = ref("");
const form = reactive<NameEntry>({
  term: "",
  genre: "west",
  group: "",
  nameType: "surname",
  genderType: "both",
});
const isGenderTypeEditable = computed(
  () => form.nameType === "surname" || form.nameType === "given",
);
let toastTimer: number | null = null;

function resolveErrorMessage(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : fallback;
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

function resetFormWithTerm(term: string): void {
  form.term = term.trim();
  form.genre = "west";
  form.group = "";
  form.nameType = "surname";
  form.genderType = "both";
  editingTerm.value = "";
  editorModeLabel.value = "[添加]";
}

async function loadEntryByTerm(term: string): Promise<void> {
  const normalizedTerm = term.trim();
  if (!normalizedTerm) {
    resetFormWithTerm("");
    return;
  }

  const existing = await invoke<NameEntry | null>("get_entry", { term: normalizedTerm });
  if (existing) {
    form.term = existing.term;
    form.genre = existing.genre;
    form.group = existing.group ?? "";
    form.nameType = existing.nameType === "both" ? "surname" : existing.nameType;
    form.genderType =
      form.nameType === "surname" || form.nameType === "given"
        ? existing.genderType
        : "both";
    editingTerm.value = existing.term;
    editorModeLabel.value = "[修改]";
  } else {
    resetFormWithTerm(normalizedTerm);
  }
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
    const seedTerm = await invoke<string | null>("take_editor_seed");
    if (seedTerm && seedTerm.trim()) {
      await loadEntryByTerm(seedTerm);
    }
  } catch (error) {
    showToast(resolveErrorMessage(error, "初始化词条失败，请关闭后重试"), "error");
  }
});

onBeforeUnmount(() => {
  if (toastTimer !== null) {
    window.clearTimeout(toastTimer);
    toastTimer = null;
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

</script>

<template>
  <main class="editor-page">
    <h1>编辑词条 {{ editorModeLabel }}</h1>

    <div class="form-grid">
      <label class="field full">
        <span>词条</span>
        <input
          v-model="form.term"
          type="text"
          maxlength="120"
          placeholder="请输入词条"
        />
      </label>

      <label class="field">
        <span>风格</span>
        <select v-model="form.genre">
          <option value="east">东方</option>
          <option value="west">西方</option>
        </select>
      </label>

      <label class="field">
        <span>分组</span>
        <input
          v-model="form.group"
          type="text"
          maxlength="120"
          placeholder="留空则显示〔未分组〕"
        />
      </label>

      <label class="field">
        <span>名词类型</span>
        <select v-model="form.nameType">
          <option value="surname">姓氏</option>
          <option value="given">名字</option>
          <option value="place">地名</option>
        </select>
      </label>

      <label class="field">
        <span>性别类型</span>
        <select v-model="form.genderType" :disabled="!isGenderTypeEditable">
          <option value="both">通用</option>
          <option value="male">男性</option>
          <option value="female">女性</option>
        </select>
      </label>
    </div>

    <div class="actions">
      <div class="actions-left">
        <button
          v-if="editingTerm"
          class="danger"
          type="button"
          :disabled="deleting || saving"
          @click="requestDeleteEntry"
        >
          {{ deleting ? "删除中..." : "删除" }}
        </button>
      </div>
      <div class="actions-right">
        <button class="secondary" type="button" :disabled="deleting" @click="closeWindow">取消</button>
        <button class="primary" type="button" :disabled="saving || deleting" @click="saveEntry">
          {{ saving ? "保存中..." : "确定" }}
        </button>
      </div>
    </div>

    <div
      v-if="deleteConfirmVisible"
      class="confirm-mask"
      @click.self="closeDeleteConfirm"
    >
      <section class="confirm-dialog">
        <h2>确认删除</h2>
        <p>确定删除词条：{{ editingTerm }}？</p>
        <div class="confirm-actions">
          <button type="button" class="secondary" :disabled="deleting" @click="closeDeleteConfirm">
            取消
          </button>
          <button type="button" class="danger" :disabled="deleting" @click="deleteEntry">
            {{ deleting ? "删除中..." : "确认删除" }}
          </button>
        </div>
      </section>
    </div>

    <p v-if="toastMessage" class="system-tip" :class="`tone-${toastTone}`">{{ toastMessage }}</p>
  </main>
</template>

<style scoped src="./EditorPage.scoped.css"></style>
