import { onBeforeUnmount, ref } from "vue";
import type { ToastTone } from "../types/dict";

export function useToast(durationMs = 1800) {
  const toastMessage = ref("");
  const toastTone = ref<ToastTone>("info");
  let toastTimer: number | null = null;

  function showToast(message: string, tone: ToastTone = "info"): void {
    toastMessage.value = message;
    toastTone.value = tone;
    if (toastTimer !== null) {
      window.clearTimeout(toastTimer);
    }
    toastTimer = window.setTimeout(() => {
      toastMessage.value = "";
      toastTimer = null;
    }, durationMs);
  }

  onBeforeUnmount(() => {
    if (toastTimer !== null) {
      window.clearTimeout(toastTimer);
      toastTimer = null;
    }
  });

  return {
    showToast,
    toastMessage,
    toastTone,
  };
}
