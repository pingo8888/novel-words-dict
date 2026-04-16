<script setup lang="ts">
import { useEditorPage } from "./useEditorPage";

const {
  bundledExistsDictName,
  closeDeleteConfirm,
  closeWindow,
  deleteConfirmVisible,
  deleteEntry,
  deleting,
  editorModeLabel,
  editingTerm,
  form,
  isGenderTypeEditable,
  requestDeleteEntry,
  saveEntry,
  saving,
  toastMessage,
  toastTone,
} = useEditorPage();
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
          <option value="creature">生物</option>
          <option value="gear">装备</option>
          <option value="item">物品</option>
          <option value="skill">技能</option>
          <option value="faction">势力</option>
          <option value="nickname">绰号</option>
          <option value="others">其他</option>
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
        <p v-if="!editingTerm && bundledExistsDictName" class="bundled-exists-tip">
          “{{ bundledExistsDictName }}”已有此词条
        </p>
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
