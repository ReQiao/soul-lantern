<script setup lang="ts">
/**
 * 设置：贡献者计划 + 反馈 + 版本。
 *
 * 贡献者等级的规则全在服务端（奖励、30 天内降级收回），这里只负责说清楚、
 * 让用户**两次确认**之后再改。第一次确认时先用 dryRun 问服务端「这次会发 / 扣多少」，
 * 把真实数字告诉用户——奖励额是服务端配置，界面上写死的数只是参考。
 */
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { auth, desktop, openAuth, refreshAuth } from "../logic/auth";
import { useMorphPopup } from "../logic/morphPopup";
import { diagnosticsText } from "../logic/telemetry";
import { DISPLAY_VERSION, IS_PRERELEASE } from "../logic/version";

const props = defineProps<{
  open: boolean;
  origin?: HTMLElement | null;
  animate?: boolean;
}>();
const emit = defineEmits<{ "update:open": [open: boolean]; toast: [message: string] }>();

const { onEnter, onLeave } = useMorphPopup({
  getOrigin: () => props.origin,
  getAnimate: () => props.animate !== false,
});

function close() {
  emit("update:open", false);
}

const ISSUES_URL = "https://github.com/ReQiao/soul-lantern/issues";
const BILIBILI_URL = "https://space.bilibili.com/3690984732887987/";

const LEVELS = [
  {
    level: 0,
    name: "不贡献",
    reward: "",
    desc: "不上报任何数据。",
  },
  {
    level: 1,
    name: "贡献者",
    reward: "1000",
    desc: "上报电脑架构信息（系统版本、CPU、内存、屏幕尺寸、软件版本）和出错时的操作记录（最近点了哪些按钮、错误信息）。不记录你在输入框里打的字。",
  },
  {
    level: 2,
    name: "高级贡献者",
    reward: "2000",
    desc: "在贡献者的基础上，再上报 AI 模式里你输入的提示词和 AI 的输出结果，用来改进提示词和指令生成。",
  },
] as const;

const appVersion = ref("");
watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    step.value = "idle";
    if (!appVersion.value && desktop) {
      try {
        const info = await invoke<{ appVersion: string }>("telemetry_system_info");
        appVersion.value = info.appVersion;
      } catch {
        // 拿不到版本号不影响别的
      }
    }
  },
);

// ---------------- 贡献者：两次确认 ----------------

type Step = "idle" | "confirm1" | "confirm2";
const step = ref<Step>("idle");
const target = ref(0);
const previewDelta = ref(0);
const busy = ref(false);

const current = computed(() => auth.value.contributorLevel);
const targetInfo = computed(() => LEVELS[target.value]);

function levelName(level: number): string {
  return LEVELS[level]?.name ?? "";
}

async function choose(level: number) {
  if (busy.value || level === current.value) return;
  target.value = level;
  busy.value = true;
  try {
    const v = await invoke<{ delta: number }>("contributor_set", { level, dryRun: true });
    previewDelta.value = v.delta;
    step.value = "confirm1";
  } catch (err) {
    emit("toast", `出错了：${String(err)}`);
  } finally {
    busy.value = false;
  }
}

const deltaText = computed(() => {
  const d = previewDelta.value;
  if (d > 0) return `这次会获得 ${d} 灵魂币。`;
  if (d < 0) return `你升级还不满 30 天，现在改会扣回 ${-d} 灵魂币（余额不够的话扣到 0）。`;
  return target.value > current.value
    ? "这一档的奖励你之前已经领过了，这次不会再发灵魂币。"
    : "这次不会扣灵魂币。";
});

async function apply() {
  busy.value = true;
  try {
    const v = await invoke<{ level: number; delta: number }>("contributor_set", {
      level: target.value,
      dryRun: false,
    });
    await refreshAuth();
    step.value = "idle";
    const coin = v.delta > 0 ? `，获得 ${v.delta} 灵魂币` : v.delta < 0 ? `，扣回 ${-v.delta} 灵魂币` : "";
    emit("toast", v.level === 0 ? `已退出贡献者计划${coin}` : `已成为${levelName(v.level)}${coin}，谢谢！`);
  } catch (err) {
    emit("toast", `出错了：${String(err)}`);
  } finally {
    busy.value = false;
  }
}

// ---------------- 反馈 ----------------

async function openLink(url: string) {
  try {
    if (desktop) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } else {
      window.open(url, "_blank", "noopener");
    }
  } catch {
    emit("toast", "打不开链接，请手动访问：" + url);
  }
}

