/**
 * 共享的"从按钮里飞出来"弹窗动画。
 *
 * 抽自 ItemPickerModal——那套动画是这个项目里调得最细的一段（真机踩过好几次
 * 坑，见下面各函数注释），值得所有"从一个按钮点出来的弹窗"都用它，而不是
 * 各自再淡入淡出一次。要求只有一条：过渡元素内部得有这几个节点（class 名
 * 是硬约定，不是巧合）：
 *
 *   .picker-scrim         —— 遮罩，独立于过渡元素本身（见 onEnter 里的注释）
 *   .modal-card 的具体那块 —— 弹窗本体，会被临时接管 position/size/radius
 *   .picker-brand          —— 品牌色覆盖层："展开的第一段里，弹窗看起来
 *                              还是那颗按钮"，铺满整块卡片
 *   .picker-brand-label    —— 品牌色覆盖层里的文字
 *   .picker-inner          —— 内容层，变形期间尺寸被钉死，只负责透明度
 *
 * 没有 `origin`（没有一个具体按钮可以"飞出来"）、或者 `animate` 为 false、
 * 或者系统要求减弱动效时，自动退化成一个 140~160ms 的纯淡入淡出——这不是
 * "动画没生效"，是刻意的降级路径，跟 ItemPickerModal 原来的行为一致。
 */

export interface MorphPopupHandle {
  card: HTMLElement;
  inner: HTMLElement;
  brand: HTMLElement;
  brandLabel: HTMLElement;
  scrim: HTMLElement;
}

function partsOf(el: Element): Partial<MorphPopupHandle> {
  return {
    card: el.querySelector<HTMLElement>(".picker-card") ?? undefined,
    inner: el.querySelector<HTMLElement>(".picker-inner") ?? undefined,
    brand: el.querySelector<HTMLElement>(".picker-brand") ?? undefined,
    brandLabel: el.querySelector<HTMLElement>(".picker-brand-label") ?? undefined,
    scrim: el.querySelector<HTMLElement>(".picker-scrim") ?? undefined,
  };
}

function boxOf(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}
type Box = ReturnType<typeof boxOf>;

function boxStyle(box: Box, radius: number) {
  return {
    left: `${box.x}px`,
    top: `${box.y}px`,
    width: `${box.w}px`,
    height: `${box.h}px`,
    borderRadius: `${radius}px`,
  };
}

/** 以 centerOn 为中心，放一个 w×h 大小的框——用来算"移到中点时保持原尺寸"的过渡框 */
function centeredBox(w: number, h: number, centerOn: Box): Box {
  return { x: centerOn.x + centerOn.w / 2 - w / 2, y: centerOn.y + centerOn.h / 2 - h / 2, w, h };
}

/** 以 box 为中心整体放大/缩小 factor 倍——回弹动画靠这个和主尺寸补间接在一起，不单开一段 */
function scaledBox(box: Box, factor: number): Box {
  const w = box.w * factor;
  const h = box.h * factor;
  return { x: box.x + box.w / 2 - w / 2, y: box.y + box.h / 2 - h / 2, w, h };
}

/**
 * 把弹窗打扮成"就是那颗按钮"。抄的是 computed style 而不是写死一份颜色：
 * 按钮的皮肤以后在 CSS 里怎么改，这层跟着变，不会哪天悄悄对不上。
 */
function paintBrand(brand: HTMLElement, label: HTMLElement, origin: HTMLElement) {
  const cs = getComputedStyle(origin);
  brand.style.background = cs.backgroundImage === "none" ? cs.backgroundColor : cs.backgroundImage;
  label.style.color = cs.color;
  label.style.font = cs.font;
  label.textContent = (origin.textContent ?? "").trim();
}

/**
 * 变形期间把这块玻璃摘掉。backdrop-filter 的位移图是按元素当前尺寸生成的，
 * 而这套动画每一帧都在改 width/height——不摘的话 ResizeObserver 会跟着每帧
 * 重新生成一整张位移图再写回内联样式，代价刚好落在最需要流畅的那几百毫秒上。
 */
function glassOff(card: HTMLElement) {
  card.dataset.glassOff = "1";
}
function glassOn(card: HTMLElement) {
  delete card.dataset.glassOff;
}

