import { createApp } from "vue";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/global.css";
import "./composables/useTheme";
import App from "./App.vue";

createApp(App).mount("#app");
