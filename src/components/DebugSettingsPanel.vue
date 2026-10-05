<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { debugState, disableDebug, refreshDebug, type DebugState } from "../logic/debug";

const cacheDir = ref("");
const eulaAccepted = ref(false);
const changing = ref(false);
const error = ref("");
let timer: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    await refreshDebug();
    if (!cacheDir.value) cacheDir.value = debugState.value.cacheDir;
  } catch (e) { error.value = String(e); }
}
onMounted(() => { void refresh(); timer = setInterval(() => { void refresh(); }, 1000); });
onUnmounted(() => clearInterval(timer));
async function choose() {
  const path = await open({ directory: true, multiple: false, title: "选择调试缓存目录", defaultPath: cacheDir.value || undefined });
  if (typeof path === "string") cacheDir.value = path;
}
async function toggle() {
  changing.value = true;
  error.value = "";
  try {
    if (debugState.value.enabled) await disableDebug();
    else debugState.value = await invoke<DebugState>("debug_enable", { cacheDir: cacheDir.value, eulaAccepted: eulaAccepted.value });
    cacheDir.value = debugState.value.cacheDir;
  } catch (e) { error.value = String(e); }
  finally { changing.value = false; }
}
</script>

<template>
  <div class="debug-settings">
    <p>本机调试只对当前已认证的管理员会话生效，退出登录、切换账号或退出程序后关闭。</p>
    <p class="debug-warning">验证会启动 Minecraft 服务端，消耗较多 CPU 与内存（Java 堆内存上限 1 GB，进程还会占用额外内存）。首次使用需要下载缺少的 JDK 21、JDK 25 和对应版本服务端；之后复用缓存。</p>
    <label for="debug-cache">调试缓存目录</label>
    <div class="debug-cache-row">
      <input id="debug-cache" v-model="cacheDir" :disabled="debugState.enabled || debugState.busy" spellcheck="false" />
      <button type="button" :disabled="debugState.enabled || debugState.busy" @click="choose">选择目录</button>
    </div>
    <label v-if="!debugState.enabled" class="check-line">
      <input v-model="eulaAccepted" type="checkbox" />我已阅读并同意 Minecraft EULA
    </label>
    <button v-if="!debugState.enabled" type="button" @click="openUrl('https://www.minecraft.net/en-us/eula')">阅读 Minecraft EULA</button>
    <button type="button" :disabled="changing || (!debugState.enabled && (!eulaAccepted || !cacheDir.trim() || debugState.busy))" @click="toggle">
      {{ changing ? '处理中…' : debugState.enabled ? '关闭调试并停止服务端' : '开启本机调试' }}
    </button>
    <p v-if="debugState.progress" role="status">{{ debugState.progress }}</p>
    <p v-if="error" class="auth-error" role="alert">{{ error }}</p>
    <p class="sub-text">开启后，AI 与手动模式的每条指令旁会显示「验证指令」。版本分组使用组内最早版本，报告会显示实际测试版本。指令仅在独立临时世界执行；基岩版暂不支持。</p>
  </div>
</template>

<style scoped>
.debug-settings { display: grid; gap: 12px; }
.debug-settings p { margin: 0; line-height: 1.6; }
.debug-cache-row { display: flex; gap: 8px; flex-wrap: wrap; }
.debug-cache-row input { flex: 1; min-width: 200px; }
.debug-warning { color: #ffc989; }
</style>