/**
 * 把内容层的尺寸钉成定值。变形动画逐帧改的是卡片的 width/height，而内容层
 * 是 100%×100%——不钉住的话，里面的内容每一帧都要重新布局一次，实测能把
 * 收回动画拖到个位数帧。钉成最终尺寸之后，内容层在整段变形里一动不动，
 * 卡片外框缩到多小都只是把它裁掉。
 */
function pinInner(inner: HTMLElement, box: Box) {
  inner.style.width = `${box.w}px`;
  inner.style.height = `${box.h}px`;
}
function unpinInner(inner: HTMLElement) {
  inner.style.width = "";
  inner.style.height = "";
}

function resetCardBox(card: HTMLElement) {
  card.style.position = "";
  card.style.margin = "";
  card.style.overflow = "";
  card.style.left = "";
  card.style.top = "";
  card.style.width = "";
  card.style.height = "";
  card.style.borderRadius = "";
}

export interface UseMorphPopupOptions {
  /** 触发这个弹窗的按钮元素。拿不到就退化成纯淡入淡出。 */
  getOrigin: () => HTMLElement | null | undefined;
  /** 同 AiPanel 的 :animate 约定：关了界面动画时跳过展开/收回动效。 */
  getAnimate: () => boolean;
}

export function useMorphPopup({ getOrigin, getAnimate }: UseMorphPopupOptions) {
  const prefersReducedMotion =
    typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  function onEnter(el: Element, done: () => void) {
    const { card, inner, brand, brandLabel, scrim } = partsOf(el);
    const originEl = getOrigin();
    const skip =
      getAnimate() === false ||
      prefersReducedMotion ||
      !card ||
      !inner ||
      !originEl ||
      !brand ||
      !brandLabel ||
      !scrim;
    if (skip) {
      if (scrim) scrim.style.opacity = "1";
      const anim = (el as HTMLElement).animate([{ opacity: 0 }, { opacity: 1 }], {
        duration: prefersReducedMotion ? 1 : 160,
        easing: "ease",
      });
      anim.onfinish = done;
      return;
    }

    const from = boxOf(originEl);
    const to = boxOf(card);
    const travelPoint = centeredBox(from.w, from.h, to);
    // 先量再改：一旦把卡片改成按钮大小，内容层的尺寸就跟着塌了，量不到目标值。
    pinInner(inner, boxOf(inner));

    // 真按钮要藏起来：弹窗这时正顶着它的皮和它的字站在同一个位置，
    // 底下再露出一个一模一样的按钮就穿帮了。
    originEl.style.visibility = "hidden";
    glassOff(card);
    card.style.position = "fixed";
    card.style.margin = "0";
    card.style.overflow = "hidden";
    Object.assign(card.style, boxStyle(from, 16));
    inner.style.opacity = "0";
    paintBrand(brand, brandLabel, originEl);
    brand.style.opacity = "1";

    const duration = 480;
    const TRAVEL = 0.34;
    const UNFURL = 0.74;
    const OVERSHOOT = 0.84;

    const boxAnim = card.animate(
      [
        { ...boxStyle(from, 16), offset: 0, easing: "cubic-bezier(0.4,0.05,0.35,1)" },
        { ...boxStyle(travelPoint, 16), offset: TRAVEL, easing: "cubic-bezier(0.25,0.6,0.35,1)" },
        { ...boxStyle(to, 26), offset: UNFURL, easing: "ease-out" },
        { ...boxStyle(scaledBox(to, 1.03), 27), offset: OVERSHOOT, easing: "ease-in-out" },
        { ...boxStyle(scaledBox(to, 0.994), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.4 },
        { ...boxStyle(scaledBox(to, 1.003), 26), offset: OVERSHOOT + (1 - OVERSHOOT) * 0.68 },
        { ...boxStyle(to, 26), offset: 1 },
      ],
      { duration, easing: "linear", fill: "forwards" },
    );

    brandLabel.animate(
      [
        { opacity: 1, offset: 0 },
        { opacity: 0, offset: TRAVEL * 0.35 },
        { opacity: 0, offset: 1 },
      ],
      { duration, easing: "ease-out", fill: "forwards" },
    );
    brand.animate(
      [
        { opacity: 1, offset: 0 },
        { opacity: 0.05, offset: TRAVEL * 0.55 },
        { opacity: 0, offset: 1 },
      ],
      { duration, easing: "ease-out", fill: "forwards" },
    );

    inner.animate(
      [
        { opacity: 0, offset: 0 },
        { opacity: 0, offset: TRAVEL },
        { opacity: 1, offset: UNFURL },
        { opacity: 1, offset: 1 },
      ],
      { duration, easing: "ease-out", fill: "forwards" },
    );

    const scrimAnim = scrim.animate(
      [
        { opacity: 0, offset: 0 },
        { opacity: 0, offset: TRAVEL },
        { opacity: 1, offset: UNFURL },
        { opacity: 1, offset: 1 },
      ],
      { duration, fill: "forwards" },
    );

    boxAnim.onfinish = () => {
      boxAnim.cancel();
      scrimAnim.cancel();
      resetCardBox(card);
      unpinInner(inner);
      inner.style.opacity = "";
      brand.style.opacity = "0";
      scrim.style.opacity = "1";
      glassOn(card);
      done();
    };
  }

  function onLeave(el: Element, done: () => void) {
    const { card, inner, brand, brandLabel, scrim } = partsOf(el);
    const originEl = getOrigin();
    const skip =
      getAnimate() === false ||
      prefersReducedMotion ||
      !card ||
      !inner ||
      !originEl ||
      !brand ||
      !brandLabel ||
      !scrim;
    if (skip) {
      // 兜底也要把按钮放出来：展开时动画还开着（按钮被藏起来了），关的时候
      // 用户刚好把「启用界面动画」关掉——这条路径不还原的话按钮就永久消失了。
      if (originEl) originEl.style.visibility = "";
      const anim = (el as HTMLElement).animate([{ opacity: 1 }, { opacity: 0 }], {
        duration: prefersReducedMotion ? 1 : 140,
        easing: "ease",
      });
      anim.onfinish = done;
      return;
    }

    const from = boxOf(card);
    const to = boxOf(originEl);
    const shrinkPoint = centeredBox(to.w, to.h, from);
    pinInner(inner, boxOf(inner));

    glassOff(card);
    card.style.position = "fixed";
    card.style.margin = "0";
    card.style.overflow = "hidden";
    Object.assign(card.style, boxStyle(from, 26));
    paintBrand(brand, brandLabel, originEl);
    brand.style.opacity = "0";

    const duration = 430;
    const SHRINK = 0.34;

    const boxAnim = card.animate(
      [
        { ...boxStyle(from, 26), offset: 0, easing: "cubic-bezier(0.4,0,0.5,1)" },
        { ...boxStyle(shrinkPoint, 16), offset: SHRINK, easing: "cubic-bezier(0.3,0,0.2,1)" },
        { ...boxStyle(to, 16), offset: 1 },
      ],
      { duration, easing: "linear", fill: "forwards" },
    );

    brand.animate(
      [
        { opacity: 0, offset: 0 },
        { opacity: 0.95, offset: SHRINK },
        { opacity: 1, offset: 1 },
      ],
      { duration, easing: "ease-in", fill: "forwards" },
    );
    brandLabel.animate(
      [
        { opacity: 0, offset: 0 },
        { opacity: 0, offset: SHRINK * 0.5 },
        { opacity: 1, offset: SHRINK },
        { opacity: 1, offset: 1 },
      ],
      { duration, easing: "ease-out", fill: "forwards" },
    );

    inner.animate(
      [
        { opacity: 1, offset: 0 },
        { opacity: 0, offset: SHRINK * 0.6 },
        { opacity: 0, offset: 1 },
      ],
      { duration, easing: "ease-in", fill: "forwards" },
    );

    scrim.animate(
      [
        { opacity: 1, offset: 0 },
        { opacity: 0, offset: SHRINK * 0.6 },
        { opacity: 0, offset: 1 },
      ],
      { duration, fill: "forwards" },
    );

    boxAnim.onfinish = () => {
      originEl.style.visibility = "";
      unpinInner(inner);
      brand.style.opacity = "0";
      glassOn(card);
      done();
    };
  }

  return { onEnter, onLeave };
}
