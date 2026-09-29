import { createApp } from "vue";
import App from "./App.vue";
import { installTelemetry } from "./logic/telemetry";

const app = createApp(App);
// 贡献者计划的错误捕获和操作轨迹。没加入计划的人只记在本地，不发出去（见 telemetry.ts）。
installTelemetry(app);
app.mount("#app");
