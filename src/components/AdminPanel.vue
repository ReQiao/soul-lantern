<script setup lang="ts">
/**
 * 管理页。和「手动模式」「AI 模式」并列的第三个模式。
 *
 * # 这一整页都不是安全边界
 *
 * 界面上有没有这个入口、按钮是不是灰的，全都只是**方便**。真正的拦截在服务端：
 * 每一个 `admin_*` 命令都要求当前会话已经用 ADMIN_TOKEN 解锁过，没解锁一律
 * 返回 404（不是 403——403 等于承认这儿有个管理接口）。所以就算有人把前端
 * 改了、把入口强行画出来，一个调用也过不去。
 *
 * # 危险动作的处理
 *
 * 三处要求"把名字原样打一遍"才放行：删用户、改 AUTH_PEPPER、改 LEDGER_PATH。
 * 这不是拖慢流程，是因为这三件事**不可撤销**，而它们在界面上和"改个模型名"
 * 长得一模一样，只差一次手滑。
 */
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { auth, refreshAuth } from "../logic/auth";
import CustomSelect from "./CustomSelect.vue";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{ active?: boolean }>();
const emit = defineEmits<{ toast: [message: string, duration?: number] }>();

type Tab = "users" | "env" | "policy" | "health";
const tab = ref<Tab>("users");
const tabOptions = [
  { label: "用户", value: "users" },
  { label: "环境变量", value: "env" },
  { label: "价格配置", value: "policy" },
  { label: "运行状态", value: "health" },
];

const busy = ref(false);
const errorText = ref("");

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  if (busy.value) return;
  busy.value = true;
  errorText.value = "";
  try {
    return await fn();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    // 服务端对"没解锁管理权限"返回的是 404，客户端那层会转成"服务器拒绝了
    // 这次请求"。原样显示的话看起来像功能坏了，其实是要重新认证。
    errorText.value = msg.includes("拒绝了这次请求")
      ? "管理权限没有生效，可能是重新登录过了。到 AI 模式的账号区点「管理员认证」再输一次 token。"
      : msg;
    return undefined;
  } finally {
    busy.value = false;
  }
}

// ---------------- 用户 ----------------

interface AdminUser {
  userId: string;
  username: string;
  phoneMasked: string;
  balance: number;
  activated: boolean;
  isAdmin: boolean;
  createdAt: number;
  lastLoginAt: number;
  works: number;
  favorites: number;
}

const users = ref<AdminUser[]>([]);
const userFilter = ref("");
const selected = ref<AdminUser | null>(null);
const deltaText = ref("");
const deleteConfirm = ref("");

const filteredUsers = computed(() => {
  const q = userFilter.value.trim().toLowerCase();
  if (!q) return users.value;
  return users.value.filter(
    (u) =>
      u.username.toLowerCase().includes(q) ||
      u.userId.toLowerCase().includes(q) ||
      u.phoneMasked.includes(q),
  );
});

async function loadUsers() {
  const got = await run(() => invoke<AdminUser[]>("admin_users"));
  if (got) {
    users.value = got;
    // 选中的那个人可能刚被改过余额/删掉了，跟着刷新一下引用。
    if (selected.value) {
      selected.value = got.find((u) => u.userId === selected.value?.userId) ?? null;
    }
  }
}

function pick(u: AdminUser) {
  selected.value = u;
  deltaText.value = "";
  deleteConfirm.value = "";
  errorText.value = "";
}

async function adjust(sign: 1 | -1) {
  const u = selected.value;
  if (!u) return;
  const n = Number(deltaText.value.trim());
  if (!Number.isFinite(n) || n <= 0) {
    errorText.value = "填一个大于 0 的数，加还是扣由按钮决定。";
    return;
  }
  const r = await run(() =>
    invoke<AdminUser>("admin_adjust_balance", { query: u.userId, delta: sign * Math.floor(n) }),
  );
  if (r) {
    emit("toast", `${u.username} 的余额现在是 ${r.balance}`);
    deltaText.value = "";
    await loadUsers();
  }
}

async function removeUser() {
  const u = selected.value;
  if (!u) return;
  const r = await run(() =>
    invoke("admin_delete_user", { query: u.userId, confirm: deleteConfirm.value }),
  );
  if (r !== undefined) {
    emit("toast", `已删除 ${u.username}`);
    selected.value = null;
    deleteConfirm.value = "";
    await loadUsers();
  }
}

