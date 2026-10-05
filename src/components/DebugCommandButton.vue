<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { GiveVersion } from "../logic/builder";
import { debugAvailable, debugBusy, debugState, refreshDebug, type DebugResult } from "../logic/debug";

const props = defineProps<{ command: string; version: GiveVersion }>();
const result = ref<DebugResult | null>(null);
const error = ref("");
const running = ref(false);
let timer: ReturnType<typeof setInterval> | undefined;
let revision = 0;
watch(() => [props.command, props.version, debugAvailable.value], () => { revision++; result.value = null; error.value = ""; });
onUnmounted(() => { revision++; clearInterval(timer); });
async function validate() {
  if (debugBusy.value || debugState.value.busy || !debugAvailable.value) return;
  const current = revision;
  running.value = true;
  debugBusy.value = true;
  result.value = null;
  error.value = "";
  timer = setInterval(() => { void refreshDebug().catch(() => {}); }, 700);
  try {
    const report = await invoke<DebugResult>("debug_validate", { version: props.version, command: props.command });
    if (current === revision) result.value = report;
  } catch (e) { if (current === revision) error.value = String(e); }
  finally {
    running.value = false;
    debugBusy.value = false;
    clearInterval(timer);
    await refreshDebug().catch(() => {});
  }
}
</script>

<template>
  <div v-if="debugAvailable && version !== 'bedrock' && command.trim()" class="debug-command">
    <button type="button" :disabled="debugBusy || debugState.busy" @click="validate">{{ running ? '验证中…' : '验证指令' }}</button>
    <p v-if="running" class="debug-status" role="status">{{ debugState.progress || '正在验证管理员会话…' }}</p>
    <p v-if="error" class="auth-error" role="alert">{{ error }}</p>
    <details v-if="result" class="debug-result" :class="result.status" open>
      <summary>Minecraft {{ result.version }} · {{ result.status === 'syntax_ok' ? '语法通过' : '语法错误' }}</summary>
      <p>{{ result.message }}</p>
      <pre v-if="result.response">{{ result.response }}</pre>
      <details v-if="result.logs"><summary>服务端日志</summary><pre>{{ result.logs }}</pre></details>
    </details>
  </div>
</template>

<style scoped>
.debug-command { min-width: 0; grid-column: 1 / -1; }
.debug-status, .debug-result p { margin: 8px 0; line-height: 1.5; }
.debug-result { margin-top: 8px; padding: 10px; border: 1px solid #76bb9270; border-radius: 8px; }
.debug-result.syntax_error { border-color: #ed8c8c; }
.debug-result pre { white-space: pre-wrap; overflow-wrap: anywhere; font-size: .82rem; max-height: 260px; overflow-y: auto; }
</style>
