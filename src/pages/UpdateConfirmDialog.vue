<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { focusFirstElement, trapTabKey } from "../utils/a11y";

const props = defineProps<{
  visible: boolean;
  title: string;
  message: string;
  confirmText: string;
  cancelText: string;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const dialogRef = ref<HTMLElement | null>(null);
let restoreFocusTarget: HTMLElement | null = null;
let backdropPointerDown = false;

function onMaskPointerDown(event: PointerEvent): void {
  backdropPointerDown = event.target === event.currentTarget;
}

function onMaskClick(event: MouseEvent): void {
  if (event.target !== event.currentTarget) {
    backdropPointerDown = false;
    return;
  }
  if (backdropPointerDown) {
    emit("cancel");
  }
  backdropPointerDown = false;
}

function onDialogKeydown(event: KeyboardEvent): void {
  if (!dialogRef.value) {
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    emit("cancel");
    return;
  }
  trapTabKey(event, dialogRef.value);
}

watch(
  () => props.visible,
  async (visible) => {
    if (visible) {
      restoreFocusTarget = document.activeElement as HTMLElement | null;
      await nextTick();
      if (dialogRef.value) {
        focusFirstElement(dialogRef.value);
      }
      return;
    }
    if (restoreFocusTarget && document.contains(restoreFocusTarget)) {
      restoreFocusTarget.focus();
    }
    restoreFocusTarget = null;
  },
);
</script>

<template>
  <div
    v-if="visible"
    class="update-confirm-mask"
    @pointerdown="onMaskPointerDown"
    @click.self="onMaskClick"
  >
    <section
      ref="dialogRef"
      class="update-confirm-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-confirm-title"
      aria-describedby="update-confirm-desc"
      tabindex="-1"
      @keydown="onDialogKeydown"
    >
      <h2 id="update-confirm-title">{{ title }}</h2>
      <p id="update-confirm-desc">{{ message }}</p>
      <div class="update-confirm-actions">
        <button type="button" class="secondary" @click="emit('cancel')">
          {{ cancelText }}
        </button>
        <button type="button" class="primary" @click="emit('confirm')">
          {{ confirmText }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped src="./UpdateConfirmDialog.scoped.css"></style>
