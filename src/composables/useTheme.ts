import { ref, watch } from "vue";

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "forgerust:theme";

function systemPrefersDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

function applyTheme(t: Theme) {
  const resolved = t === "system" ? (systemPrefersDark() ? "dark" : "light") : t;
  document.documentElement.setAttribute("data-theme", resolved);
}

const stored = (localStorage.getItem(STORAGE_KEY) as Theme | null) ?? "light";
const theme = ref<Theme>(stored);

applyTheme(theme.value);

watch(theme, (t) => {
  localStorage.setItem(STORAGE_KEY, t);
  applyTheme(t);
});

if (window.matchMedia) {
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", () => {
      if (theme.value === "system") applyTheme("system");
    });
}

export function useTheme() {
  return { theme };
}
