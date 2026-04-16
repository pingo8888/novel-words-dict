<script setup lang="ts">
defineProps<{
  visible: boolean;
  settingsSaving: boolean;
  projectDataDir: string;
  settingsForm: {
    dictDir: string;
    hotkey: string;
  };
}>();

const emit = defineEmits<{
  close: [];
  save: [];
}>();
</script>

<template>
  <div v-if="visible" class="settings-mask" @click.self="emit('close')">
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
        <button type="button" class="secondary" :disabled="settingsSaving" @click="emit('close')">
          取消
        </button>
        <button type="button" class="primary" :disabled="settingsSaving" @click="emit('save')">
          {{ settingsSaving ? "保存中..." : "保存" }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped src="./HomeSettingsDialog.scoped.css"></style>
