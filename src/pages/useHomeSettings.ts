import { invoke } from "@tauri-apps/api/core";
import { reactive, ref } from "vue";
import { resolveErrorMessage } from "../utils/error";

interface AppSettingsResponse {
  dictDir: string;
  hotkey: string;
  projectDataDir: string;
}

interface UseHomeSettingsOptions {
  onError: (message: string) => void;
  onAfterSave?: () => Promise<void> | void;
}

export function useHomeSettings(options: UseHomeSettingsOptions) {
  const activeHotkey = ref("Alt+Z");
  const settingsVisible = ref(false);
  const settingsSaving = ref(false);
  const projectDataDir = ref("");
  const settingsForm = reactive({
    dictDir: "",
    hotkey: "Alt+Z",
  });

  async function loadSettings(): Promise<void> {
    const settings = await invoke<AppSettingsResponse>("get_app_settings");
    projectDataDir.value = settings.projectDataDir;
    settingsForm.dictDir = settings.dictDir;
    settingsForm.hotkey = settings.hotkey;
    activeHotkey.value = settings.hotkey;
  }

  async function initializeSettings(): Promise<void> {
    try {
      await loadSettings();
    } catch (error) {
      options.onError(resolveErrorMessage(error, "读取设置失败"));
    }
  }

  async function openSettings(): Promise<void> {
    try {
      await loadSettings();
      settingsVisible.value = true;
    } catch (error) {
      options.onError(resolveErrorMessage(error, "读取设置失败"));
    }
  }

  function closeSettings(): void {
    if (settingsSaving.value) {
      return;
    }
    settingsVisible.value = false;
  }

  async function saveSettings(): Promise<void> {
    settingsSaving.value = true;
    try {
      const saved = await invoke<AppSettingsResponse>("save_app_settings", {
        request: {
          dictDir: settingsForm.dictDir.trim(),
          hotkey: settingsForm.hotkey.trim(),
        },
      });
      projectDataDir.value = saved.projectDataDir;
      settingsForm.dictDir = saved.dictDir;
      settingsForm.hotkey = saved.hotkey;
      activeHotkey.value = saved.hotkey;
      settingsVisible.value = false;
      if (options.onAfterSave) {
        await options.onAfterSave();
      }
    } catch (error) {
      options.onError(resolveErrorMessage(error, "保存设置失败"));
    } finally {
      settingsSaving.value = false;
    }
  }

  return {
    activeHotkey,
    closeSettings,
    initializeSettings,
    openSettings,
    projectDataDir,
    saveSettings,
    settingsForm,
    settingsSaving,
    settingsVisible,
  };
}
