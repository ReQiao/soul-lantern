/**
 * 验证登录弹窗（AuthModal）改用 picker 结构+动画之后，确实在用共享的
 * "从按钮飞出来"那套动画，而不是原来那个纯淡入淡出的 modal-fade。
 *
 * 跑法：node scripts/auth-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5197;
const vite = spawn("npx", ["vite", "--port", String(PORT), "--strictPort"], { stdio: "ignore" });
process.on("exit", () => vite.kill());

async function waitForServer() {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/`);
      if (r.ok) return;
    } catch {}
    await sleep(500);
  }
  throw new Error("vite 没起来");
}
await waitForServer();

const browser = await chromium.launch({
  executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
});
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

// 登录按钮只在"桌面版"分支里渲染（浏览器里 AI 面板只显示一条"去桌面版用"提示）。
// isTauri() 就是读 window.isTauri，这里在页面脚本跑之前把它冒充成 true，
// 好让 AiPanel 露出登录按钮——invoke() 之后会因为没有真实 Tauri 后端而报错，
// 那些错误跟这次要验证的动画无关，过滤掉即可。
await page.addInitScript(() => {
  window.isTauri = true;
});

await page.goto(`http://127.0.0.1:${PORT}/`, { waitUntil: "networkidle" });
await page.evaluate(() => {
  const el = document.querySelector(".eula-text");
  if (el) el.scrollTop = el.scrollHeight;
});
await sleep(300);
await page.locator(".eula-box button.primary-btn").first().click({ force: true }).catch(() => {});
await sleep(1000);

await page.locator(".mode-switch button", { hasText: "AI 模式" }).click();
await sleep(500);

const loginBtn = page.locator(".ai-account button", { hasText: "登录 / 注册" }).first();
console.log("找到登录按钮:", await loginBtn.count());
const before = await loginBtn.boundingBox();
await loginBtn.click();
await sleep(50);

const midway = await page.evaluate(() => {
  const card = document.querySelector(".picker-card.auth-card");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  return { w: Math.round(r.width), h: Math.round(r.height) };
});
console.log("展开中途（~50ms）卡片尺寸:", midway, "起点按钮:", before && { w: Math.round(before.width), h: Math.round(before.height) });

await sleep(600);
const settled = await page.evaluate(() => {
  const card = document.querySelector(".picker-card.auth-card");
  const overlay = document.querySelector(".picker-overlay");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  return {
    w: Math.round(r.width),
    h: Math.round(r.height),
    overlayExists: !!overlay,
    title: document.querySelector(".auth-head h2")?.textContent,
    formVisible: !!document.querySelector(".auth-form"),
  };
});
console.log("展开完成:", settled);

console.log("关闭前 overlay 数量:", await page.locator(".picker-overlay").count());
await page.locator(".picker-close").click();
for (let i = 0; i < 8; i++) {
  await sleep(150);
  const n = await page.locator(".picker-overlay").count();
  console.log(`  +${(i + 1) * 150}ms overlay 数量=${n}`);
}
const closed = await page.evaluate(() => ({
  overlayGone: !document.querySelector(".picker-overlay"),
  originVisible: (() => {
    const btns = [...document.querySelectorAll(".ai-account button")].filter(
      (b) => b.textContent?.trim() === "登录 / 注册",
    );
    return btns.every((b) => getComputedStyle(b).visibility !== "hidden");
  })(),
}));
console.log("收回完成:", closed);

// 冒充桌面版之后，invoke() 会因为没有真实 Tauri 后端而报错——这些跟本次要
// 验证的弹窗动画无关，过滤掉，只关心其他意外错误。
const realErrors = errors.filter((e) => !/tauri|invoke/i.test(e));
console.log("\n控制台错误（已过滤掉冒充桌面版导致的 invoke 报错）:", realErrors.length ? realErrors.slice(0, 10) : "（无）");
await browser.close();
vite.kill();
process.exit(realErrors.length ? 1 : 0);
