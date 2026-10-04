import { readFileSync, writeFileSync } from 'node:fs';
import { ensureServerJar } from './mc-verifier/mojang.mjs';
import { startServer } from './mc-verifier/server.mjs';
import { RconClient } from './mc-verifier/rcon.mjs';
// 1.20.5 注册表包含默认关闭的 1.21 实验内容，需以正式版实际接受的 ID 为准。
export async function filterExperimentalCatalog(file, cache) {
  const data = JSON.parse(readFileSync(file, 'utf8'));
  const catalog = data.java_1_20_5;
  const jarPath = await ensureServerJar('1.20.5', cache, console.log);
  const server = await startServer({jarPath, version:'1.20.5', log:console.log});
  let rcon;
  try {
    rcon = new RconClient(server.rcon);
    await rcon.connect();
    await rcon.send('forceload add 0 0');
    const kept = [];
    for (const row of catalog.items) {
      const response = await rcon.send(`give @a ${row[0]} 1`);
      if (/No player was found/.test(response)) kept.push(row);
      else if (/Unknown|not enabled|disabled|<--\[HERE\]/i.test(response)) console.log(`移除实验物品 ${row[0]}`);
      else throw new Error(`无法确认物品 ${row[0]}: ${response}`);
    }
    catalog.items = kept;
    const blocks = [];
    for (const row of catalog.blocks) {
      const response = await rcon.send(`setblock 0 100 0 ${row[0]} destroy`);
      if (/Unknown|not enabled|disabled|<--\[HERE\]/i.test(response)) console.log(`移除实验方块 ${row[0]}`);
      else if (/Changed|could not be placed|No blocks were|already/i.test(response)) blocks.push(row);
      else throw new Error(`无法确认方块 ${row[0]}: ${response}`);
    }
    catalog.blocks = blocks;
    const enchants = [];
    for (const id of catalog.enchantments) {
      const response = await rcon.send(`give @a stone[enchantments={levels:{"${id}":1}}] 1`);
      if (/No player was found/.test(response)) enchants.push(id);
      else if (/Unknown|not enabled|disabled|<--\[HERE\]/i.test(response)) console.log(`移除实验附魔 ${id}`);
      else throw new Error(`无法确认附魔 ${id}: ${response}`);
    }
    catalog.enchantments = enchants;
    writeFileSync(file, JSON.stringify(data));
  } finally {
    if (rcon) await rcon.close().catch(()=>{});
    await server.stop();
  }
}