async function impersonate() {
  const u = selected.value;
  if (!u) return;
  if (
    !confirm(
      `以「${u.username}」的身份进入软件？\n\n` +
        "你当前的管理员登录会被顶掉（本地只存得下一条会话），" +
        "要变回管理员得重新登录、重新输一次 token。",
    )
  ) {
    return;
  }
  const name = await run(() => invoke<string>("admin_impersonate", { query: u.userId }));
  if (name) {
    await refreshAuth();
    emit("toast", `现在你是「${name}」。要变回管理员请重新登录。`, 8000);
  }
}

// ---------------- 环境变量 ----------------

interface EnvEntry {
  key: string;
  value: string;
  secret: boolean;
  dangerous: boolean;
  needsRestart: boolean;
}
interface EnvView {
  path: string | null;
  writable: boolean;
  entries: EnvEntry[];
}

const env = ref<EnvView | null>(null);
/** 正在编辑哪一项，以及编辑框里的新值和确认串。 */
const editingKey = ref("");
const editValue = ref("");
const editConfirm = ref("");
const newKey = ref("");
const newValue = ref("");

async function loadEnv() {
  const got = await run(() => invoke<EnvView>("admin_get_env"));
  if (got) env.value = got;
}

function startEdit(e: EnvEntry) {
  editingKey.value = e.key;
  // 密钥类服务端只回"已设置（N 字符）"，那不是真值，不能拿它当编辑框的初始
  // 内容——否则一点保存就把那句中文写进 .env 了。
  editValue.value = e.secret ? "" : e.value;
  editConfirm.value = "";
  errorText.value = "";
}

async function saveEnv(key: string, value: string, confirm?: string) {
  const r = await run(() =>
    invoke<{ needsRestart: boolean }>("admin_set_env", { key, value, confirm: confirm || null }),
  );
  if (r) {
    editingKey.value = "";
    newKey.value = "";
    newValue.value = "";
    emit("toast", r.needsRestart ? `${key} 已保存，要重启服务端才生效` : `${key} 已保存`);
    await loadEnv();
  }
}

// ---------------- 价格 ----------------

const policyText = ref("");
const policyPath = ref("");

async function loadPolicy() {
  const got = await run(() =>
    invoke<{ path: string; exists: boolean; policy: unknown }>("admin_get_policy"),
  );
  if (got) {
    policyPath.value = got.path;
    policyText.value = JSON.stringify(got.policy, null, 2);
  }
}

async function savePolicy() {
  const r = await run(() => invoke("admin_set_policy", { policyJson: policyText.value }));
  if (r !== undefined) emit("toast", "价格配置已保存，重启服务端后生效");
}

// ---------------- 运行状态 ----------------

const health = ref<Record<string, unknown> | null>(null);

async function loadHealth() {
  const got = await run(() => invoke<Record<string, unknown>>("admin_health"));
  if (got) health.value = got;
}

async function doRestart() {
  if (!confirm("重启服务端？\n\n重启期间（大约 3~5 秒）所有人都连不上。")) return;
  const r = await run(() => invoke("admin_restart"));
  if (r !== undefined) {
    emit("toast", "已发出重启指令，大约 3~5 秒后恢复", 8000);
    health.value = null;
  }
}

/** 运行状态里那些字段的中文名。没列到的原样显示 key。 */
const HEALTH_LABELS: Record<string, string> = {
  ok: "整体正常",
  users: "用户数",
  accounts: "账户数",
  sessions: "在线会话",
  works: "万灯集作品数",
  totalBalance: "灵魂币总量",
  persistOk: "账本落盘正常",
  persistAgeSecs: "距上次落盘（秒）",
  ledgerPath: "账本路径",
  policyPath: "价格配置路径",
  envPath: ".env 路径",
  topupEnabled: "免费充值口",
  adminTokenSet: "ADMIN_TOKEN 已配",
  smsLogMode: "短信日志模式",
  aiEndpoint: "AI 接口地址",
  aiModel: "默认模型",
  serverTime: "服务器时间戳",
};

