import { computed, onUnmounted, ref } from "vue";

type SpeechRecognitionLike = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onstart: (() => void) | null;
  onend: (() => void) | null;
  onerror: ((event: { error?: string }) => void) | null;
  onresult: ((event: { results: SpeechRecognitionResultList; resultIndex: number }) => void) | null;
  start: () => void;
  stop: () => void;
  abort: () => void;
};

type SpeechRecognitionCtor = new () => SpeechRecognitionLike;

declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  }
}

export function useSpeechRecognition(onText: (text: string) => void) {
  const listening = ref(false);
  const error = ref("");
  const supported = computed(() => Boolean(window.SpeechRecognition || window.webkitSpeechRecognition));
  let recognition: SpeechRecognitionLike | null = null;

  function ensureRecognition() {
    const Ctor = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!Ctor) return null;
    if (recognition) return recognition;

    recognition = new Ctor();
    recognition.lang = "zh-CN";
    recognition.continuous = false;
    recognition.interimResults = false;
    recognition.onstart = () => {
      listening.value = true;
      error.value = "";
    };
    recognition.onend = () => {
      listening.value = false;
    };
    recognition.onerror = (event) => {
      error.value = event.error || "speech-error";
      listening.value = false;
    };
    recognition.onresult = (event) => {
      const chunks: string[] = [];
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const result = event.results.item(i);
        if (result?.isFinal && result[0]?.transcript) chunks.push(result[0].transcript.trim());
      }
      const text = chunks.filter(Boolean).join("");
      if (text) onText(text);
    };
    return recognition;
  }

  function start() {
    const rec = ensureRecognition();
    if (!rec) {
      error.value = "unsupported";
      return false;
    }
    if (listening.value) return true;
    rec.start();
    return true;
  }

  function stop() {
    if (recognition && listening.value) recognition.stop();
  }

  function toggle() {
    if (listening.value) {
      stop();
      return true;
    }
    return start();
  }

  onUnmounted(() => {
    if (recognition) recognition.abort();
  });

  return { supported, listening, error, start, stop, toggle };
}