async function copyDiagnostics() {
  try {
    await navigator.clipboard.writeText(await diagnosticsText());
    emit("toast", "诊断信息已复制，反馈时粘贴进去就行");
  } catch {
    emit("toast", "复制失败");
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition :css="false" @enter="onEnter" @leave="onLeave">
      <div v-if="open" class="modal-overlay picker-overlay" @click.self="close">
        <div class="picker-scrim"></div>
        <div class="modal-card picker-card settings-card">
          <div class="picker-brand" aria-hidden="true">
            <span class="picker-brand-label"></span>
          </div>
          <div class="picker-inner settings-inner">
            <div class="plaza-head">
              <h2>设置</h2>
              <button class="picker-close" type="button" aria-label="关闭" @click="close">×</button>
            </div>

            <!-- ================= 贡献者计划 ================= -->
            <section class="settings-section">
              <h3>贡献者计划</h3>
              <p class="settings-note">
                自愿加入，帮我们发现问题、改进 AI 生成。奖励一次性发放；随时可以在这里取消，
                升级后 30 天内取消会收回那一档的奖励。数据只用于改进本软件，不出售、不共享给第三方。
              </p>

              <template v-if="auth.loggedIn">
                <div v-if="step === 'idle'" class="settings-levels">
                  <button
                    v-for="l in LEVELS"
                    :key="l.level"
                    type="button"
                    class="settings-level"
                    :class="{ active: current === l.level }"
                    :disabled="busy"
                    @click="choose(l.level)"
                  >
                    <span class="settings-level-head">
                      <strong>{{ l.name }}</strong>
                      <span v-if="l.reward" class="settings-reward">奖励 {{ l.reward }} 灵魂币</span>
                      <span v-if="current === l.level" class="settings-current">当前</span>
                    </span>
                    <span class="settings-level-desc">{{ l.desc }}</span>
                  </button>
                </div>

                <div v-else-if="step === 'confirm1'" class="settings-confirm">
                  <p>
                    <strong>{{ target === 0 ? "退出贡献者计划" : `成为${targetInfo.name}` }}</strong>
                  </p>
                  <p v-if="target > 0">{{ targetInfo.desc }}</p>
                  <p v-else>之后不再上报任何数据。</p>
                  <p class="settings-delta">{{ deltaText }}</p>
                  <div class="settings-actions">
                    <button type="button" @click="step = 'idle'">取消</button>
                    <button type="button" class="primary-btn" @click="step = 'confirm2'">继续</button>
                  </div>
                </div>

                <div v-else class="settings-confirm">
                  <p>
                    <strong>最后确认一次：</strong>
                    确定要{{ target === 0 ? "退出贡献者计划" : `成为${targetInfo.name}` }}吗？
                  </p>
                  <p class="settings-delta">{{ deltaText }}</p>
                  <div class="settings-actions">
                    <button type="button" :disabled="busy" @click="step = 'idle'">再想想</button>
                    <button type="button" class="primary-btn" :disabled="busy" @click="apply">
                      {{ busy ? "处理中…" : "确定" }}
                    </button>
                  </div>
                </div>
              </template>
              <p v-else class="settings-note">
                奖励发到账号上，所以要先
                <button type="button" class="auth-link" @click="openAuth('login', $event)">登录</button>
                才能加入。
              </p>
            </section>

            <!-- ================= 反馈 ================= -->
            <section class="settings-section">
              <h3>反馈问题</h3>
              <p class="settings-note">
                遇到 bug 或者有建议，欢迎反馈。每一条有效反馈都会获赠测试用的灵魂币，
                记得附上你的用户名{{ auth.loggedIn ? `（${auth.username}）` : "" }}。
                先点「复制诊断信息」再粘贴进反馈里，能帮我们更快找到问题。
              </p>
              <div class="settings-actions settings-actions-wrap">
                <button type="button" class="primary-btn" @click="openLink(ISSUES_URL)">GitHub Issues（推荐）</button>
                <button type="button" @click="openLink(BILIBILI_URL)">B 站私信</button>
                <button type="button" @click="copyDiagnostics">复制诊断信息</button>
              </div>
              <p class="settings-note settings-note-small">
                B 站私信不推荐（消息容易漏看），只在打不开 GitHub、又没有加速器时用。
              </p>
            </section>

            <!-- ================= 版本 ================= -->
            <section class="settings-section settings-version">
              灵魂灯笼 {{ DISPLAY_VERSION }}{{ IS_PRERELEASE ? "（测试版）" : "" }}{{ appVersion ? ` · 内部版本 ${appVersion}` : "" }}
            </section>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
