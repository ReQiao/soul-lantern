<script setup lang="ts">
/**
 * 万灯集 —— 用户之间互相分享模板的广场。
 *
 * 名字的来历：每个人发出去的一份模板就是一盏灯，聚起来是一条灯火长街。
 * `集` 一层是集市、一层是文集，正好对应"能逛"和"能收藏"两件事。
 *
 * 和「手动模式」「AI 模式」「管理」并列的第四个模式，不是从某个模式里弹出来的
 * 小窗口——以前是弹窗，问题是手动模式和 AI 模式各挂一份实例、各自的入口按钮，
 * 用户逛完手动模板想接着看 AI 模板还得先关掉再从另一边打开。现在只有一份，
 * 靠页面内的 `kind` 切换看哪一侧，跟点哪个按钮进来的无关。
 *
 * 内部装三屏（列表 / 详情 / 发布），用 `view` 切。做成三个独立组件的话
 * 「列表 → 详情 → 返回列表」要把筛选条件、滚动位置在组件之间搬来搬去，
 * 而它们本来就是同一件事的三个阶段。
 *
 * `kind` 决定看的是手动模板还是 AI 模板——服务端也是同一张表用 `kind` 区分，
 * 那是因为它们在产品上是同一件事（"别人做好的东西我拿来用"），只有 payload
 * 的含义不同：手动是表单 JSON，AI 是一段提示词。发布用的 payload 由父组件
 * 两份都传进来（`manualPayload`/`aiPayload`），这边只按当前 `kind` 挑一份用——
 * 手动表单和 AI 输入框分别活在别的组件里，这边不该也不需要知道它们的细节。
 */
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { auth, desktop, openAuth } from "../logic/auth";
import { renderMarkdown } from "../logic/markdown";
import CustomSelect from "./CustomSelect.vue";
import InfoTip from "./InfoTip.vue";

const props = defineProps<{
  manualPayload: string;
  aiPayload: string;
  /** 从哪个模式切进来的，决定刚打开时看哪一侧——纯图方便，不是限制，
   *  页面里随时能用上面那个切换器换到另一侧。 */
  defaultKind?: "manual" | "ai";
}>();
const emit = defineEmits<{
  toast: [message: string];
  /** 用户在"手动模板"这一侧点了"用这个"。 */
  useManual: [payload: string, title: string];
  /** 用户在"AI 模板"这一侧点了"用这个"。 */
  useAi: [payload: string, title: string];
}>();

const kind = ref<"manual" | "ai">(props.defaultKind ?? "manual");
const currentPayload = computed(() => (kind.value === "manual" ? props.manualPayload : props.aiPayload));

interface Brief {
  id: string;
  kind: string;
  title: string;
  icon: string | null;
  category: string;
  authorName: string;
  createdAt: number;
  likes: number;
  downloads: number;
  commentCount: number;
  liked: boolean;
  favorited: boolean;
  excerpt: string;
}
/**
 * 列表项里的评论**条数**。
 *
 * 服务端把这个字段叫 `commentCount` 而不是 `comments`，是因为详情视图会把
 * brief 摊平之后再带一个 `comments: Comment[]`——同名的话 JSON 里会出现两个
 * `comments` 键（一个数字一个数组）。这个冲突最早就是被这边的类型检查撞出来的。
 */
interface Comment {
  id: string;
  authorId: string;
  authorName: string;
  body: string;
  createdAt: number;
}
interface Detail extends Brief {
  detailMd: string;
  payload: string;
  /** 完整的评论列表。列表视图那边只有条数（`commentCount`）。 */
  comments: Comment[];
  canDelete: boolean;
  authorId: string;
}

type View = "list" | "detail" | "publish";
const view = ref<View>("list");
const busy = ref(false);
const errorText = ref("");

// ---------------- 列表 ----------------

const works = ref<Brief[]>([]);
const categories = ref<{ key: string; label: string }[]>([]);
const filterCategory = ref("");
const filterScope = ref<"" | "mine" | "favorites">("");
const sort = ref("new");
const search = ref("");

