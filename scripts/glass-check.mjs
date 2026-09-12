/**
 * 在真浏览器里量一遍这次玻璃改动的影响。
 *
 * 这个脚本是**临时验证工具**，不进 npm test：它要起 vite、开无头浏览器，
 * 跑一次十几秒，而且 sandbox 里是软件渲染，绝对帧率不可信——只有"改动前后
 * 的相对差"有意义。
 *
 * 跑法：node scripts/glass-check.mjs
 */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 5199;

const vite = spawn("npx", ["vite", "--port", String(PORT), "--strictPort"], {
  stdio: "ignore",
  detached: false,
});

process.on("exit", () => vite.kill());

async function waitForServer() {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/`);
      if (r.ok) return;
    } catch {
      // 还没起来
    }
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
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});

await page.goto(`http://127.0.0.1:${PORT}/`, { waitUntil: "networkidle" });

// EULA 门禁：滚到底再同意，否则进不去主界面
await page.evaluate(() => {
  const el = document.querySelector(".eula-text");
  if (el) el.scrollTop = el.scrollHeight;
});
await sleep(300);
const agree = page.locator(".eula-box button.primary-btn");
if (await agree.count()) {
  await agree.first().click({ force: true }).catch(() => {});
}
await sleep(1200);

// 【必须在任何滑杆操作之前测】app-shell 的真实默认底色，免得被后面的拖拽测试污染
const shellDefault0 = await page.evaluate(
  () => getComputedStyle(document.querySelector(".app-shell")).backgroundImage,
);
console.log("app-shell 真实默认底色（未动过滑杆）:", shellDefault0);
console.log(
  "  与改造前一致（0.36/0.47）:",
  shellDefault0.includes("0.36") && shellDefault0.includes("0.47") ? "是" : "否 ← 外观被改动了",
);

console.log("--- 主界面 ---");
console.log("模式切换按钮:", await page.locator(".mode-switch button").allTextContents());
console.log("撤销/重做/重置:", await page.locator(".top-form button").allTextContents());
console.log("clarity 滑杆:", await page.locator(".clarity-slider").count());

// --clarity 有没有真的写到 :root 上
const clarityVar = await page.evaluate(() =>
  getComputedStyle(document.documentElement).getPropertyValue("--clarity").trim(),
);
console.log("--clarity =", clarityVar || "(空)");

// 打开模板库，看看玻璃有没有装上、阴影变量有没有按尺寸算出来
await page.locator(".top-form button", { hasText: "模板库" }).click();
await sleep(700);
const glass = await page.evaluate(() => {
  const el = document.querySelector(".tpl-card");
  if (!el) return null;
  const inline = el.style;
  const r = el.getBoundingClientRect();
  return {
    w: Math.round(r.width),
    h: Math.round(r.height),
    hasLens: (inline.backdropFilter || "").includes("url("),
    hasContrast: (inline.backdropFilter || "").includes("contrast("),
    liftY: inline.getPropertyValue("--glass-lift-y"),
    liftBlur: inline.getPropertyValue("--glass-lift-blur"),
  };
});
console.log("模板库弹窗:", glass);

// 默认 clarity（0.55）下弹窗底色必须**和改造前逐字一致**——这次把硬编码的
// 0.17/0.13 换成了跟着 clarity 走的 calc，系数就是照着还原这两个数取的。
const defaultTint = await page.evaluate(
  () => getComputedStyle(document.querySelector(".tpl-card")).backgroundImage,
);
console.log("默认底色:", defaultTint);
console.log(
  "  与改造前一致:",
  defaultTint.includes("0.17") && defaultTint.includes("0.13") ? "是" : "否 ← 外观被改动了",
);

// clarity 拖一下，确认已经装好的面板真的跟着重算了（滤镜串必须变）
const clarityEffect = await page.evaluate(async () => {
  const el = document.querySelector(".tpl-card");
  const before = el.style.backdropFilter;
  const slider = document.querySelector(".clarity-slider");
  slider.value = "0";
  slider.dispatchEvent(new Event("input", { bubbles: true }));
  slider.dispatchEvent(new Event("change", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 500));
  const after = el.style.backdropFilter;
  return {
    changed: before !== after,
    rootVar: getComputedStyle(document.documentElement).getPropertyValue("--clarity").trim(),
    // clarity=0 时染色应该压到最实的一端
    // 自定义属性 getPropertyValue 拿到的是没解析的 token 串，看不出实际数值。
    // 要验"染色真的变了"，得去读一个**真正消费**它的元素的 computed background。
    tintResolved: getComputedStyle(document.querySelector(".tpl-card")).backgroundImage,
  };
});
console.log("拖 clarity 到 0:", clarityEffect);

// .app-shell 才是用户几乎全程看得见的那片玻璃，之前滑杆完全碰不到它
const shellDefault = shellDefault0;

const shellAtZero = await page.evaluate(async () => {
  const slider = document.querySelector(".clarity-slider");
  slider.value = "0";
  slider.dispatchEvent(new Event("input", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 200));
  return getComputedStyle(document.querySelector(".app-shell")).backgroundImage;
});
console.log("拖到 0 之后 app-shell 底色:", shellAtZero);
console.log("  真的变了:", shellAtZero !== shellDefault ? "是" : "否 ← 滑杆还是没接上主界面");

const shellAtOne = await page.evaluate(async () => {
  const slider = document.querySelector(".clarity-slider");
  slider.value = "1";
  slider.dispatchEvent(new Event("input", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 200));
  return getComputedStyle(document.querySelector(".app-shell")).backgroundImage;
});
console.log("拖到 1 之后 app-shell 底色:", shellAtOne);

// 换一个小得多的浮层，验证"厚度随尺寸变"确实产生了不同的值
await page.keyboard.press("Escape");
await page.evaluate(() => {
  document.querySelector(".modal-overlay")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
});
await sleep(400);

// 帧率：连续 2 秒记录 rAF 间隔
async function fps(label) {
  const r = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const frames = [];
        let last = performance.now();
        const t0 = last;
        function tick(now) {
          frames.push(now - last);
          last = now;
          if (now - t0 < 2000) requestAnimationFrame(tick);
          else {
            frames.sort((a, b) => a - b);
            resolve({
              fps: Math.round(1000 / (frames.reduce((s, x) => s + x, 0) / frames.length)),
              worst: Math.round(frames[frames.length - 1]),
            });
          }
        }
        requestAnimationFrame(tick);
      }),
  );
  console.log(`${label}: ${r.fps} fps, 最长一帧 ${r.worst}ms`);
  return r;
}

console.log("\n--- 帧率（sandbox 是软件渲染，只看相对差）---");
await fps("静止");
// 万灯集现在是模式切换栏里的一个模式，不是弹窗——按钮挪到了 .mode-switch。
await page.locator(".mode-switch button", { hasText: "万灯集" }).click();
await sleep(900);
await fps("万灯集页面打开");

console.log("\n--- 控制台错误 ---");
console.log(errors.length ? errors.slice(0, 10) : "（无）");

await browser.close();
vite.kill();
process.exit(errors.length ? 1 : 0);
