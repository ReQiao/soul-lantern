# Java 1.21.5 / 1.21.6~1.21.8 / 1.21.9~1.21.10 / 26.1 / 26.2 / 26.3 give 语法

## 来源与验证

- 验证方式：本仓库 `scripts/mc-verifier`，用 Mojang 官方 server.jar + RCON 实测。
- 验证版本：1.21.5、1.21.6、1.21.9、26.1、26.2（全部 server 实证，PASS=21 FAIL=0）。
- 26.1 / 26.2 / 26.3 服务器需 Java 25+ 运行，已用 JDK 25 实测确认与 modern 完全一致。
- 验证日期：2026-06。
- 原始结果：`scripts/mc-verifier/results/1.21.5/`、`results/1.21.6/`、`results/1.21.9/`、
  `results/26.1/`、`results/26.2/`。

## 版本族归属

1.21.5 / 1.21.6~1.21.8 / 1.21.9~1.21.10 / 26.1 / 26.2 / 26.3 均属于 **modern 族**，
与 1.21.11 完全一致（PASS=21 FAIL=0）：

builder 路由：默认 `buildModernFamily(form, MODERN_PROFILE)`。

## 与 1.21.11 对比

服务器实证结果：五个代表版本（1.21.5、1.21.6、1.21.9、26.1、26.2）的每条探针结果与
1.21.11 完全相同。包括：
- 文本：直接 JSON 数组
- enchantments：扁平形式
- attribute_modifiers：数组形式，type 不带引号
- can_place_on / can_break：直接引号列表 `[{blocks:"..."}]`
- 支持 consumable / glider / death_protection
- 支持 tooltip_display

## 版本边界

经验证，mid→modern 的语法切换发生在 **1.21.4 → 1.21.5** 之间：
- 1.21.4：mid 族（SNBT 文本、predicates 包装、无 tooltip）
- 1.21.5：modern 族（直接 JSON、直接列表、支持 tooltip）

## 对应测试

- 单元/快照：`src/logic/builder.test.mjs`（用例 26–29）。
- 服务器回归：`npm run verify-syntax -- 1.21.5 1.21.6 1.21.9 26.1 26.2`。

## AI 指令的独立版本边界

give 的版本族不能用来判断所有指令的数据格式：

| 变化 | 起始版本 |
|---|---|
| 实体属性改为 attributes / id / base，modifier 改用资源 ID | 1.21 |
| 属性 ID 去除 generic. / player. / zombie. 前缀 | 1.21.2 |
| 实体装备改为 equipment，文本改为原生 SNBT | 1.21.5 |
| BlockState 的 Name / Properties 改为 id / properties | 26.3 |

原始 execute.run、选择器和 NBT 字符串仍需针对目标版本验证。语法接受也不表示所有字段生效，私有服务端工作流会读回血量、名称、装备及容器附魔，检查实际效果。
