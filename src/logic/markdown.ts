/**
 * 一个很小的 Markdown 渲染器，专门给万灯集的作品详情用。
 *
 * # 为什么不装 marked / markdown-it
 *
 * 不是为了省一个依赖，是为了**安全边界能一眼看清**。
 *
 * 这段文本来自任意登录用户，渲染出来的 HTML 跑在 Tauri 的 webview 里，而那个
 * webview 挂着 `invoke`——一次 XSS 不是"弹个框"，是能直接调后端命令（读存档
 * 目录、写 datapack、拿登录态）。通用 Markdown 库默认**允许内联 HTML**，
 * 要安全就必须再叠一个 DOMPurify，于是安全性取决于"两个库的版本组合有没有
 * 已知绕过"，那是个需要持续跟进的东西。
 *
 * 这里反过来做：**先把整段文本的 HTML 实体全部转义掉**，之后再往里加标签。
 * 原文里任何 `<script>`、`onerror=`、`javascript:` 在第一步就已经变成纯文本了，
 * 后面的正则只可能加上白名单里那几种标签。这是"默认不可能出事"，
 * 而不是"我们努力过滤了"。
 *
 * 代价是只支持一个子集：标题、粗体、斜体、行内代码、代码块、列表、引用、
 * 分割线、链接。对"讲清楚这个模板怎么用"这件事够了。
 */

/** 第一步，也是唯一一道真正的防线。 */
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * 链接只放行 http/https。
 *
 * 【这一条不能省】`javascript:` 和 `data:text/html` 都能在 `href` 里执行脚本，
 * 而它们不含任何会被 escapeHtml 转义的字符——上一步拦不住它们，只能在这里拦。
 */
function safeUrl(raw: string): string | null {
  const u = raw.trim();
  // 【注意这里拿到的是**已经转义过**的文本】所以原文里的 `"` 到这里已经是
  // `&quot;` 了，`[^"]` 这类字符类根本看不见它。不额外拦一道的话，
  // `https://a.com"` 会被判成合法 URL，最后进到 data-href 里；虽然它只会被
  // 交给 opener 而不会当 HTML 解析（伤不到人），但那已经不是一个真 URL 了。
  if (/&(quot|#39|lt|gt|amp);/i.test(u)) return null;
  return /^https?:\/\/[^\s<>"']+$/i.test(u) ? u : null;
}

/** 行内代码的占位符前缀。用一个正常文本里不会出现的形状，避免误伤。 */
const CODE_SLOT = "CODE";

/** 行内元素。输入**必须**已经转义过。 */
function inline(escaped: string): string {
  let s = escaped;
  // 行内代码最先处理：它里面的 * 和 _ 不该再被当成强调语法。
  // 用占位符把它们摘出去，最后再放回来。
  const codes: string[] = [];
  s = s.replace(/`([^`]+)`/g, (_m, code: string) => {
    codes.push(code);
    return `${CODE_SLOT}${codes.length - 1}`;
  });

  // [文字](链接)。href 走白名单校验；不合格的整条按纯文本留下，不生成标签。
  //
  // 渲染成 data-href 而不是真的 href：在 Tauri 里点一个普通外链会**导航走**，
  // 整个应用被换成那个网页且回不来。组件那边拦 click 走 opener 插件在系统
  // 浏览器里打开。
  s = s.replace(/\[([^\]]*)\]\(([^)]+)\)/g, (m, text: string, url: string) => {
    const safe = safeUrl(url);
    if (!safe) return m;
    return `<a class="md-link" data-href="${safe}">${text || safe}</a>`;
  });

  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  // 斜体避开已经配对的 **：上一条先跑，剩下的单星号才轮到这里。
  s = s.replace(/(^|[^*])\*([^*]+)\*(?!\*)/g, "$1<em>$2</em>");

  return s.replace(
    new RegExp(`${CODE_SLOT}(\\d+)`, "g"),
    (_m, i: string) => `<code>${codes[Number(i)]}</code>`,
  );
}

/**
 * 渲染成 HTML 字符串，交给 `v-html`。
 *
 * 返回值里只可能出现下面这些标签：h1~h3 / p / strong / em / code / pre /
 * ul / ol / li / blockquote / hr / a。
 */
export function renderMarkdown(src: string): string {
  const lines = escapeHtml(src ?? "").split("\n");
  const out: string[] = [];
  let listType: "ul" | "ol" | null = null;
  let inFence = false;
  let fence: string[] = [];

  const closeList = () => {
    if (listType) {
      out.push(`</${listType}>`);
      listType = null;
    }
  };

  for (const raw of lines) {
    const line = raw.trimEnd();

    // 代码块：整段原样输出，**不做行内处理**——代码里的 * 和 [] 就该是它本身。
    if (/^```/.test(line.trim())) {
      if (inFence) {
        out.push(`<pre><code>${fence.join("\n")}</code></pre>`);
        fence = [];
        inFence = false;
      } else {
        closeList();
        inFence = true;
      }
      continue;
    }
    if (inFence) {
      fence.push(line);
      continue;
    }

    if (!line.trim()) {
      closeList();
      continue;
    }

    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      closeList();
      const level = heading[1].length;
      out.push(`<h${level}>${inline(heading[2])}</h${level}>`);
      continue;
    }

    if (/^(-{3,}|\*{3,})$/.test(line.trim())) {
      closeList();
      out.push("<hr>");
      continue;
    }

    // 转义之后 `>` 已经变成了 `&gt;`，所以引用要按转义后的形状来匹配。
    const quote = /^&gt;\s?(.*)$/.exec(line);
    if (quote) {
      closeList();
      out.push(`<blockquote>${inline(quote[1])}</blockquote>`);
      continue;
    }

    const ul = /^[-*]\s+(.*)$/.exec(line);
    if (ul) {
      if (listType !== "ul") {
        closeList();
        out.push("<ul>");
        listType = "ul";
      }
      out.push(`<li>${inline(ul[1])}</li>`);
      continue;
    }

    const ol = /^\d+\.\s+(.*)$/.exec(line);
    if (ol) {
      if (listType !== "ol") {
        closeList();
        out.push("<ol>");
        listType = "ol";
      }
      out.push(`<li>${inline(ol[1])}</li>`);
      continue;
    }

    closeList();
    out.push(`<p>${inline(line)}</p>`);
  }

  // 文件结尾还开着的代码块：把已经收集的内容照常吐出来，别整段吞掉。
  if (inFence && fence.length) out.push(`<pre><code>${fence.join("\n")}</code></pre>`);
  closeList();
  return out.join("\n");
}