const healthRows = computed(() =>
  Object.entries(health.value ?? {}).map(([k, v]) => ({
    key: k,
    label: HEALTH_LABELS[k] ?? k,
    value: typeof v === "boolean" ? (v ? "是" : "否") : String(v),
    /** 需要一眼看到的坏消息：落盘失败、免费充值口开着、短信没真发。 */
    bad:
      (k === "persistOk" && v === false) ||
      (k === "ok" && v === false) ||
      (k === "topupEnabled" && v === true) ||
      (k === "smsLogMode" && v === true),
  })),
);

// ---------------- 载入 ----------------

async function loadTab() {
  if (tab.value === "users") await loadUsers();
  else if (tab.value === "env") await loadEnv();
  else if (tab.value === "policy") await loadPolicy();
  else await loadHealth();
}

watch(tab, () => void loadTab());
// 每次切进管理页都重拉一次：这些数据（余额、在线会话、落盘状态）随时在变，
// 缓存住只会让人对着旧数字做判断。
watch(
  () => props.active,
  (on, was) => {
    if (on && !was) void loadTab();
  },
);
</script>

<template>
  <section class="card admin-card">
    <div v-if="!auth.adminVerified" class="ai-notice">
      <p><strong>管理页需要先认证。</strong></p>
      <p>到 AI 模式的账号区点「管理员认证」，输入服务器 <code>.env</code> 里的 ADMIN_TOKEN。</p>
    </div>

    <template v-else>
      <div class="admin-head">
        <CustomSelect v-model="tab" :options="tabOptions" class="admin-tabs" />
        <button type="button" :disabled="busy" @click="loadTab">刷新</button>
        <span class="admin-who">{{ auth.username }} &lt;管理员&gt;</span>
      </div>

      <p v-if="errorText" class="auth-error">{{ errorText }}</p>

      <!-- ================= 用户 ================= -->
      <template v-if="tab === 'users'">
        <input v-model="userFilter" class="admin-search" placeholder="按用户名 / ID / 手机号筛选" />

        <div class="admin-split">
          <div class="admin-list">
            <button
              v-for="u in filteredUsers"
              :key="u.userId"
              type="button"
              class="admin-user"
              :class="{ 'admin-user-on': selected?.userId === u.userId }"
              @click="pick(u)"
            >
              <span class="admin-user-name">
                {{ u.username }}<span v-if="u.isAdmin" class="admin-badge">管理员</span>
              </span>
              <span class="admin-user-meta">{{ u.phoneMasked }} · {{ u.balance }} 币</span>
            </button>
            <p v-if="!filteredUsers.length" class="plaza-md-empty">没有匹配的用户。</p>
          </div>

          <div v-if="selected" class="admin-detail">
            <h4>{{ selected.username }}</h4>
            <dl class="admin-kv">
              <dt>用户 ID</dt><dd>{{ selected.userId }}</dd>
              <dt>手机号</dt><dd>{{ selected.phoneMasked }}</dd>
              <dt>余额</dt><dd>{{ selected.balance }} 灵魂币</dd>
              <dt>作品 / 收藏</dt><dd>{{ selected.works }} / {{ selected.favorites }}</dd>
            </dl>

            <div class="admin-block">
              <span class="field-label">
                调整余额
                <InfoTip text="改完之后，这个用户下次打开软件会看到一个弹窗，告诉他余额被管理员改了多少。" />
              </span>
              <div class="admin-row">
                <input v-model="deltaText" inputmode="numeric" placeholder="数量" />
                <button type="button" :disabled="busy" @click="adjust(1)">+ 加</button>
                <button type="button" :disabled="busy" @click="adjust(-1)">− 扣</button>
              </div>
            </div>

            <div class="admin-block">
              <span class="field-label">以他的身份进入软件</span>
              <p class="admin-hint">
                用来复现"他那边到底看到什么"。会顶掉你当前的管理员登录，
                变回来要重新登录 + 重新输 token。
              </p>
              <button type="button" :disabled="busy" @click="impersonate">切换身份</button>
            </div>

            <div class="admin-block admin-danger-block">
              <span class="field-label">删除这个账号</span>
              <p class="admin-hint">
                连同他的余额、会话、万灯集作品和评论一起删掉，<strong>不可撤销</strong>。
                确认请把用户名「{{ selected.username }}」原样填进去。
              </p>
              <div class="admin-row">
                <input v-model="deleteConfirm" :placeholder="selected.username" />
                <button
                  type="button"
                  class="plaza-danger"
                  :disabled="busy || deleteConfirm !== selected.username"
                  @click="removeUser"
                >
                  删除
                </button>
              </div>
            </div>
          </div>
          <div v-else class="admin-detail plaza-md-empty">从左边选一个用户。</div>
        </div>
      </template>

      <!-- ================= 环境变量 ================= -->
      <template v-else-if="tab === 'env'">
        <p class="admin-hint">
          文件：<code>{{ env?.path ?? "（这台服务不是用 --env-file 启动的）" }}</code>
          <span v-if="env && !env.writable" class="admin-warn">· 服务对这个文件没有写权限</span>
        </p>
        <p class="admin-hint">
          密钥类只显示"已设置（N 字符）"，<strong>不会</strong>把明文发到这里来——
          这个软件曾经因为把 API key 打进安装包被人抠出来盗刷过一次。改是照常能改的。
        </p>

        <div class="admin-env-list">
          <div v-for="e in env?.entries ?? []" :key="e.key" class="admin-env-row">
            <div class="admin-env-key">
              <code>{{ e.key }}</code>
              <span v-if="e.dangerous" class="admin-danger-tag">危险</span>
              <span v-if="e.needsRestart" class="admin-restart-tag">需重启</span>
            </div>

            <template v-if="editingKey === e.key">
              <input
                v-model="editValue"
                class="admin-env-input"
                :type="e.secret ? 'password' : 'text'"
                :placeholder="e.secret ? '输入新的值' : ''"
              />
              <input
                v-if="e.dangerous"
                v-model="editConfirm"
                class="admin-env-input"
                :placeholder="`确认请填 ${e.key}`"
              />
              <div class="admin-row">
                <button
                  type="button"
                  :disabled="busy || (e.dangerous && editConfirm !== e.key)"
                  @click="saveEnv(e.key, editValue, editConfirm)"
                >
                  保存
                </button>
                <button type="button" @click="editingKey = ''">取消</button>
              </div>
            </template>
            <template v-else>
              <span class="admin-env-value" :class="{ 'admin-env-secret': e.secret }">
                {{ e.value || "（空）" }}
              </span>
              <button type="button" class="auth-link" @click="startEdit(e)">改</button>
            </template>
          </div>
        </div>

        <div class="admin-block">
          <span class="field-label">新增一项</span>
          <div class="admin-row">
            <input v-model="newKey" placeholder="KEY_NAME" spellcheck="false" />
            <input v-model="newValue" placeholder="值" spellcheck="false" />
            <button type="button" :disabled="busy || !newKey.trim()" @click="saveEnv(newKey, newValue)">
              添加
            </button>
          </div>
        </div>
      </template>

      <!-- ================= 价格 ================= -->
      <template v-else-if="tab === 'policy'">
        <p class="admin-hint">
          文件：<code>{{ policyPath }}</code>。改完<strong>重启服务端</strong>才生效
          （价格是启动时读进内存的）。
        </p>
        <p class="admin-hint">
          保存前服务端会完整解析一遍并跑合法性检查，不合格直接拒绝——
          这一步不能省：写出一份坏配置的话，服务下次启动会<strong>拒绝启动</strong>
          （那是刻意的，价格错了宁可不开门），而那时候管理页也连不上了。
        </p>
        <textarea v-model="policyText" class="admin-policy" spellcheck="false"></textarea>
        <div class="admin-row">
          <button class="primary-btn" type="button" :disabled="busy" @click="savePolicy">
            保存价格配置
          </button>
          <button type="button" :disabled="busy" @click="loadPolicy">放弃修改，重新载入</button>
        </div>
      </template>

      <!-- ================= 运行状态 ================= -->
      <template v-else>
        <dl class="admin-kv admin-health">
          <template v-for="row in healthRows" :key="row.key">
            <dt>{{ row.label }}</dt>
            <dd :class="{ 'admin-warn': row.bad }">{{ row.value }}</dd>
          </template>
        </dl>
        <p v-if="!healthRows.length" class="plaza-md-empty">点「刷新」拉一次。</p>

        <div class="admin-block admin-danger-block">
          <span class="field-label">重启服务端</span>
          <p class="admin-hint">
            改完 <code>.env</code> 或价格配置之后用。重启期间大约 3~5 秒所有人都连不上。
          </p>
          <button type="button" class="plaza-danger" :disabled="busy" @click="doRestart">
            重启
          </button>
        </div>
      </template>
    </template>
  </section>
</template>
