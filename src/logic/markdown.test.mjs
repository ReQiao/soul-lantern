/**
 * 万灯集详情渲染的测试。重点全在**注入**上：这段文本来自任意登录用户，
 * 渲染结果跑在挂着 `invoke` 的 Tauri webview 里，一次 XSS 等于能直接调后端命令。
 */
import { renderMarkdown } from "./markdown.ts";

let pass = 0;
let fail = 0;
function check(name, cond) {
  if (cond) {
    pass++;
    console.log(`  PASS  ${name}`);
  } else {
    fail++;
    console.log(`  FAIL  ${name}`);
  }
}

console.log("markdown 渲染");

// ---------------- 注入 ----------------
const script = renderMarkdown("<script>alert(1)</script>");
check("原始 script 标签被转义成纯文本", !script.includes("<script") && script.includes("&lt;script"));

const img = renderMarkdown('<img src=x onerror="alert(1)">');
check("内联 HTML 属性不会活下来", !img.includes('onerror="') && img.includes("&lt;img"));

const js = renderMarkdown("[点我](javascript:alert(1))");
check("javascript: 链接不生成 a 标签", !js.includes("<a"));

const dataUri = renderMarkdown("[点我](data:text/html;base64,PHNjcmlwdD4=)");
check("data: 链接不生成 a 标签", !dataUri.includes("<a"));

const ok = renderMarkdown("[官网](https://example.com/a?b=1)");
check("http(s) 链接放行", ok.includes('data-href="https://example.com/a?b=1"'));
check("链接不用真 href（Tauri 里点普通外链会把整个应用导航走）", !ok.includes(" href="));

// 想靠一个引号提前闭合 data-href、再塞一个事件属性进去。
// 真正要验的是"**生成的标签里**没有多出属性"，而不是"整段输出里不含这个词"——
// 那个词作为纯文本出现在标签外面是完全无害的，之前那条断言写错了对象。
const quoteBreak = renderMarkdown('[x](https://a.com") onmouseover=alert(1) x="');
const tags = quoteBreak.match(/<[^>]*>/g) ?? [];
check(
  "拼不出新属性：所有生成的标签里都没有事件处理器",
  tags.every((t) => !/\son\w+\s*=/i.test(t)),
);
check(
  "带引号的 URL 干脆不当链接（转义之后它已经不是一个真 URL 了）",
  !quoteBreak.includes("<a"),
);

// ---------------- 语法 ----------------
check("标题", renderMarkdown("# 标题").includes("<h1>标题</h1>"));
check("三级标题", renderMarkdown("### 小标题").includes("<h3>小标题</h3>"));
check("粗体", renderMarkdown("这是**重点**").includes("<strong>重点</strong>"));
check("斜体", renderMarkdown("这是*斜的*").includes("<em>斜的</em>"));
check("行内代码", renderMarkdown("用 `give @s` 命令").includes("<code>give @s</code>"));

const fenced = renderMarkdown("```\ngive @s stone\n```");
check("代码块", fenced.includes("<pre><code>give @s stone</code></pre>"));
check("代码块里的星号不当强调处理", renderMarkdown("```\na * b * c\n```").includes("a * b * c"));

const list = renderMarkdown("- 一\n- 二");
check("无序列表", list.includes("<ul>") && list.split("<li>").length === 3);
const olist = renderMarkdown("1. 一\n2. 二");
check("有序列表", olist.includes("<ol>") && olist.split("<li>").length === 3);
check("引用", renderMarkdown("> 提示").includes("<blockquote>提示</blockquote>"));
check("分割线", renderMarkdown("---").includes("<hr>"));

// 列表后面接段落时要把 ul 关掉，否则后面所有内容都被吸进列表里
const mixed = renderMarkdown("- 一\n\n普通段落");
check("列表后正常闭合", mixed.includes("</ul>") && mixed.includes("<p>普通段落</p>"));

// 行内代码里的星号不该被当成强调——这是很容易写错的一处
check("行内代码内部不做强调替换", renderMarkdown("`a*b*c`").includes("<code>a*b*c</code>"));

// 没闭合的代码块不能把内容整段吞掉
check("未闭合代码块也要吐出内容", renderMarkdown("```\n没关").includes("没关"));

check("空输入不炸", renderMarkdown("") === "");

console.log(`\nResults: ${pass} passed, ${fail} failed`);
if (fail) process.exit(1);
