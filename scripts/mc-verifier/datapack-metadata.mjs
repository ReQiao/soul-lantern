// 直接编译客户端的元数据函数，供原版游戏验证实际输出；不编译 Tauri。
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const source = fs.readFileSync(path.join(root,'src-tauri/src/datapack.rs'),'utf8');
function extract(name) {
  const start = source.indexOf(`fn ${name}(`);
  let depth = 0, end = source.indexOf('{',start);
  do { if (source[end] === '{') depth++; if (source[end] === '}') depth--; end++; } while (depth && end < source.length);
  if (start < 0 || depth) throw new Error(`找不到元数据函数 ${name}`);
  return source.slice(start,end);
}
const scratch = fs.mkdtempSync(path.join(os.tmpdir(),'soul-pack-check-'));
fs.mkdirSync(path.join(scratch,'src'));
fs.writeFileSync(path.join(scratch,'Cargo.toml'),'[package]\nname="soul-pack-check"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nserde_json="1"\n');
const versions = JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const keys = Object.keys(versions).map(v => `("${v}", ${JSON.stringify(v.startsWith('26.') ? `java_${v.replace('.', '_')}${v === '26.1' ? '' : '_plus'}` : v === '1.21' ? 'java_1_21' : v === '1.21.11' ? 'java_1_21_11_plus' : `java_${v.replaceAll('.', '_')}`)})`).join(',');
const program = `${extract('pack_format_for_version')}\n${extract('pack_metadata')}\nfn main() { let mut result = serde_json::Map::new(); for (version,key) in [${keys}] { result.insert(version.into(), pack_metadata(key)); } println!("{}", serde_json::Value::Object(result)); }`;
fs.writeFileSync(path.join(scratch,'src/main.rs'),program);
const output = execFileSync('cargo',['run','--quiet','--manifest-path',path.join(scratch,'Cargo.toml')],{encoding:'utf8'});
fs.writeFileSync(process.argv[3],output);
