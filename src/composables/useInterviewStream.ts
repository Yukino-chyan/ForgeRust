import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface TokenEvent {
  interviewId: number;
  chunk: string;
}

// 监听后端 interview-token 事件，把流式文本累积到 streamingText。
// 单窗口单场面试，无需按 interviewId 过滤；组件在每轮开始前 resetStream()。
export function useInterviewStream() {
  const streamingText = ref("");
  let unlisten: UnlistenFn | null = null;

  async function ensureListener() {
    if (unlisten) return;
    unlisten = await listen<TokenEvent>("interview-token", (e) => {
      streamingText.value += e.payload.chunk;
    });
  }

  function resetStream() {
    streamingText.value = "";
  }

  function stop() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  return { streamingText, ensureListener, resetStream, stop };
}
