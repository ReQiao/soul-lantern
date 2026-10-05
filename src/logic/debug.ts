import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { auth, desktop } from "./auth";

export interface DebugState { enabled: boolean; busy: boolean; cacheDir: string; progress: string }
export interface DebugResult { version: string; status: "syntax_ok" | "syntax_error"; message: string; response: string; logs: string }
export const debugState = ref<DebugState>({ enabled: false, busy: false, cacheDir: "", progress: "" });
export const debugAvailable = computed(() => desktop && auth.value.adminVerified && debugState.value.enabled);
export const debugBusy = ref(false);

export async function refreshDebug() {
  if (desktop) debugState.value = await invoke<DebugState>("debug_state");
}
export async function disableDebug() {
  if (desktop) debugState.value = await invoke<DebugState>("debug_disable");
}
watch(() => [auth.value.loggedIn, auth.value.adminVerified, auth.value.username], () => {
  if (!desktop) return;
  if (!auth.value.loggedIn || !auth.value.adminVerified) {
    debugState.value.enabled = false;
    void disableDebug().catch(() => {});
  } else void refreshDebug().catch(() => {});
});
