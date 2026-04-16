<script setup lang="ts">
import { ref, watch } from "vue";
import { Lock, Mars, Settings, Venus, VenusAndMars } from "lucide-vue-next";
import HomeSettingsDialog from "./HomeSettingsDialog.vue";
import { useHomePage } from "./useHomePage";

const {
  activeHotkey,
  closeSettings,
  dictionaries,
  filters,
  formatGroupLabel,
  getGenderIconClass,
  getNameTypeIcons,
  handleEntryClick,
  loading,
  isGenderFilterEditable,
  nextPage,
  openEditor,
  openSettings,
  pageDisplay,
  prevPage,
  projectDataDir,
  query,
  queryButtonLoading,
  renderItems,
  result,
  saveSettings,
  settingsForm,
  settingsSaving,
  settingsVisible,
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
          总词条数：〔{{ result.totalAll }}〕，取词快捷键：〔{{ activeHotkey }}〕
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
          <span>命中词条：〔{{ result.total }}〕</span>
          <p v-if="toastMessage" class="system-tip" :class="`tone-${toastTone}`">{{ toastMessage }}</p>
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
    </div>
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
      @update:dict-dir="settingsForm.dictDir = $event"
      @update:hotkey="settingsForm.hotkey = $event"
      @close="closeSettings"
      @save="saveSettings"
    />
  </main>
</template>

<style scoped src="./HomePage.scoped.css"></style>
