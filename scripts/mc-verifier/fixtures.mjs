// 在临时原版世界中验证服务端构建器输出，并读回关键实体数据。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ensureServerJar } from './mojang.mjs';
import { startServer } from './server.mjs';
import { RconClient } from './rcon.mjs';
const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const cache = process.env.SOUL_LANTERN_MC_CACHE || path.join(here, 'cache');
let failures = 0;
for (const [version, commands] of Object.entries(fixtures)) {
  const results = [];
  let server, rcon;
  try {
    const jarPath = await ensureServerJar(version, cache, console.log);
    server = await startServer({jarPath, version, log: console.log});
    rcon = new RconClient(server.rcon);
    await rcon.connect();
    await rcon.send('forceload add 0 0');
    await rcon.send('gamerule doMobSpawning false');
    const modern = !/^1\.(20|21\.[1-4]$)/.test(version) && version !== '1.21';
    const equipment = modern ? 'equipment:{mainhand:{id:"minecraft:diamond_sword",count:1}}' : 'HandItems:[{id:"minecraft:diamond_sword",count:1},{}]';
    await rcon.send(`summon zombie 0 100 0 {NoAI:1b,Tags:["sl_verify"],${equipment}}`);
    for (const {kind, command} of commands) {
      const response = await rcon.send(command.replace(/^\//, ''));
      const expectedNoPlayer = kind === 'give' && /No player was found/.test(response);
      const okay = !!response && (expectedNoPlayer || !/<--\[HERE\]|Unknown|Expected|Incorrect|Unexpected|Invalid|Failed to|Could not|cannot|can't|Error|No entity was found/i.test(response));
      results.push({kind, command, response, okay});
      if (!okay) failures++;
      console.log(`${version} ${kind}: ${okay ? 'PASS' : 'FAIL'} ${response}`);
    }
    const prefixed = version === '1.20.5' || version === '1.21' || version === '1.21.1';
    const attribute = `minecraft:${prefixed ? 'generic.' : ''}max_health`;
    const response = await rcon.send(`attribute @e[tag=sl_summon,limit=1] ${attribute} base get`);
    const okay = /40(?:\.0)?/.test(response) && !/<--\[HERE\]/.test(response);
    results.push({kind:'summon_health_readback',response,okay});
    if (!okay) failures++;
    const name = await rcon.send('data get entity @e[tag=sl_summon,limit=1] CustomName');
    const nameOkay = /验证/.test(name) && (!modern || !/['"](?:\[)?\{"text"/.test(name));
    results.push({kind:'entity_name_readback',response:name,okay:nameOkay});
    if (!nameOkay) failures++;
    const sign = await rcon.send('data get block 6 100 0 front_text.messages[0]');
    const signOkay = /验证/.test(sign) && (!modern || !/['"](?:\[)?\{"text"/.test(sign));
    results.push({kind:'sign_text_readback',response:sign,okay:signOkay});
    if (!signOkay) failures++;
    const container = await rcon.send('data get block 1 100 0 Items[0].components."minecraft:enchantments"');
    const containerOkay = /sharpness/.test(container) && /3/.test(container) && !/No elements|<--\[HERE\]/.test(container);
    results.push({kind:'container_enchantment_readback',response:container,okay:containerOkay});
    if (!containerOkay) failures++;
    const slot = modern ? 'equipment.mainhand' : 'HandItems[0]';
    const enchant = await rcon.send(`data get entity @e[tag=sl_summon,limit=1] ${slot}.components."minecraft:enchantments"`);
    const enchantOkay = /sharpness/.test(enchant) && /3/.test(enchant) && !/No elements|<--\[HERE\]/.test(enchant);
    results.push({kind:'equipment_enchantment_readback',response:enchant,okay:enchantOkay});
    if (!enchantOkay) failures++;
  } catch (error) {
    results.push({error:String(error),okay:false});
    failures++;
    console.error(version, error);
  } finally {
    if (rcon) await rcon.close().catch(() => {});
    if (server) await server.stop();
    const out = path.join(here,'results',version);
    fs.mkdirSync(out,{recursive:true});
    fs.writeFileSync(path.join(out,'builder.json'),JSON.stringify({version,results},null,2));
  }
}
if (failures) throw new Error(`${failures} 个构建器验证项失败`);
