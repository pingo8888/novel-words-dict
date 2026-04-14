<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMounted, reactive, ref } from "vue";

type NameType = "both" | "surname" | "given";
type GenderType = "both" | "male" | "female";

interface NameEntry {
  term: string;
  group: string;
  nameType: NameType;
  genderType: GenderType;
}

const errorMessage = ref("");
const saving = ref(false);
const form = reactive<NameEntry>({
  term: "",
  group: "",
  nameType: "both",
  genderType: "both",
});

function resetFormWithTerm(term: string): void {
  form.term = term.trim();
  form.group = "";
  form.nameType = "both";
  form.genderType = "both";
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
    form.group = existing.group ?? "";
    form.nameType = existing.nameType;
    form.genderType = existing.genderType;
  } else {
    resetFormWithTerm(normalizedTerm);
  }
}

async function saveEntry(): Promise<void> {
  const trimmedTerm = form.term.trim();
  if (!trimmedTerm) {
    errorMessage.value = "词条不能为空";
    return;
  }

  saving.value = true;
  errorMessage.value = "";

  try {
    await invoke("upsert_entry", {
      entry: {
        term: trimmedTerm,
        group: form.group.trim(),
        nameType: form.nameType,
        genderType: form.genderType,
      },
    });
    await invoke("close_editor_window");
  } catch (error) {
    errorMessage.value =
      typeof error === "string" ? error : "保存失败，请稍后重试";
  } finally {
    saving.value = false;
  }
}

async function closeWindow(): Promise<void> {
  await invoke("close_editor_window");
}

onMounted(async () => {
  try {
    const seedTerm = await invoke<string | null>("take_editor_seed");
    if (seedTerm && seedTerm.trim()) {
      await loadEntryByTerm(seedTerm);
    }
  } catch (error) {
    errorMessage.value =
      typeof error === "string" ? error : "初始化词条失败，请关闭后重试";
  }
});

</script>

<template>
  <main class="editor-page">
    <h1>编辑词条</h1>

    <div class="form-grid">
      <label class="field">
        <span>词条</span>
        <input
          v-model="form.term"
          type="text"
          maxlength="120"
          placeholder="请输入词条"
        />
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
        <span>姓氏类型</span>
        <select v-model="form.nameType">
          <option value="both">通用</option>
          <option value="surname">姓氏</option>
          <option value="given">名字</option>
        </select>
      </label>

      <label class="field">
        <span>性别类型</span>
        <select v-model="form.genderType">
          <option value="both">通用</option>
          <option value="male">男性</option>
          <option value="female">女性</option>
        </select>
      </label>
    </div>

    <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>

    <div class="actions">
      <button class="secondary" type="button" @click="closeWindow">取消</button>
      <button class="primary" type="button" :disabled="saving" @click="saveEntry">
        {{ saving ? "保存中..." : "确定" }}
      </button>
    </div>
  </main>
</template>

<style scoped src="./EditorPage.scoped.css"></style>
