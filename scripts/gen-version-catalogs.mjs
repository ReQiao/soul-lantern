// 逐个运行官方数据生成器；只在全部成功后更新目录，避免留下半份数据。
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { filterExperimentalCatalog } from "./filter-experimental-catalog.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const versions = [
  ["java_1_20_5", "1.20.5"], ["java_1_21", "1.21"], ["java_1_21_1", "1.21.1"],
  ["java_1_21_2", "1.21.2"], ["java_1_21_3", "1.21.3"], ["java_1_21_4", "1.21.4"],
  ["java_1_21_5", "1.21.5"], ["java_1_21_6", "1.21.6"], ["java_1_21_9", "1.21.9"],
  ["java_1_21_11_plus", "1.21.11"], ["java_26_1", "26.1"],
  ["java_26_2_plus", "26.2"], ["java_26_3_plus", "26.3"],
];

const scratch = mkdtempSync(path.join(tmpdir(), "soul-catalog-"));
const catalogs = {};
for (const [key, version] of versions) {
  const output = path.join(scratch, `${version}.ts`);
  const snapshot = path.join(scratch, `${version}.json`);
  for (let attempt = 1; ; attempt++) {
    try {
      execFileSync(process.execPath, [path.join(root, "scripts/gen-catalog.mjs"), version,
        "--output", output, "--snapshot-output", snapshot], { stdio: "inherit" });
      break;
    } catch (error) {
      if (attempt === 3) throw error;
      console.warn(`${version} 生成失败，稍后重试 (${attempt}/3)`);
      await new Promise(resolve => setTimeout(resolve, 3000));
    }
  }
  catalogs[key] = JSON.parse(readFileSync(snapshot, "utf8"));
  if (!catalogs[key].items.length || !catalogs[key].blocks.length) {
    throw new Error(`${version} 的注册表为空，拒绝覆盖现有目录`);
  }
}
writeFileSync(path.join(root, "src/data/catalog-versions.generated.json"), JSON.stringify(catalogs));
writeFileSync(path.join(root, "src/data/items.generated.ts"), readFileSync(path.join(scratch, "26.3.ts")));
await filterExperimentalCatalog(path.join(root, "src/data/catalog-versions.generated.json"), process.env.SOUL_LANTERN_MC_CACHE || path.join(root, "scripts/mc-verifier/cache"));
console.log("已更新各版本目录和 26.3 最新目录");
