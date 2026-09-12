/**
 * 验证万灯集从"弹窗"改成"跟手动模式/AI模式并列的模式"之后基本功能没坏：
 * 能从模式切换栏进去、页面内能切"手动模板/AI 模板"、列表不再显示 md 摘要、
 * 从模板库的"去万灯集逛逛"也能跳过去。
 *
 * 跑法：node scripts/plaza-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5192;
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

await page.goto(`http://127.0.0.1:${PORT}/`, { waitUntil: "networkidle" });
await page.evaluate(() => {
  const el = document.querySelector(".eula-text");
  if (el) el.scrollTop = el.scrollHeight;
});
await sleep(300);
await page.locator(".eula-box button.primary-btn").first().click({ force: true }).catch(() => {});
await sleep(1000);

console.log("模式切换按钮:", await page.locator(".mode-switch button").allTextContents());

await page.locator(".mode-switch button", { hasText: "万灯集" }).click();
await sleep(600);

const state1 = await page.evaluate(() => ({
  plazaCard: !!document.querySelector(".plaza-card"),
  noOverlay: !document.querySelector(".modal-overlay"),
  kindSwitchTexts: [...document.querySelectorAll(".plaza-kind-switch button")].map((b) => b.textContent?.trim()),
  activeKind: document.querySelector(".plaza-kind-switch button.active")?.textContent?.trim(),
  hasExcerptSpan: !!document.querySelector(".plaza-item-excerpt"),
}));
console.log("进入万灯集模式:", state1);

// 切到 AI 模板那一侧
await page.locator(".plaza-kind-switch button", { hasText: "AI 模板" }).click();
await sleep(400);
console.log("切到 AI 模板后 active:", await page.locator(".plaza-kind-switch button.active").textContent());

// 切回手动模式，再从模板库的"去万灯集逛逛"跳回来
await page.locator(".mode-switch button", { hasText: "手动模式" }).click();
await sleep(300);
await page.locator(".top-form button", { hasText: "模板库" }).click();
await sleep(700);
await page.locator(".tpl-plaza-btn").click();
await sleep(500);
const state2 = await page.evaluate(() => ({
  inPlazaMode: document.querySelector(".mode-switch button.active")?.textContent?.includes("万灯集"),
  tplModalGone: !document.querySelector(".tpl-card"),
}));
console.log("从模板库跳万灯集:", state2);

console.log("\n控制台错误:", errors.length ? errors.slice(0, 10) : "（无）");
await browser.close();
vite.kill();
process.exit(errors.length ? 1 : 0);
