import { readonly, ref } from "vue";

export type ToastKind = "success" | "error" | "warning" | "info";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

const toasts = ref<ToastItem[]>([]);
let nextId = 1;

function push(kind: ToastKind, title: string, message?: string, timeout = 3200) {
  const id = nextId++;
  toasts.value = [...toasts.value, { id, kind, title, message }];
  window.setTimeout(() => dismiss(id), timeout);
  return id;
}

function dismiss(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

export function useToast() {
  return {
    toasts: readonly(toasts),
    success: (title: string, message?: string) => push("success", title, message),
    error: (title: string, message?: string) => push("error", title, message, 5200),
    warning: (title: string, message?: string) => push("warning", title, message, 4400),
    info: (title: string, message?: string) => push("info", title, message),
    dismiss,
  };
}
