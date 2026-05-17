import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface ImportProgressEvent {
  current: number;
  total: number;
  message: string;
  is_finished: boolean;
}

const state = reactive({
  active: false,
  current: 0,
  total: 0,
  message: "",
  finished: false,
});

let unlisten: UnlistenFn | null = null;

async function ensureListener() {
  if (unlisten) return;
  unlisten = await listen<ImportProgressEvent>("import-status", (event) => {
    const p = event.payload;
    state.current = p.current;
    state.total = p.total;
    state.message = p.message;
    state.finished = p.is_finished;
    state.active = !p.is_finished;
  });
}

export function useImportProgress() {
  async function startImport(filePath: string) {
    await ensureListener();
    state.active = true;
    state.finished = false;
    state.current = 0;
    state.total = 0;
    state.message = "准备导入...";
    try {
      await invoke("import_questions_from_file", { filePath });
    } catch (e) {
      state.active = false;
      throw e;
    }
  }

  function dismiss() {
    state.finished = false;
    state.message = "";
    state.current = 0;
    state.total = 0;
  }

  return { state, startImport, dismiss };
}
