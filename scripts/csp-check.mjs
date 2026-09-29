/**
 * 用 tauri.conf.json 里的正式 CSP 跑一遍打包后的前端，看有没有东西被拦。
 *
 * 为什么不能只靠"在桌面包里点一点"：CSP 拦下来的东西多半**不报错、只是不生效**——
 * 玻璃折射滤镜被拦就是直接退化成纯模糊，肉眼很难分辨是"被拦了"还是"本来就这样"。
 * 这里在页面里挂 securitypolicyviolation 监听，拦了什么一条条列出来。
 *
 * 这是 Chromium（≈ Windows 的 WebView2）。macOS 的 WKWebView 对 CSP 的细节略有不同，
 * 以桌面包实测为准。
 *
 * 跑法：npm run build && node scripts/csp-check.mjs
 */
import { chromium } from "playwright";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const conf = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
const csp = Object.entries(conf.app.security.csp)
  .map(([k, v]) => `${k} ${v}`)
  .join("; ");
console.log("CSP:", csp);

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".json": "application/json",
};
const server = createServer(async (req, res) => {
  const url = new URL(req.url, "http://x");
  let path = normalize(decodeURIComponent(url.pathname)).replace(/^[/\\]+/, "");
  if (!path) path = "index.html";
  try {
    const body = await readFile(join("dist", path));
    res.writeHead(200, {
      "Content-Type": TYPES[extname(path)] ?? "application/octet-stream",
      "Content-Security-Policy": csp,
    });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const PORT = server.address().port;

const browser = await chromium.launch({
  executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
});
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
await page.addInitScript(() => {
  window.__csp = [];
  document.addEventListener("securitypolicyviolation", (e) => {
    window.__csp.push(`${e.effectiveDirective} ← ${e.blockedURI || "(inline)"} @ ${e.sourceFile}:${e.lineNumber}`);
  });
});
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});

await page.goto(`http://127.0.0.1:${PORT}/`, { waitUntil: "networkidle" });

// EULA 门禁
await page.evaluate(() => {
  const el = document.querySelector(".eula-text");
  if (el) el.scrollTop = el.scrollHeight;
});
await sleep(300);
const agree = page.locator(".eula-box button.primary-btn");
if (await agree.count()) await agree.first().click({ force: true }).catch(() => {});
await sleep(1000);

const lantern = await page.evaluate(() => {
  const img = document.querySelector(".app-bg-lantern");
  return img ? img.naturalWidth : -1;
});
console.log("背景灯笼图 naturalWidth:", lantern, lantern > 0 ? "✓" : "✗ 没加载出来");

// 模板库：玻璃折射滤镜（data: SVG 进 backdrop-filter url()）
await page.locator(".top-form button", { hasText: "模板库" }).click();
await sleep(800);
const lens = await page.evaluate(() => {
  const el = document.querySelector(".tpl-card");
  return el ? (el.style.backdropFilter || "").slice(0, 40) : null;
});
console.log("模板库玻璃滤镜:", lens);
await page.keyboard.press("Escape");
await sleep(600);

// 透明度滑杆
await page.evaluate(() => {
  const s = document.querySelector(".clarity-slider");
  if (!s) return;
  s.value = "0.2";
  s.dispatchEvent(new Event("input", { bubbles: true }));
});
await sleep(300);

// 切一圈模式
for (const name of ["AI", "万灯集", "手动"]) {
  const btn = page.locator(".mode-switch button", { hasText: name });
  if (await btn.count()) {
    await btn.first().click().catch(() => {});
    await sleep(700);
  }
}

const violations = await page.evaluate(() => [...window.__csp]);

// 反向对照：确认 CSP 真的在生效（内联脚本必须被拦）。放在收集之后，它自己那条不算。
const inlineBlocked = await page.evaluate(() => {
  window.__inlineRan = false;
  const s = document.createElement("script");
  s.textContent = "window.__inlineRan = true";
  document.head.appendChild(s);
  return !window.__inlineRan;
});
console.log("对照：内联脚本被拦:", inlineBlocked ? "✓" : "✗ CSP 没生效");

console.log(`\n被拦截: ${violations.length} 条`);
for (const v of violations) console.log("  ", v);
const otherErrors = errors.filter((e) => !/Content Security Policy/.test(e));
if (otherErrors.length) {
  console.log("\n其它页面错误:");
  for (const e of otherErrors) console.log("  ", e);
}

await browser.close();
server.close();
process.exit(violations.length || !inlineBlocked || lantern <= 0 ? 1 : 0);
