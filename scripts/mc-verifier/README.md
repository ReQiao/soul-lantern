# Minecraft 指令验证工具

通过临时官方 Minecraft 服务端验证生成结果。仅手动运行，不随提交启动；测试世界与用户世界隔离，服务端内存上限 1GB。各版本顺序执行，下载缓存可通过 `SOUL_LANTERN_MC_CACHE` 指定。

## 验证范围

- `index.mjs`：检查 `/give` 组件的多种候选写法。无人在线时，正确解析的命令返回 `No player was found`。
- `fixtures.mjs`：执行私有服务端构建器导出的真实命令，覆盖 14 种指令类型，并增加属性修饰符、告示牌和装饰物品样例。读回实体属性、装备附魔、容器附魔、名字和告示牌文本，检查解析成功后效果是否正确。
- `datapack-metadata.mjs`：单独编译客户端实际的数据包元数据函数，生成各版本 `pack.mcmeta`，交给原版服务端检查函数是否成功加载。
- `rcon.test.mjs`：轻量协议回归测试，检查认证确认、响应分片、空响应、连接中断和超时。

样例通过意味着这些生成路径得到验证，不能证明 AI 自由输入的所有 SNBT、选择器和 `execute` 子命令都有效。网络、认证、连接或超时错误会让验证失败，不会被当成语法通过。

## 用法

```bash
node scripts/mc-verifier/index.mjs --list
node scripts/mc-verifier/index.mjs 1.21 1.21.2 1.21.5 26.3
node --test scripts/mc-verifier/rcon.test.mjs

# 私有服务端导出 fixtures.json 后
node scripts/mc-verifier/datapack-metadata.mjs fixtures.json pack-metadata.json
node scripts/mc-verifier/fixtures.mjs fixtures.json pack-metadata.json
```

`MC_VERIFY_VERSIONS` 可限制实际测试版本，格式为 `1.21 26.3`。1.20.5～1.21.x 需要 Java 21 或更新版本，26.x 需要 Java 25 或更新版本。元数据工具还需要 Rust，但不构建整个客户端。

公开仓库的 `minecraft-verify.yml` 手动验证前端与 give 探针；私有仓库的 `verify-and-musl.yml` 手动导出和验证真实服务端命令。只有显式选择 `build_musl` 且前面的验证通过，私有流程才生成部署二进制。

## 独立语法边界

`give` 的组件分组不能直接作为所有 AI 指令的版本分组。构建器应按各项语法自己的版本边界判断，提示词与构建器保持一致。

| 特性 | 变化边界 |
|---|---|
| 实体属性 NBT | 1.21 开始使用 `attributes`、`id`、`base`；1.20.5 使用 `Attributes`、`Name`、`Base` |
| 属性修饰符标识 | 1.21 开始使用资源位置 `id`；更早版本使用 UUID 与名称 |
| 属性名称 | 1.21.2 开始移除 `generic.`、`player.`、`zombie.` 前缀 |
| 实体装备、文本组件 | 1.21.5 开始使用 `equipment` 与直接 SNBT 文本组件；更早版本使用旧装备字段与 JSON 字符串 |
| give 附魔组件 | 1.21.5 之前输出规范的 `levels` 包装，之后输出扁平映射；旧版也可能接受简写 |
| give 属性修饰符组件 | 1.21.5 之前使用 `modifiers` 包装，之后使用数组；组件形态与属性名称变化是独立的 |
| 粒子方块状态 | 26.3 使用 `id`、`properties`，此前使用 `Name`、`Properties` |

每次正式版更新：先生成官方目录，再查看变化涉及哪些规则，增加对应样例，执行真实服务端验证。没有变化的规则继续复用。

## 输出与文件

`results/<version>/raw.json` 保存 give 探针响应，`report.json` 和 `report.txt` 聚合候选写法；`builder.json` 保存真实构建器样例、读回结果与服务端日志。探针中的 `builderFamilies` 描述 give 组件分组，不代表其他指令的适配边界。

| 文件 | 职责 |
|---|---|
| `mojang.mjs` | 下载并校验官方服务端，设置网络超时 |
| `server.mjs` | 临时普通世界、RCON、测试数据包与进程清理 |
| `rcon.mjs` | 按请求 ID 接收响应分片，空响应允许，超时和断线报错 |
| `probes.mjs`、`report.mjs` | give 候选写法与结果汇总 |
| `fixtures.mjs` | 真实生成命令及关键效果读回 |

扩展探针时，应让测试物品满足组件约束。例如 `max_damage` 使用不可堆叠物品，避免把物品约束失败误判为语法失败。
