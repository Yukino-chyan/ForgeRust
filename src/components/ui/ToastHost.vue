<script setup lang="ts">
import Icon from "./Icon.vue";
import { useToast, type ToastKind } from "../../composables/useToast";

const { toasts, dismiss } = useToast();

function iconFor(kind: ToastKind) {
  if (kind === "success") return "CheckCircle2";
  if (kind === "error") return "CircleAlert";
  if (kind === "warning") return "TriangleAlert";
  return "Info";
}
</script>

<template>
  <div class="toast-host" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="toast in toasts" :key="toast.id" :class="['toast-card', toast.kind]">
        <Icon :name="iconFor(toast.kind)" :size="16" />
        <div class="toast-copy">
          <strong>{{ toast.title }}</strong>
          <span v-if="toast.message">{{ toast.message }}</span>
        </div>
        <button class="toast-close" title="关闭" @click="dismiss(toast.id)">
          <Icon name="X" :size="14" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: 20px;
  top: 20px;
  z-index: 300;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  width: min(360px, calc(100vw - 32px));
  pointer-events: none;
}
.toast-card {
  pointer-events: auto;
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: var(--sp-2);
  align-items: start;
  padding: var(--sp-3);
  background: var(--surface);
  border: 1px solid var(--border);
  border-left-width: 3px;
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  color: var(--text);
}
.toast-card.success { border-left-color: var(--success); }
.toast-card.error { border-left-color: var(--danger); }
.toast-card.warning { border-left-color: var(--warning); }
.toast-card.info { border-left-color: var(--accent); }
.toast-copy { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.toast-copy strong { font-size: var(--fs-13); font-weight: var(--fw-semibold); }
.toast-copy span { font-size: var(--fs-12); color: var(--text-muted); line-height: 1.45; overflow-wrap: anywhere; }
.toast-close { color: var(--text-subtle); border-radius: var(--radius-sm); padding: 2px; }
.toast-close:hover { color: var(--text); background: var(--surface-2); }
.toast-enter-active, .toast-leave-active { transition: all var(--dur-base) var(--ease); }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateX(12px); }
</style>
