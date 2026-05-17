<script setup lang="ts">
import { computed, type Component } from "vue";
import * as Lucide from "lucide-vue-next";

const props = withDefaults(
  defineProps<{
    name: string;
    size?: number | string;
    strokeWidth?: number;
  }>(),
  { size: 16, strokeWidth: 1.75 }
);

const component = computed<Component | null>(() => {
  const c = (Lucide as unknown as Record<string, Component>)[props.name];
  return c ?? null;
});
</script>

<template>
  <component
    v-if="component"
    :is="component"
    :size="size"
    :stroke-width="strokeWidth"
    class="fr-icon"
  />
</template>

<style scoped>
.fr-icon {
  flex-shrink: 0;
  display: inline-block;
  vertical-align: middle;
}
</style>
