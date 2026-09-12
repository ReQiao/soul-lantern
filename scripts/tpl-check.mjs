/**
 * 验证模板库弹窗改用 picker 结构+动画之后没有回归——它跟 ItemPickerModal/
 * AuthModal 是同一套 useMorphPopup，但内容里有两个各自 40vh 封顶的独立
 * 滚动网格，值得单独跑一遍确认没有把整块卡片的滚动能力搞丢。
 *
 * 跑法：node scripts/tpl-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5193;
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

const openBtn = page.locator(".top-form button", { hasText: "模板库" }).first();
const before = await openBtn.boundingBox();
await openBtn.click();
await sleep(50);

const midway = await page.evaluate(() => {
  const card = document.querySelector(".picker-card.tpl-card");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  return { w: Math.round(r.width), h: Math.round(r.height) };
});
console.log("展开中途（~50ms）卡片尺寸:", midway, "起点按钮:", before && { w: Math.round(before.width), h: Math.round(before.height) });

await sleep(600);
const settled = await page.evaluate(() => {
  const card = document.querySelector(".picker-card.tpl-card");
  const overlay = document.querySelector(".picker-overlay");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  const grid = document.querySelector(".tpl-grid");
  return {
    w: Math.round(r.width),
    h: Math.round(r.height),
    overlayExists: !!overlay,
    builtinCount: document.querySelectorAll(".plaza-item").length,
    gridScrollable: grid ? getComputedStyle(grid).overflowY === "auto" : null,
    innerScrollable: (() => {
      const inner = document.querySelector(".tpl-inner");
      return inner ? getComputedStyle(inner).overflowY === "auto" : null;
    })(),
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
    const btns = [...document.querySelectorAll(".top-form button")].filter((b) => b.textContent?.trim() === "模板库");
    return btns.every((b) => getComputedStyle(b).visibility !== "hidden");
  })(),
}));
console.log("收回完成:", closed);

console.log("\n控制台错误:", errors.length ? errors.slice(0, 10) : "（无）");
await browser.close();
vite.kill();
process.exit(errors.length ? 1 : 0);