const categoryOptions = computed(() => [
  { label: "全部分类", value: "" },
  ...categories.value.map((c) => ({ label: c.label, value: c.key })),
]);
const scopeOptions = [
  { label: "全部作品", value: "" },
  { label: "我发布的", value: "mine" },
  { label: "我收藏的", value: "favorites" },
];
const sortOptions = [
  { label: "最新", value: "new" },
  { label: "最热", value: "hot" },
  { label: "下载最多", value: "downloads" },
];

const kindLabel = computed(() => (kind.value === "manual" ? "手动模板" : "AI 模板"));

function categoryLabel(key: string): string {
  return categories.value.find((c) => c.key === key)?.label ?? key;
}

/** 时间戳 → "3 天前"。绝对时间在这种列表里没人读，相对时间才是有信息量的那个。 */
function ago(ts: number): string {
  const s = Math.max(0, Math.floor(Date.now() / 1000) - ts);
  if (s < 60) return "刚刚";
  if (s < 3600) return `${Math.floor(s / 60)} 分钟前`;
  if (s < 86400) return `${Math.floor(s / 3600)} 小时前`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)} 天前`;
  return new Date(ts * 1000).toLocaleDateString();
}

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  if (busy.value) return;
  busy.value = true;
  errorText.value = "";
  try {
    return await fn();
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
    return undefined;
  } finally {
    busy.value = false;
  }
}

async function loadList() {
  if (!desktop) return;
  const q = new URLSearchParams();
  q.set("kind", kind.value);
  if (filterCategory.value) q.set("category", filterCategory.value);
  if (filterScope.value === "mine") q.set("mine", "true");
  if (filterScope.value === "favorites") q.set("favorites", "true");
  if (search.value.trim()) q.set("q", search.value.trim());
  q.set("sort", sort.value);
  const got = await run(() => invoke<Brief[]>("plaza_list", { query: q.toString() }));
  if (got) works.value = got;
}

async function loadCategories() {
  if (!desktop || categories.value.length) return;
  try {
    categories.value = await invoke<{ key: string; label: string }[]>("plaza_categories");
  } catch {
    // 拿不到分类不该挡住浏览——筛选器退化成只有"全部分类"，列表照常能看。
  }
}

// 筛选条件一变就重拉。搜索框也走这条，简单直接——广场的数据量是"一个人发
// 几份作品"的量级，不值得为它做防抖和分页。
watch([filterCategory, filterScope, sort, search], () => void loadList());

// 切"手动模板 / AI 模板"相当于换了一整张表，分类也不共用——回列表、清筛选、
// 重新拉一遍分类和作品，不然会带着上一侧的筛选条件看这一侧的数据。
watch(kind, async () => {
  view.value = "list";
  errorText.value = "";
  filterCategory.value = "";
  categories.value = [];
  await loadCategories();
  await loadList();
});

onMounted(async () => {
  await loadCategories();
  await loadList();
});

// ---------------- 详情 ----------------

const detail = ref<Detail | null>(null);
const commentBody = ref("");
const detailHtml = computed(() =>
  detail.value?.detailMd ? renderMarkdown(detail.value.detailMd) : "",
);

async function openDetail(id: string) {
  const got = await run(() => invoke<Detail>("plaza_get", { id }));
  if (got) {
    detail.value = got;
    commentBody.value = "";
    view.value = "detail";
  }
}

/**
 * 详情里的链接。
 *
 * 渲染出来的是 `data-href` 而不是真 `href`（见 logic/markdown.ts）：在 Tauri 里
 * 点一个普通外链会把整个应用**导航走**且回不来。这里拦下点击，交给系统浏览器。
 */
function onDetailClick(e: MouseEvent) {
  const a = (e.target as HTMLElement | null)?.closest?.("a.md-link") as HTMLElement | null;
  const href = a?.dataset.href;
  if (!href) return;
  e.preventDefault();
  void openUrl(href).catch(() => emit("toast", "打不开这个链接"));
}

/** 登录才能做的动作，统一在这里挡一道，省得每个按钮各写一遍。 */
function needLogin(): boolean {
  if (auth.value.loggedIn) return false;
  emit("toast", "这个操作需要先登录");
  openAuth("login");
  return true;
}

async function toggleLike() {
  const d = detail.value;
  if (!d || needLogin()) return;
  const r = await run(() => invoke<{ liked: boolean; likes: number }>("plaza_like", { id: d.id }));
  if (r) {
    d.liked = r.liked;
    d.likes = r.likes;
    // 列表里那一条也要跟着变，否则返回列表会看到旧的点赞数。
    const inList = works.value.find((w) => w.id === d.id);
    if (inList) {
      inList.liked = r.liked;
      inList.likes = r.likes;
    }
  }
}

async function toggleFavorite() {
  const d = detail.value;
  if (!d || needLogin()) return;
  const r = await run(() => invoke<{ favorited: boolean }>("plaza_favorite", { id: d.id }));
  if (r) {
    d.favorited = r.favorited;
    const inList = works.value.find((w) => w.id === d.id);
    if (inList) inList.favorited = r.favorited;
    emit("toast", r.favorited ? "已收藏" : "已取消收藏");
    // 正停在"我收藏的"筛选里时，取消收藏之后这一条就不该还在列表里。
    if (filterScope.value === "favorites") void loadList();
  }
}

function useThis() {
  const d = detail.value;
  if (!d) return;
  // 下载计数是"记一笔"，失败不该挡住用户真正要做的事——内容已经在手上了。
  void invoke("plaza_download", { id: d.id }).catch(() => {});
  if (kind.value === "manual") emit("useManual", d.payload, d.title);
  else emit("useAi", d.payload, d.title);
}

async function postComment() {
  const d = detail.value;
  if (!d || needLogin()) return;
  const body = commentBody.value.trim();
  if (!body) return;
  const r = await run(() => invoke<Comment[]>("plaza_comment", { id: d.id, body }));
  if (r) {
    d.comments = r;
    commentBody.value = "";
  }
}

async function removeComment(commentId: string) {
  const d = detail.value;
  if (!d) return;
  const r = await run(() =>
    invoke<Comment[]>("plaza_delete_comment", { id: d.id, commentId }),
  );
  if (r) d.comments = r;
}

async function removeWork() {
  const d = detail.value;
  if (!d) return;
  if (!confirm(`确定要删除「${d.title}」吗？删了就找不回来了。`)) return;
  const ok = await run(() => invoke("plaza_delete", { id: d.id }));
  if (ok !== undefined) {
    emit("toast", "作品已删除");
    view.value = "list";
    await loadList();
  }
}

/**
 * 评论区那一条能不能删。
 *
 * 前端这个判断只决定"要不要画那个按钮"，不是权限本身——服务端会独立再判一次
 * （评论作者 / 作品作者 / 管理员三种人能删）。这里少判一种情况顶多是按钮没
 * 出来，多判一种也只是点了之后被服务端拒掉。
 */
function canRemoveComment(c: Comment): boolean {
  const d = detail.value;
  if (!d || !auth.value.loggedIn) return false;
  return c.authorName === auth.value.username || d.canDelete;
}

// ---------------- 发布 ----------------

const pubTitle = ref("");
const pubIcon = ref("");
const pubCategory = ref("other");
const pubDetail = ref("");

function startPublish() {
  if (needLogin()) return;
  if (!currentPayload.value.trim()) {
    emit(
      "toast",
      kind.value === "manual"
        ? "先把表单填好再发布——发布的是当前这份配置"
        : "先在输入框里写点东西再发布——发布的是你的提示词",
    );
    return;
  }
  pubTitle.value = "";
  pubIcon.value = "";
  pubCategory.value = "other";
  pubDetail.value = "";
  errorText.value = "";
  view.value = "publish";
}

async function doPublish() {
  const title = pubTitle.value.trim();
  if (!title) {
    errorText.value = "给作品起个名字吧。";
    return;
  }
  const got = await run(() =>
    invoke<Detail>("plaza_publish", {
      kind: kind.value,
      title,
      icon: pubIcon.value.trim() || null,
      category: pubCategory.value,
      detailMd: pubDetail.value,
      payload: currentPayload.value,
    }),
  );
  if (got) {
    emit("toast", "已发布到万灯集");
    detail.value = got;
    view.value = "detail";
    await loadList();
  }
}
</script>

<template>
  <section class="card plaza-card">
    <!-- ---------------- 头部 ---------------- -->
    <div class="plaza-head">
      <h2>
        <span class="plaza-lantern">🏮</span>
        万灯集
        <InfoTip
          text="大家互相分享模板的地方。每个人发出去的一份模板就是一盏灯。手动模板存的是整套表单配置，AI 模板存的是一段提示词。"
        />
      </h2>
      <!-- 看手动模板还是 AI 模板，跟从哪个模式点进来无关，随时能在这儿换——
           以前这是两个各自绑死一种 kind 的弹窗实例，现在只有一份。 -->
      <div class="mode-switch plaza-kind-switch" role="tablist">
        <button
          type="button"
          role="tab"
          :aria-selected="kind === 'manual'"
          :class="{ active: kind === 'manual' }"
          @click="kind = 'manual'"
        >手动模板</button>
        <button
          type="button"
          role="tab"
          :aria-selected="kind === 'ai'"
          :class="{ active: kind === 'ai' }"
          @click="kind = 'ai'"
        >AI 模板</button>
      </div>
    </div>

    <!-- ================= 列表 ================= -->
    <template v-if="view === 'list'">
      <div class="plaza-filters">
        <CustomSelect v-model="filterCategory" :options="categoryOptions" />
        <CustomSelect v-model="filterScope" :options="scopeOptions" />
        <CustomSelect v-model="sort" :options="sortOptions" />
        <input
          v-model="search"
          class="plaza-search"
          placeholder="搜作品名 / 作者"
          spellcheck="false"
        />
        <button class="primary-btn plaza-pub-btn" type="button" @click="startPublish">
          发布我的
        </button>
      </div>

      <p v-if="errorText" class="auth-error">{{ errorText }}</p>

      <div v-if="busy && !works.length" class="plaza-empty">正在把灯点上…</div>
      <div v-else-if="!works.length" class="plaza-empty">
        <p v-if="filterScope === 'favorites'">还没有收藏任何作品。</p>
        <p v-else-if="filterScope === 'mine'">你还没有发布过{{ kindLabel }}。</p>
        <p v-else>这里还空着——发布第一份{{ kindLabel }}吧。</p>
      </div>

      <div v-else class="plaza-grid">
        <button
          v-for="w in works"
          :key="w.id"
          type="button"
          class="plaza-item"
          @click="openDetail(w.id)"
        >
          <span class="plaza-item-icon">{{ w.icon || "🏮" }}</span>
          <span class="plaza-item-body">
            <span class="plaza-item-title">{{ w.title }}</span>
            <!-- 【不要把 excerpt 加回来】点进详情之前不该看到内容摘要——那是
                 作者写的 Markdown 说明，点进去看才是"逛"的乐趣，列表里剧透
                 完了详情页就没人点了。见 Detail 里的 excerpt 字段依然保留
                 （服务端还在下发），只是这里不渲染。 -->
            <span class="plaza-item-meta">
              <span class="plaza-tag">{{ categoryLabel(w.category) }}</span>
              <span>{{ w.authorName }}</span>
              <span>{{ ago(w.createdAt) }}</span>
              <span :class="{ 'plaza-liked': w.liked }">♥ {{ w.likes }}</span>
              <span>↓ {{ w.downloads }}</span>
              <span v-if="w.commentCount">💬 {{ w.commentCount }}</span>
                    <span v-if="w.favorited" class="plaza-faved">★ 已收藏</span>
                  </span>
                </span>
              </button>
            </div>
          </template>

          <!-- ================= 详情 ================= -->
          <template v-else-if="view === 'detail' && detail">
            <div class="plaza-detail-head">
              <button type="button" class="auth-link" @click="view = 'list'">← 返回列表</button>
            </div>

            <div class="plaza-detail-title">
              <span class="plaza-item-icon">{{ detail.icon || "🏮" }}</span>
              <div>
                <h3>{{ detail.title }}</h3>
                <p class="plaza-detail-meta">
                  <span class="plaza-tag">{{ categoryLabel(detail.category) }}</span>
                  {{ detail.authorName }} · {{ ago(detail.createdAt) }} ·
                  ↓ {{ detail.downloads }}
                </p>
              </div>
            </div>

            <!-- v-html 的输入只可能来自 renderMarkdown：它先把整段文本转义再加
                 白名单标签，原文里的 <script>/onerror= 在第一步就已经是纯文本了。
                 别在这里塞任何别的来源。 -->
            <div v-if="detailHtml" class="plaza-md" @click="onDetailClick" v-html="detailHtml"></div>
            <p v-else class="plaza-md plaza-md-empty">作者没有写说明。</p>

            <div class="plaza-actions">
              <button class="primary-btn" type="button" @click="useThis">
                {{ kind === "manual" ? "载入这份模板" : "用这段提示词" }}
              </button>
              <button type="button" :class="{ 'plaza-on': detail.liked }" @click="toggleLike">
                ♥ {{ detail.likes }}
              </button>
              <button type="button" :class="{ 'plaza-on': detail.favorited }" @click="toggleFavorite">
                {{ detail.favorited ? "★ 已收藏" : "☆ 收藏" }}
              </button>
              <button v-if="detail.canDelete" type="button" class="plaza-danger" @click="removeWork">
                删除
              </button>
            </div>

            <p v-if="errorText" class="auth-error">{{ errorText }}</p>

            <div class="plaza-comments">
              <h4>评论（{{ detail.comments.length }}）</h4>
              <div v-for="c in detail.comments" :key="c.id" class="plaza-comment">
                <div class="plaza-comment-head">
                  <strong>{{ c.authorName }}</strong>
                  <span>{{ ago(c.createdAt) }}</span>
                  <button
                    v-if="canRemoveComment(c)"
                    type="button"
                    class="auth-link"
                    @click="removeComment(c.id)"
                  >
                    删除
                  </button>
                </div>
                <p class="plaza-comment-body">{{ c.body }}</p>
              </div>
              <p v-if="!detail.comments.length" class="plaza-md-empty">还没有人评论。</p>

              <div class="plaza-comment-form">
                <input
                  v-model="commentBody"
                  placeholder="说点什么…"
                  maxlength="500"
                  @keydown.enter="postComment"
                />
                <button type="button" :disabled="busy || !commentBody.trim()" @click="postComment">
                  发表
                </button>
              </div>
            </div>
          </template>

          <!-- ================= 发布 ================= -->
          <template v-else-if="view === 'publish'">
            <div class="plaza-detail-head">
              <button type="button" class="auth-link" @click="view = 'list'">← 返回列表</button>
            </div>

            <div class="auth-form plaza-publish">
              <p class="auth-hint">
                发布的是<strong>你现在的{{ kind === "manual" ? "表单配置" : "提示词" }}</strong>。
                发布之后可以在「我发布的」里找到它，也随时能删。
              </p>

              <label class="auth-field">
                <span>作品名称</span>
                <input v-model="pubTitle" maxlength="40" placeholder="比如：一刀秒杀的审判之剑" />
              </label>

              <div class="plaza-pub-row">
                <label class="auth-field plaza-pub-icon">
                  <span>图标（可选）</span>
                  <input v-model="pubIcon" maxlength="2" placeholder="⚔" />
                </label>
                <label class="auth-field plaza-pub-cat">
                  <span>分类</span>
                  <CustomSelect
                    v-model="pubCategory"
                    :options="categories.map((c) => ({ label: c.label, value: c.key }))"
                  />
                </label>
              </div>

              <label class="auth-field">
                <span>
                  作品详情
                  <InfoTip text="支持 Markdown：# 标题、**粗体**、`代码`、- 列表、> 引用、```代码块```、[文字](链接)。" />
                </span>
                <textarea
                  v-model="pubDetail"
                  class="plaza-pub-detail"
                  maxlength="4000"
                  placeholder="讲讲这个模板是干什么的、怎么用、有什么注意事项…（支持 Markdown）"
                ></textarea>
              </label>

              <p v-if="errorText" class="auth-error">{{ errorText }}</p>

              <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doPublish">
                {{ busy ? "发布中…" : "发布到万灯集" }}
              </button>
            </div>
          </template>
  </section>
</template>
