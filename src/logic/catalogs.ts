import data from '../data/catalog-versions.generated.json';
import { BEDROCK_ITEMS, BEDROCK_BLOCKS, BEDROCK_ENTITIES, ENCHANTS, EFFECTS, ATTRIBUTES, type CatalogRow } from '../data/catalog';
import type { GiveVersion } from './builder';

interface VersionCatalog {
  minecraftVersion: string;
  items: readonly CatalogRow[];
  blocks: readonly CatalogRow[];
  entities: readonly CatalogRow[];
  particles: readonly CatalogRow[];
  enchantments: string[];
  effects: string[];
  effectRows: CatalogRow[];
  attributeRows: CatalogRow[];
  attributes: string[];
}
const versions = data as unknown as Record<Exclude<GiveVersion, 'bedrock'>, VersionCatalog>;
export function getVersionCatalog(version: GiveVersion) {
  return versions[version === 'bedrock' ? 'java_26_3_plus' : version];
}
export function getItemCatalog(version: GiveVersion): readonly CatalogRow[] {
  return version === 'bedrock' ? BEDROCK_ITEMS : getVersionCatalog(version).items;
}
export function getBlockCatalog(version: GiveVersion): readonly CatalogRow[] {
  return version === 'bedrock' ? BEDROCK_BLOCKS : getVersionCatalog(version).blocks;
}
export function getEntityCatalog(version: GiveVersion): readonly CatalogRow[] {
  return version === 'bedrock' ? BEDROCK_ENTITIES : getVersionCatalog(version).entities;
}
export function getParticleCatalog(version: GiveVersion): readonly CatalogRow[] {
  return getVersionCatalog(version).particles;
}
export function getEffectCatalog(version: GiveVersion): readonly CatalogRow[] {
  const catalog = getVersionCatalog(version);
  return (catalog.effectRows || EFFECTS.filter(row => catalog.effects.includes(row[0]))).map(row => EFFECTS.find(known => known[0] === row[0]) || row);
}
export function getEnchantmentCatalog(version: GiveVersion) {
  const ids = new Set(getVersionCatalog(version).enchantments);
  return ENCHANTS.filter(row => ids.has(row[0]));
}
export function getAttributeCatalog(version: GiveVersion): readonly CatalogRow[] {
  const ids = new Set(getVersionCatalog(version).attributes.map(id => id.replace(/minecraft:(generic|player|zombie)\./, 'minecraft:')));
  return (getVersionCatalog(version).attributeRows || ATTRIBUTES.filter(row => ids.has(row[0]))).map(row => ATTRIBUTES.find(known => known[0] === row[0]) || row);
}
// 切换版本时保留原始选择，但阻止已知的高版本原版物品被带入旧版指令。
const knownItems = new Map<string, string>();
for (const catalog of Object.values(versions)) {
  for (const [id, name] of catalog.items) { knownItems.set(id, id); knownItems.set(name, id); }
}
export function unsupportedItemMessage(version: GiveVersion, item: string): string | null {
  if (version === 'bedrock') return null;
  const raw = item.trim();
  const id = knownItems.get(raw) || knownItems.get(raw.includes(':') ? raw : `minecraft:${raw}`);
  if (id && !getItemCatalog(version).some(row => row[0] === id || row[1] === raw)) {
    return `所选物品 ${raw} 不属于 Minecraft ${getVersionCatalog(version).minecraftVersion}，请重新选择物品。`;
  }
  return null;
}
