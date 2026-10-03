/**
 * 一键部署用的存档列表。手动模式和 AI 模式各有一个部署面板，共用这一份：
 * 在一边选了存档，另一边也跟着变，启动时也只扫一次。
 *
 * 扫到的、选过的存档和上次用的那个由 Rust 那边存在配置目录的 saves.json，
 * 重启软件后打开就能直接选（见 src-tauri/src/datapack.rs）。
 */
import { ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";

export interface SaveInfo {
  name: string;
  path: string;
  /** 版本隔离的实例名（.minecraft/versions/<实例>/saves 里的存档才有）。 */
  instance?: string | null;
}

export const saves = ref<SaveInfo[]>([]);
export const selectedSave = ref("");

/** 重新扫描。返回找到几个存档。 */
export async function refreshSaves(): Promise<number> {
  if (!isTauri()) return 0;
  const r = await invoke<{ saves: SaveInfo[]; last: string | null }>("datapack_list_saves");
  saves.value = r.saves;
  if (!r.saves.some((s) => s.path === selectedSave.value)) {
    const last = r.last && r.saves.some((s) => s.path === r.last) ? r.last : null;
    selectedSave.value = last ?? r.saves[0]?.path ?? "";
  }
  return r.saves.length;
}

let firstLoad: Promise<unknown> | null = null;
/** 面板挂载时调用：整个软件只自动扫一次。 */
export function loadSavesOnce(): Promise<unknown> {
  firstLoad ??= refreshSaves().catch(() => 0);
  return firstLoad;
}

/**
 * 用户手动选了一个存档文件夹。记下来之后重新扫一遍——它所在的 .minecraft
 * 会被加入扫描范围，同一个目录下的其它存档、其它实例也就都出来了。
 * 不是存档（没有 level.dat）会抛错，错误文字可以直接给用户看。
 */
export async function pickSave(path: string): Promise<SaveInfo> {
  const info = await invoke<SaveInfo>("datapack_remember_save", { path });
  await refreshSaves();
  if (!saves.value.some((s) => s.path === info.path)) saves.value = [...saves.value, info];
  selectedSave.value = info.path;
  return info;
}

// 换了存档就记成「上次用的」，下次打开默认选它。
watch(selectedSave, (path) => {
  if (path && isTauri()) invoke("datapack_remember_save", { path }).catch(() => {});
});

/** 下拉框里显示的名字：隔离实例里的存档带上实例名；重名的再带上所在文件夹。 */
export function saveLabel(s: SaveInfo, all: SaveInfo[]): string {
  const base = s.instance ? `${s.name}（${s.instance}）` : s.name;
  const dup = all.filter((o) => (o.instance ? `${o.name}（${o.instance}）` : o.name) === base).length > 1;
  if (!dup) return base;
  const parts = s.path.split(/[\\/]/).filter(Boolean);
  const where = parts.slice(Math.max(0, parts.length - 4), parts.length - 2).join("/");
  return `${base} · ${where}`;
}
