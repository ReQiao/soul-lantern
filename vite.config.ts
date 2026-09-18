import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  // 【这里曾经挂着 vite-plugin-javascript-obfuscator，已经拆掉，不要加回来】
  // 拆的理由不是它不好用，是它在这个项目里已经没有保护对象了：
  //   1. 客户端按 AGPL-3.0 开源，源码就摆在 github.com/ReQiao/soul-lantern，
  //      对着源码去读混淆产物毫无意义——混淆一个自由软件是自相矛盾的；
  //   2. 它当初真正想保护的「自然语言 → 意图 → 确定性指令」那条核心逻辑，
  //      早就整体搬到服务端了（服务端是独立的私有仓库，不在这里）。
  // 顺带：controlFlowFlattening + deadCodeInjection 让每次 build 明显变慢。
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
