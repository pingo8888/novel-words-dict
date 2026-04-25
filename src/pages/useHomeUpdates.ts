import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { ref } from "vue";
import type { ToastTone } from "../types/dict";
import { resolveErrorMessage } from "../utils/error";

interface UpdateConfirmOptions {
  title: string;
  message: string;
  confirmText: string;
  cancelText: string;
}

interface UseHomeUpdatesOptions {
  showToast: (message: string, tone?: ToastTone) => void;
}

export function useHomeUpdates({ showToast }: UseHomeUpdatesOptions) {
  const updateChecking = ref(false);
  const updateConfirmVisible = ref(false);
  const updateConfirmTitle = ref("");
  const updateConfirmMessage = ref("");
  const updateConfirmConfirmText = ref("确认");
  const updateConfirmCancelText = ref("取消");
  let resolveUpdateConfirm: ((confirmed: boolean) => void) | null = null;

  function buildUpdateConfirmText(update: Update): string {
    const notes = (update.body ?? "").trim();
    const notesPreview = notes.length > 400 ? `${notes.slice(0, 400)}...` : notes;
    const notesText = notesPreview ? `\n\n更新说明：\n${notesPreview}` : "";
    return `发现新版本 ${update.version}（当前 ${update.currentVersion}）。是否立即下载并安装？${notesText}`;
  }

  function resolveAndCloseUpdateConfirm(confirmed: boolean): void {
    const resolver = resolveUpdateConfirm;
    resolveUpdateConfirm = null;
    updateConfirmVisible.value = false;
    if (resolver) {
      resolver(confirmed);
    }
  }

  function acceptUpdateConfirm(): void {
    resolveAndCloseUpdateConfirm(true);
  }

  function cancelUpdateConfirm(): void {
    resolveAndCloseUpdateConfirm(false);
  }

  function requestUpdateConfirm(options: UpdateConfirmOptions): Promise<boolean> {
    if (resolveUpdateConfirm) {
      resolveUpdateConfirm(false);
      resolveUpdateConfirm = null;
    }
    updateConfirmTitle.value = options.title;
    updateConfirmMessage.value = options.message;
    updateConfirmConfirmText.value = options.confirmText;
    updateConfirmCancelText.value = options.cancelText;
    updateConfirmVisible.value = true;

    return new Promise<boolean>((resolve) => {
      resolveUpdateConfirm = resolve;
    });
  }

  async function installUpdate(update: Update): Promise<void> {
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        showToast("开始下载更新...");
        return;
      }
      if (event.event === "Finished") {
        showToast("下载完成，正在安装...");
      }
    });
    const shouldRelaunch = await requestUpdateConfirm({
      title: "更新安装完成",
      message: "是否立即重启应用？",
      confirmText: "立即重启",
      cancelText: "稍后重启",
    });
    if (!shouldRelaunch) {
      showToast("更新已安装，请稍后重启应用生效");
      return;
    }
    await relaunch();
  }

  async function checkForUpdates(manual = false): Promise<void> {
    if (!manual && import.meta.env.DEV) {
      return;
    }

    if (updateChecking.value) {
      if (manual) {
        showToast("正在检查更新，请稍候");
      }
      return;
    }

    updateChecking.value = true;
    try {
      const update = await check();
      if (!update) {
        if (manual) {
          showToast("当前已是最新版本");
        }
        return;
      }

      const shouldInstall = await requestUpdateConfirm({
        title: "发现新版本",
        message: buildUpdateConfirmText(update),
        confirmText: "下载并安装",
        cancelText: "暂不更新",
      });
      if (!shouldInstall) {
        if (manual) {
          showToast("已取消更新");
        }
        return;
      }

      await installUpdate(update);
    } catch (error) {
      const fallbackMessage = manual ? "检查更新失败" : "自动检查更新失败";
      showToast(resolveErrorMessage(error, fallbackMessage), "error");
    } finally {
      updateChecking.value = false;
    }
  }

  function cleanupUpdateConfirm(): void {
    if (resolveUpdateConfirm) {
      resolveUpdateConfirm(false);
      resolveUpdateConfirm = null;
    }
  }

  return {
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
  };
}
