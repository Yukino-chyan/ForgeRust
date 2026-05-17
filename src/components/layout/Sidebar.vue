<script setup lang="ts">
import Icon from "../ui/Icon.vue";

export type View =
  | "dashboard"
  | "training"
  | "wrong_book"
  | "ai_generate"
  | "question_library"
  | "mock_interview"
  | "settings";

defineProps<{
  current: View;
  apiConfigured: boolean;
}>();

defineEmits<{
  (e: "navigate", view: View): void;
}>();

interface NavItem {
  view: View;
  label: string;
  icon: string;
  badge?: string;
  disabled?: boolean;
}

const primary: NavItem[] = [
  { view: "dashboard",        label: "概览",     icon: "LayoutDashboard" },
  { view: "training",         label: "题库训练", icon: "BookOpen" },
  { view: "wrong_book",       label: "错题本",   icon: "Bookmark" },
  { view: "ai_generate",      label: "AI 出题",  icon: "Sparkles" },
  { view: "question_library", label: "题库管理", icon: "Library" },
];

const secondary: NavItem[] = [
  { view: "mock_interview", label: "模拟面试", icon: "MessageSquare", badge: "即将上线", disabled: true },
];
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">F</div>
      <div class="brand-text">
        <div class="brand-name">ForgeRust</div>
        <div class="brand-sub">v1.0 · 面试备考</div>
      </div>
    </div>

    <nav class="nav">
      <button
        v-for="item in primary"
        :key="item.view"
        :class="['nav-item', { active: current === item.view }]"
        @click="$emit('navigate', item.view)"
      >
        <Icon :name="item.icon" :size="16" />
        <span class="nav-label">{{ item.label }}</span>
      </button>
    </nav>

    <div class="nav-divider"></div>

    <nav class="nav nav-secondary">
      <button
        v-for="item in secondary"
        :key="item.view"
        :class="['nav-item', { active: current === item.view, disabled: item.disabled }]"
        :disabled="item.disabled"
      >
        <Icon :name="item.icon" :size="16" />
        <span class="nav-label">{{ item.label }}</span>
        <span v-if="item.badge" class="nav-badge">{{ item.badge }}</span>
      </button>
    </nav>

    <div class="footer">
      <button
        :class="['nav-item', 'settings-entry', { active: current === 'settings' }]"
        @click="$emit('navigate', 'settings')"
      >
        <Icon name="Settings" :size="16" />
        <span class="nav-label">设置</span>
        <span :class="['api-dot', apiConfigured ? 'ok' : 'warn']" :title="apiConfigured ? 'API 已配置' : '未配置 API Key'"></span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-w);
  flex-shrink: 0;
  height: 100%;
  background: var(--surface);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: var(--sp-4) var(--sp-3);
}

.brand {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-2) var(--sp-6);
}
.brand-mark {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  background: var(--accent);
  color: var(--text-on-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-mono);
  font-weight: var(--fw-semibold);
  font-size: var(--fs-16);
  flex-shrink: 0;
}
.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
  min-width: 0;
}
.brand-name {
  font-weight: var(--fw-semibold);
  font-size: var(--fs-14);
  color: var(--text);
}
.brand-sub {
  font-size: 11px;
  color: var(--text-subtle);
  margin-top: 2px;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  width: 100%;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  font-size: var(--fs-13);
  color: var(--text-muted);
  text-align: left;
  transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.nav-item:hover:not(:disabled):not(.active) {
  background: var(--surface-2);
  color: var(--text);
}
.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: var(--fw-medium);
}
.nav-item.disabled {
  cursor: not-allowed;
  color: var(--text-subtle);
}
.nav-label { flex: 1; }
.nav-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--surface-2);
  color: var(--text-subtle);
  border: 1px solid var(--border);
  font-weight: var(--fw-regular);
}

.nav-divider {
  height: 1px;
  background: var(--border);
  margin: var(--sp-3) var(--sp-2);
}

.footer {
  margin-top: auto;
  padding-top: var(--sp-3);
  border-top: 1px solid var(--border);
}

.api-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-left: auto;
  flex-shrink: 0;
}
.api-dot.ok   { background: var(--success); }
.api-dot.warn { background: var(--warning); }
</style>
