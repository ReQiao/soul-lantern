/**
 * 登录态的单一真相源。
 *
 * 为什么要从 AiPanel 里抽出来：登录门禁已经不只是 AI 面板内部的事了——
 * **点「AI 模式」这个动作本身**就要看登录态（未登录就不切过去，直接弹登录框），
 * 而那个按钮在 App.vue 上。两个组件各存一份 ref 会立刻不同步：
 * 在 AiPanel 里登录成功，App.vue 那份还是"未登录"，门禁就永远放不开。
 *
 * 所以这里用模块级 ref 做单例——`import` 到哪里都是同一份。Vue 的响应式本来
 * 就不要求状态住在组件里，为这点共享引入 Pinia 反而是杀鸡用牛刀。
 *
 * **token 不在这里，也不该在这里**：它由 Rust 侧（src-tauri/src/session.rs）
 * 存盘，前端只知道"登没登录、用户名是什么"。少一条被注入脚本顺走的路径。
 */
import { computed, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";

export const desktop = isTauri();

/** 管理员改过余额之后留给用户的一条留言。 */
export interface BalanceNotice {
  /** 正数是加、负数是扣。界面按正负选两套完全不同的文案。 */
  delta: number;
  balanceAfter: number;
  at: number;
}

export interface AuthState {
  loggedIn: boolean;
  username: string;
  phoneMasked: string;
  balance: number;
  activated: boolean;
  /**
   * 服务器连不上。必须和 `loggedIn=false` 分开：这两件事在界面上是两句不同的话，
   * 混成一句的话用户会以为自己账号出了问题，跑去反复重新注册。
   */
  offline: boolean;
  /** 这个账号是管理员。名字后面缀 `<管理员>` 靠它。 */
  isAdmin: boolean;
  /**
   * 当前会话已经用 ADMIN_TOKEN 解锁过管理权限。
   *
   * 和 `isAdmin` 分开是服务端的两层设计：账号标记只管显示，真正的权限跟着
   * 会话走、登出即失效、每次重新登录都要重新验一次 token。管理页入口只在
   * 这个为 true 时出现——但它**不是安全边界**，真正的拦截在服务端，
   * 前端把入口强行画出来也没用，每个调用照样被 404 挡回去。
   */
  adminVerified: boolean;
  /** 收藏的万灯集作品 id。 */
  favorites: string[];
  /**
   * 管理员改余额留下的通知。
   *
   * **服务端读到即清空**，所以这批数据一辈子只会到达客户端一次。
   * `refreshAuth` 拿到之后必须立刻搬进 `pendingNotices`（下面那个队列），
   * 否则下一次 `refreshAuth` 覆盖 `auth.value` 时就永远丢了。
   */
  notices: BalanceNotice[];
}

const LOGGED_OUT: AuthState = {
  loggedIn: false,
  username: "",
  phoneMasked: "",
  balance: 0,
  activated: false,
  offline: false,
  isAdmin: false,
  adminVerified: false,
  favorites: [],
  notices: [],
};

/**
 * 待弹出的余额变动通知。
 *
 * 【为什么要单独一个队列，不直接读 auth.value.notices】服务端那批通知是
 * **一次性**的：`/v1/auth/me` 读到就清空，第二次调用返回的是空数组。而
 * `refreshAuth()` 会被很多地方调用（启动、登录后、充值 401 之后、点 AI 模式…），
 * 只要其中任意一次发生在"通知还没来得及弹出来"之前，`auth.value` 一被覆盖，
 * 那批通知就再也找不回来了——用户永远不知道管理员给他发过币。
 * 所以拿到的第一时间就搬进这个独立的队列，弹完再出队。
 */
export const pendingNotices = ref<BalanceNotice[]>([]);

export const auth = ref<AuthState>({ ...LOGGED_OUT });

/**
 * 服务端的登录门禁开关（/v1/version 的 authRequired）。
 *
 * 取不到时默认 false——宁可放行也不要把用户锁死在一个连不上服务器的门禁后面。
 * 真去调 /v1/ai/generate 还是会被服务端 401 挡住，安全性不受影响，
 * 但用户至少能看清"是服务器连不上"而不是对着打不开的登录框发呆。
 */
export const authRequired = ref(false);

/**
 * 短信签名（用户在短信开头方括号里看到的名字），由服务端下发。
 *
 * 注册界面要在**点获取验证码之前**就把它显示出来：走的是免资质通道，署名是
 * 服务商的名字而不是「灵魂灯笼」，事先不打招呼的话用户会把验证码当诈骗短信忽略。
 * 拿不到（老服务端 / 连不上）就是 null，界面退回泛化文案。
 */
export const smsSignName = ref<string | null>(null);

/**
 * 测试开关：`localStorage` 里 `soul-lantern-gate` = "on" 时强制进入门禁状态。
 *
 * 门禁真正生效需要三件事同时成立：跑在 Tauri 里、服务端说要登录、当前没登录。
 * 前两条在浏览器里都不成立（`isTauri()` 是 false，而且开发时连的多半是还没
 * 部署新版的服务器），于是这条 UI 分支**在桌面包之外根本走不到**，
 * 只能靠"打包发出去再看"来验——那太贵了。
 *
 * 这个开关只会**多加**一道门禁，不会绕过任何鉴权（服务端该 401 还是 401），
 * 所以留在正式版里没有安全代价。
 */
function gateForced(): boolean {
  try {
    return localStorage.getItem("soul-lantern-gate") === "on";
  } catch {
    return false;
  }
}

/** 需要登录、但还没登录。门禁判断只看这一个值。 */
export const gated = computed(
  () => !auth.value.loggedIn && (gateForced() || (desktop && authRequired.value)),
);

// ---------------- 弹窗 ----------------

export type AuthMode = "login" | "register" | "reset" | "change" | "rename" | "admin";

export const authModalOpen = ref(false);
/** 打开时停在哪一屏。改密码要能直接跳过去，不然用户得先看到登录表单再自己找。 */
export const authModalMode = ref<AuthMode>("login");

/**
 * 登录成功后要不要顺势切进 AI 模式。
 *
 * 只有"点 AI 模式被门禁拦下来"这一条路径会把它设成 true。从账号区点「登录」
 * 进来的不设——那时候用户没有表达过"我要进 AI 模式"，替他切过去是自作主张。
 */
export const pendingAiSwitch = ref(false);

export function openAuth(mode: AuthMode) {
  authModalMode.value = mode;
  authModalOpen.value = true;
}

// ---------------- 刷新 ----------------

export async function refreshAuth() {
  if (!desktop) return;
  try {
    const next = await invoke<AuthState>("auth_state");
    // 先把一次性通知搬走再赋值，见 pendingNotices 的注释。
    if (next.notices?.length) pendingNotices.value.push(...next.notices);
    auth.value = next;
  } catch {
    // auth_state 自己已经把各种失败折叠成"未登录"了，走到这里说明 invoke 本身炸了
  }
}

/**
 * 用户名的显示形式：管理员后面缀 `<管理员>`。
 *
 * 用户名本身被服务端禁止含「管理员」三个字（auth.rs::validate_username），
 * 所以这个后缀不会和真名混淆。
 */
export function displayName(name: string, isAdmin: boolean): string {
  return isAdmin ? `${name} <管理员>` : name;
}

export async function refreshAuthRequired() {
  if (!desktop) return;
  try {
    authRequired.value = await invoke<boolean>("auth_required");
  } catch {
    authRequired.value = false;
  }
  try {
    smsSignName.value = await invoke<string | null>("auth_sms_sign_name");
  } catch {
    smsSignName.value = null;
  }
}

/**
 * 重新确认一次登录态和门禁开关。
 *
 * 存在的理由是一个真实的死锁：`authRequired` 默认 false，而 `auth_required`
 * 在**服务器连不上时也返回 false**（那是刻意的——宁可放行也不要把用户锁死在
 * 一个连不上服务器的门禁后面）。如果这两个值只在 onMounted 拉一次，那么
 * "软件启动那一刻恰好断网/服务器在重启" 就会让用户永远停在
 * "不显示门禁、也不显示账号区" 的状态里，界面上没有任何一处能打开登录框，
 * 网络恢复了也不行，除非重启软件。
 *
 * 所以凡是拿到"需要登录"这类信号的地方（充值 401、生成失败、点 AI 模式）
 * 都回头刷一次。
 */
export async function recheckAuth() {
  await Promise.all([refreshAuthRequired(), refreshAuth()]);
}

/** 退出登录。toast 由调用方发——这个模块不该知道界面怎么提示。 */
export async function logout() {
  try {
    await invoke("auth_logout");
  } finally {
    auth.value = { ...LOGGED_OUT };
  }
}
