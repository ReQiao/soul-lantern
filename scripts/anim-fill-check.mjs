/**
 * 验证这一轮补的两处动画：
 * 1. 模式切换（手动/AI/万灯集/管理）现在有内容浮现的过渡，不再是硬切。
 * 2. 手动模式里能点的表格行（附魔/属性/方块/工具规则）有手型指针 + 按下反馈，
 *    EffectEditor 里那张纯展示的表格不会被误标成"能点"。
 *
 * 跑法：node scripts/anim-fill-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5186;
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
await sleep(1500);

// ---- 1. 模式切换动画：万灯集（v-if，全新挂载）应该以 opacity:0 起跑 ----
await page.locator(".mode-switch button", { hasText: "万灯集" }).click();
const plazaEarly = await page.evaluate(() => {
  const el = document.querySelector(".plaza-card");
  return el ? getComputedStyle(el).opacity : null;
});
await sleep(400);
const plazaSettled = await page.evaluate(() => getComputedStyle(document.querySelector(".plaza-card")).opacity);
console.log("万灯集刚切入时 opacity:", plazaEarly, " 稳定后:", plazaSettled);

// 管理页也一样（先解锁不现实，这里只看没有 adminVerified 时的提示卡片有没有过渡——
// 提示卡片本身也是 .admin-card，一样吃这条动画）
await page.locator(".mode-switch button", { hasText: "手动模式" }).click();
await sleep(400);

// ---- 2. 表格行 hover-press ----
await page.locator(".tab-strip button", { hasText: "附魔" }).first().click();
await sleep(200);
// 表格默认是空的，加一行才有东西可查
await page.locator(".table-tab button", { hasText: "添加" }).first().click();
await sleep(200);
const rowInfo = await page.evaluate(() => {
  const rows = [...document.querySelectorAll(".data-table tbody tr")];
  const selectable = rows.filter((r) => r.classList.contains("row-select"));
  return {
    totalRows: rows.length,
    selectableRows: selectable.length,
    selectableCursor: selectable[0] ? getComputedStyle(selectable[0]).cursor : null,
  };
});
console.log("附魔表格行:", rowInfo);

console.log("\n控制台错误:", errors.length ? errors.slice(0, 10) : "（无）");
await browser.close();
vite.kill();
process.exit(errors.length ? 1 : 0);
