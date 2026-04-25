<script setup lang="ts">
import { ref, watch } from "vue";
import { Lock, Settings } from "lucide-vue-next";
import GroupSuggestPanel from "./GroupSuggestPanel.vue";
import HomeSettingsDialog from "./HomeSettingsDialog.vue";
import UpdateConfirmDialog from "./UpdateConfirmDialog.vue";
import { useHomePage } from "./useHomePage";

const {
  activeHotkey,
  acceptUpdateConfirm,
  appVersion,
  cancelUpdateConfirm,
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
  handleKeywordBlur,
  handleKeywordClick,
  handleKeywordInput,
  handleKeywordKeydown,
  handleKeywordKeyup,
  handleEntryClick,
  handleResultWheel,
  keywordInputRef,
  loading,
  isGenderFilterEditable,
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
  prevGroupSuggestPage,
  saveSettings,
  selectGroupSuggestion,
  settingsForm,
  settingsSaving,
  settingsVisible,
  updateChecking,
  updateConfirmCancelText,
  updateConfirmConfirmText,
  updateConfirmMessage,
  updateConfirmTitle,
  updateConfirmVisible,
  shouldShowGenderIcon,
  toastMessage,
  toastTone,
} = useHomePage();

const pageContentRef = ref<HTMLElement | null>(null);
watch(settingsVisible, (visible) => {
  if (!pageContentRef.value) {
    return;
  }
  pageContentRef.value.toggleAttribute("inert", visible);
});
</script>

