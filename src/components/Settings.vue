<script setup lang="ts">
import { ref, onMounted, inject, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "./ui/Icon.vue";
import { useTheme, type Theme } from "../composables/useTheme";

const apiKey = inject<Ref<string>>("apiKey", ref(""));
const apiUrl = inject<Ref<string>>("apiUrl", ref("https://zenmux.ai/api/v1/chat/completions"));

const localKey = ref("");
const localUrl = ref("");
const saving = ref(false);
const message = ref("");
const messageType = ref<"ok" | "err" | "">("");

const { theme } = useTheme();
const themeOptions: { value: Theme; label: string; icon: string }[] = [
  { value: "light",  label: "浅色",     icon: "Sun" },
  { value: "dark",   label: "深色",     icon: "Moon" },
  { value: "system", label: "跟随系统", icon: "Monitor" },
];

onMounted(async () => {
  try {
    const cfg: any = await invoke("get_api_config");
    localKey.value = cfg.api_key ?? "";
    localUrl.value = cfg.api_url ?? localUrl.value;
  } catch (e) {
    console.warn("加载 API 配置失败", e);
  }
});

async function save() {
  saving.value = true;
  message.value = "";
  messageType.value = "";
  try {
    await invoke("set_api_config", { apiKey: localKey.value, apiUrl: localUrl.value });
    apiKey.value = localKey.value;
    apiUrl.value = localUrl.value;
    message.value = "已保存";
    messageType.value = "ok";
  } catch (e) {
    message.value = String(e);
    messageType.value = "err";
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="fr-page settings">
    <header>
      <h1 class="fr-page-title">设置</h1>
      <p class="fr-page-subtitle">配置 API 凭据与外观偏好。</p>
    </header>

    <section class="fr-card group">
      <h2 class="group-title">API 凭据</h2>
      <p class="group-desc">配置 OpenAI 协议兼容的 LLM 端点。所有 AI 评分与出题都通过这里。</p>

      <div class="field">
        <label>API Key</label>
        <input v-model="localKey" type="password" class="fr-input" placeholder="sk-..." />
      </div>
      <div class="field">
        <label>API URL</label>
        <input v-model="localUrl" type="text" class="fr-input" placeholder="https://..." />
      </div>

      <div class="actions">
        <span :class="['msg', messageType]">
          <Icon v-if="messageType === 'ok'" name="Check" :size="14" />
          <Icon v-else-if="messageType === 'err'" name="X" :size="14" />
          {{ message }}
        </span>
        <button class="fr-btn fr-btn-primary" :disabled="saving" @click="save">
          {{ saving ? "保存中..." : "保存" }}
        </button>
      </div>
    </section>

    <section class="fr-card group">
      <h2 class="group-title">外观</h2>
      <p class="group-desc">主题偏好会保存在本地。</p>

      <div class="theme-row">
        <label
          v-for="opt in themeOptions"
          :key="opt.value"
          :class="['theme-pick', { active: theme === opt.value }]"
        >
          <input type="radio" :value="opt.value" v-model="theme" />
          <Icon :name="opt.icon" :size="16" />
          <span>{{ opt.label }}</span>
        </label>
      </div>
    </section>

    <section class="fr-card group">
      <h2 class="group-title">关于</h2>
      <dl class="about">
        <dt>版本</dt><dd class="fr-mono">v1.0</dd>
        <dt>技术栈</dt><dd>Tauri 2 · Vue 3 · SQLite · Rust</dd>
      </dl>
    </section>
  </div>
</template>

<style scoped>
.settings {
  max-width: 720px;
  margin: 0 auto;
}
/* 块级布局：子项按自然高度堆叠并溢出，交给 .fr-page 的 overflow-y:scroll 滚动。
   不可用 flex 列，否则 flex-shrink 会把子项压扁、内容被裁而无法滚动。
   用相邻间距替代原来的 gap。 */
.settings > * + * {
  margin-top: var(--sp-4);
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}
.group-title {
  font-size: var(--fs-16);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.group-desc {
  font-size: var(--fs-13);
  color: var(--text-muted);
  margin-top: -4px;
  margin-bottom: var(--sp-2);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field label {
  font-size: var(--fs-12);
  font-weight: var(--fw-medium);
  color: var(--text-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: var(--sp-3);
  margin-top: var(--sp-2);
}
.msg {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--fs-12);
  color: var(--text-subtle);
}
.msg.ok { color: var(--success); }
.msg.err { color: var(--danger); }

.theme-row {
  display: flex;
  gap: var(--sp-2);
}
.theme-pick {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-size: var(--fs-13);
  color: var(--text-muted);
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease);
}
.theme-pick input { display: none; }
.theme-pick:hover { border-color: var(--border-strong); color: var(--text); }
.theme-pick.active {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: transparent;
  font-weight: var(--fw-medium);
}

.about {
  display: grid;
  grid-template-columns: 80px 1fr;
  gap: 8px 16px;
  font-size: var(--fs-13);
}
.about dt { color: var(--text-muted); }
.about dd { color: var(--text); }
</style>
