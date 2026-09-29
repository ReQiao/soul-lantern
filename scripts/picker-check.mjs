/**
 * 验证物品选择弹窗（ItemPickerModal）在把动画逻辑抽成共享 composable
 * 之后没有回归——这是这个项目里调得最细的一段动画，值得单独跑一遍确认。
 *
 * 跑法：node scripts/picker-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5198;
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

// 手动模式默认就在，找一个会打开 ItemPickerModal 的按钮——物品选择按钮
const openBtn = page.locator("button", { hasText: "选择…" }).first();
console.log("找到选择按钮:", await openBtn.count());
const before = await openBtn.boundingBox();
await openBtn.click();
await sleep(50);

const midway = await page.evaluate(() => {
  const card = document.querySelector(".picker-card");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  return { w: Math.round(r.width), h: Math.round(r.height), left: Math.round(r.left) };
});
console.log("展开中途（~50ms）卡片尺寸:", midway, "起点按钮:", before && { w: Math.round(before.width), h: Math.round(before.height) });

await sleep(600);
const settled = await page.evaluate(() => {
  const card = document.querySelector(".picker-card");
  const overlay = document.querySelector(".picker-overlay");
  if (!card) return null;
  const r = card.getBoundingClientRect();
  return {
    w: Math.round(r.width),
    h: Math.round(r.height),
    hasGlassOff: card.dataset.glassOff === "1",
    overlayExists: !!overlay,
    itemsVisible: document.querySelectorAll(".picker-grid button").length,
  };
});
console.log("展开完成:", settled);

// 挑第一项按钮，检查它没有超出/裁切——测行高问题
const rowBox = await page.evaluate(() => {
  const btn = document.querySelector(".picker-grid button");
  if (!btn) return null;
  const name = btn.querySelector(".picker-name");
  const small = btn.querySelector("small");
  const bBox = btn.getBoundingClientRect();
  const nBox = name?.getBoundingClientRect();
  const sBox = small?.getBoundingClientRect();
  return {
    button: { top: bBox.top, bottom: bBox.bottom, h: bBox.height },
    name: nBox && { top: nBox.top, bottom: nBox.bottom },
    small: sBox && { top: sBox.top, bottom: sBox.bottom },
    nameOverflows: nBox && (nBox.top < bBox.top - 0.5 || nBox.bottom > bBox.bottom + 0.5),
    smallOverflows: sBox && (sBox.top < bBox.top - 0.5 || sBox.bottom > bBox.bottom + 0.5),
  };
});
console.log("首项行内容是否超出按钮:", rowBox);

// 先用关闭按钮试一次（更直接，排除 Escape 键盘事件路由问题）
console.log("关闭前 overlay 数量:", await page.locator(".picker-overlay").count());
await page.locator(".picker-close").click();
for (let i = 0; i < 10; i++) {
  await sleep(150);
  const n = await page.locator(".picker-overlay").count();
  console.log(`  +${(i + 1) * 150}ms overlay 数量=${n}`);
}
const closed = await page.evaluate(() => ({
  overlayGone: !document.querySelector(".picker-overlay"),
  originVisible: (() => {
    const btns = [...document.querySelectorAll("button")].filter((b) => b.textContent?.trim() === "选择…");
    return btns.every((b) => getComputedStyle(b).visibility !== "hidden");
  })(),
}));
console.log("收回完成:", closed);

console.log("\n控制台错误:", errors.length ? errors.slice(0, 10) : "（无）");
await browser.close();
vite.kill();
process.exit(errors.length ? 1 : 0);