<template>
  <main class="home-page">
    <div ref="pageContentRef" class="home-content">
      <div class="top-row">
        <p class="description-inline">
          总词条数：[{{ result.totalAll }}]
        </p>
        <div class="top-row-actions">
          <button class="ui-icon-btn settings-icon-btn" type="button" title="设置" @click="openSettings">
            <Settings :size="16" :stroke-width="2" />
          </button>
        </div>
      </div>

      <section class="filters">
        <label class="ui-field field">
          <span class="ui-field-label">词库</span>
          <select v-model="filters.dictId" class="ui-control">
            <option v-for="item in dictionaries" :key="item.id" :value="item.id">
              {{ item.name }}
            </option>
          </select>
        </label>

        <label class="ui-field field">
          <span class="ui-field-label">风格</span>
          <select v-model="filters.genreType" class="ui-control">
            <option value="all">所有</option>
            <option value="china">中国</option>
            <option value="japan">日本</option>
            <option value="west">西方</option>
          </select>
        </label>

        <label class="ui-field field">
          <span class="ui-field-label">名词类型</span>
          <select v-model="filters.nameType" class="ui-control">
            <option value="all">所有</option>
            <option value="surname">姓氏</option>
            <option value="given">名字</option>
            <option value="place">地名</option>
            <option value="myth">神话</option>
            <option value="people">人物</option>
            <option value="creature">生物</option>
            <option value="monster">怪物</option>
            <option value="gear">装备</option>
            <option value="food">食物</option>
            <option value="item">物品</option>
            <option value="skill">技能</option>
            <option value="faction">势力</option>
            <option value="title">头衔</option>
            <option value="nickname">绰号</option>
            <option value="book">书籍</option>
            <option value="others">其他</option>
          </select>
        </label>

        <label class="ui-field field">
          <span class="ui-field-label">性别</span>
          <select v-model="filters.genderType" class="ui-control" :disabled="!isGenderFilterEditable">
            <option value="all">所有</option>
            <option value="male">男性</option>
            <option value="female">女性</option>
          </select>
        </label>

        <label class="ui-field field keyword">
          <span class="ui-field-label">关键字</span>
          <div class="keyword-input-wrap">
            <input
              class="ui-control"
              ref="keywordInputRef"
              v-model="filters.keyword"
              type="text"
              maxlength="120"
              placeholder="多个关键字用空格分隔，自定/内置可筛选词库，@前缀可匹配分组"
              @input="handleKeywordInput"
              @click="handleKeywordClick"
              @blur="handleKeywordBlur"
              @keyup="handleKeywordKeyup"
              @keydown="handleKeywordKeydown"
            />
            <button
              v-if="filters.keyword.length > 0"
              type="button"
              class="keyword-clear-btn"
              aria-label="清空关键字"
              @click="clearKeyword"
            >
              ×
            </button>
            <GroupSuggestPanel
              v-if="groupSuggestVisible"
              :items="groupSuggestPageItems"
              :page="groupSuggestPage"
              :page-count="groupSuggestPageCount"
              :selected-index="groupSuggestSelectedIndex"
              @select="selectGroupSuggestion"
              @prev-page="prevGroupSuggestPage"
              @next-page="nextGroupSuggestPage"
            />
          </div>
        </label>

        <button class="ui-btn ui-btn-primary query-btn" type="button" :disabled="queryButtonLoading" @click="query(true)">
          {{ queryButtonLoading ? "查询中..." : "查询" }}
        </button>
      </section>

      <section class="result-panel" @wheel="handleResultWheel">
        <div class="result-summary">
          <span>命中词条：[{{ result.total }}]</span>
        </div>

        <div class="entry-grid">
          <button
            v-for="entry in renderItems"
            :key="`${entry.dictId}-${entry.term}-${entry.group}-${entry.nameType}-${entry.genderType}-${entry.genre}`"
            class="entry-item"
            type="button"
            @click="handleEntryClick($event, entry)"
            @contextmenu.prevent="openEditor(entry)"
          >
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
                v-if="shouldShowGenderIcon(entry.nameType)"
                class="entry-icon"
                :class="getGenderIconClass(entry.genderType)"
              >
                {{
                  entry.genderType === "male"
                    ? "♂"
                    : entry.genderType === "female"
                      ? "♀"
                      : "⚥"
                }}
              </span>
            </div>
            <div class="entry-main">
              <span class="term">{{ entry.term }}</span>
              <span class="group">{{ formatGroupLabel(entry.group) }}</span>
            </div>
          </button>
        </div>

        <div class="pagination">
          <button class="ui-btn ui-btn-primary" type="button" :disabled="loading || filters.page <= 1" @click="prevPage">
            上一页
          </button>
          <span>{{ pageDisplay }}</span>
          <button
            class="ui-btn ui-btn-primary"
            type="button"
            :disabled="loading || filters.page >= result.pageCount"
            @click="nextPage"
          >
            下一页
          </button>
        </div>
      </section>
    </div>
    <div class="action-hints">
      <p class="action-hints-left">复制：[左键]  编辑：[右键]  查词：[Ctrl+左键]  添加：[{{ activeHotkey }}]</p>
      <p class="action-hints-right">版本：{{ appVersion || "-" }}</p>
    </div>
    <p v-if="toastMessage" class="ui-system-tip system-tip floating-system-tip" :class="`tone-${toastTone}`">
      {{ toastMessage }}
    </p>
    <p
      v-if="toastMessage"
      class="sr-only"
      :role="toastTone === 'error' ? 'alert' : 'status'"
      :aria-live="toastTone === 'error' ? 'assertive' : 'polite'"
    >
      {{ toastMessage }}
    </p>

    <HomeSettingsDialog
      :visible="settingsVisible"
      :settings-saving="settingsSaving"
      :project-data-dir="projectDataDir"
      :dict-dir="settingsForm.dictDir"
      :hotkey="settingsForm.hotkey"
      :search-engine="settingsForm.searchEngine"
      :update-checking="updateChecking"
      @update:hotkey="settingsForm.hotkey = $event"
      @update:search-engine="settingsForm.searchEngine = $event"
      @open-dir-failed="notifyOpenDirFailed"
      @check-update="checkForUpdates(true)"
      @close="closeSettings"
      @save="saveSettings"
    />
    <UpdateConfirmDialog
      :visible="updateConfirmVisible"
      :title="updateConfirmTitle"
      :message="updateConfirmMessage"
      :confirm-text="updateConfirmConfirmText"
      :cancel-text="updateConfirmCancelText"
      @confirm="acceptUpdateConfirm"
      @cancel="cancelUpdateConfirm"
    />
  </main>
</template>

<style scoped src="./HomePage.scoped.css"></style>
