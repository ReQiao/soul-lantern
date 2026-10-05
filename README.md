# Soul Lantern（灵魂灯笼）

一个面向 Minecraft 的指令生成器，支持 **Java 1.20.5 ~ 26.3** 与 **基岩版**。

目标是解决 MC 新手以及指令熟手在编写指令 JSON 和物品组件时步骤繁琐、容易写错的问题：
可以在**手动模式**里一项项点选生成，也可以在 **AI 模式**里用大白话描述想要的效果，
再到**万灯集**里和大家分享、借鉴模板。界面为深蓝液态玻璃风格，全中文。

**下载：[GitHub Releases](https://github.com/ReQiao/soul-lantern/releases)**（当前为 5.0-rc1 测试版）

> 安装包没有数字签名。Windows 弹出「Windows 已保护你的电脑」时，点「更多信息」→「仍要运行」；
> 杀毒软件拦截时请放心允许，本程序没有任何病毒，源代码全部公开在本仓库。

## 功能

### 手动模式

完全在本地运行，不联网、不需要账号。

- 完整的物品、方块、附魔、属性数据库（物品表来自 Mojang 官方数据，中文译名齐全），分类选择 + 搜索
- 显示名称、物品名称、物品描述的富文本编辑：加粗、斜体、下划线、删除线、混淆、字体、文字颜色 / 渐变色、阴影颜色与透明度
- 完整文本组件：内嵌图标 / 头像、悬停 / 点击 / 插入事件、翻译 / 按键 / 选择器 / 计分板 / NBT 组件，按所选版本自动取舍
- 附魔、属性修饰符、可放置 / 可破坏方块限制
- 基础组件、食物、消耗与食用效果、死亡保护与死亡效果、工具规则、custom_data
- 基岩版数据值、物品锁、死亡保留
- 撤销 / 重做 / 重置，自动保存草稿
- 模板：内置模板、JSON 导出 / 导入、从万灯集收藏的模板
- 一键复制指令，一键部署到存档

### AI 模式

用大白话描述想要的效果，比如「做一把能射 TNT 的弓」，自动生成指令并给出思路说明。需要账号。

- 生成的指令由确定性的构建器兜底，AI 编造的物品 / 方块 / 实体 / 附魔 id 会被拦下，并按所选版本校验
- 多轮对话：可以在上一次的结果上继续追问修改（比如「改成用箭」）
- 多个模型可选，按真实用量扣灵魂币，生成前会先按所选模型预估这次最多花多少
- 需要持续生效的效果（比如落地爆炸）会单独列出，只能通过一键部署挂到数据包的循环上
- 测试期间可以「使用自己的 API key」：支持 OpenAI 格式的接口，key 加密保存在本机，不经过我们的服务器，不消耗灵魂币（正式版会关闭）

### 万灯集

大家分享模板的地方，每个人发出去的一份模板就是一盏灯。

- 发布自己的手动模板或 AI 提示词，支持 Markdown 说明
- 浏览、搜索、分类筛选，点赞、收藏、评论，一键载入使用
- 不合适的内容可以举报，多人举报后会先隐藏等待审核

### 一键部署到存档

- 选择存档后，把生成的指令打包成数据包放进存档，进游戏 `/reload` 即可生效
- 循环效果自动挂到 tick 上；会识别存档的实际版本，和当前选择不一致时给出提示
- 数据包命名空间为 `soul_lantern`，每次部署前先整包清空，不会残留旧的循环效果
- **部署前请先备份存档**

### 其它

- 设置页：贡献者计划、反馈入口（可一键复制诊断信息）、版本信息
- 发布新版本后，软件顶部会提示并可一键打开下载页
- 玻璃透明度可调，可关闭界面动画，F11 全屏

## 下载与安装

在 [Releases](https://github.com/ReQiao/soul-lantern/releases) 页面按系统选择：

| 系统 | 文件 |
|---|---|
| Windows（64 位 / 32 位 / ARM64） | `…-windows-x64.exe`（推荐）或 `.msi` |
| macOS（Intel / Apple 芯片） | `…-macos-x64.dmg` / `…-macos-silicon.dmg` |
| Linux（x64 / ARM64） | `.AppImage`、`.deb` 或 `.rpm` |

- Windows 安装时可以选择只为自己安装，还是为这台电脑的所有用户安装。
- 软件依赖微软的 WebView2 组件（Windows 11 和绝大多数 Windows 10 自带）。缺少时安装程序会提示并自动下载安装。
- 4.x 老版本已无法连接新服务器，请安装新版本。

## 账号与灵魂币

手动模式不需要账号。AI 模式和万灯集的发布、互动需要用手机号注册（验证码短信由阿里云发出，短信开头的签名是服务商的名字）。

AI 生成会按真实调用量扣除**灵魂币**。测试期间**暂不开放充值和激活码**，灵魂币可以通过以下方式获得：

- 注册赠送
- 反馈问题：每一条有效反馈都会获赠测试用的灵魂币（反馈时附上用户名）
- 贡献者计划：在「设置」里自愿加入，上报使用数据可获得奖励，随时可以退出

测试版的账号、灵魂币和万灯集作品**可能会在正式版之前清空**。

## 支持的版本

### Java 版

版本选择：Java 1.20.5 / 1.21 / 1.21.1 ~ 1.21.6 / 1.21.9 / 1.21.11+ / 26.1 / 26.2 / 26.3。
生成的是新版物品组件格式，例如：

```mcfunction
give @a minecraft:stone[custom_name=[{"text":"石头","color":"#7aa2ff"}],unbreakable={}] 1
```

手动模式已支持的组件：

```text
custom_name  item_name  lore  rarity  enchantment_glint_override
enchantments  attribute_modifiers  can_place_on  can_break  unbreakable
glider  death_protection  damage  max_damage  max_stack_size  repair_cost
tooltip_display  food  consumable  on_consume_effects  tool  tool.rules
custom_data
```

### 基岩版

主要支持基础 `/give` 格式：

```mcfunction
/give @a cobblestone 1 0 {"minecraft:can_place_on":{"blocks":["stone"]}}
```

已支持：物品 ID、数量、数据值、`can_place_on`、`can_destroy`、`item_lock`、`keep_on_death`。
基岩版暂不支持 Java 版的富文本、属性、食物效果等组件语法；AI 模式暂不支持基岩版。

Minecraft 指令语法会随版本变化，本项目以实测语法为准。如果某个组件在游戏里报错，
请以游戏提示为准，并反馈可复现的正确指令和错误指令。

## 快捷键

| 键 | 作用 |
|---|---|
| `Ctrl+Z` / `Ctrl+Y` | 手动模式撤销 / 重做 |
| `Ctrl+Enter` | AI 模式里直接发起生成 |
| `F11` | 全屏 / 退出全屏 |

## 反馈

- [GitHub Issues](https://github.com/ReQiao/soul-lantern/issues)（推荐）
- [B 站私信](https://space.bilibili.com/3690984732887987/)（打不开 GitHub、又没有加速器时用）

软件里「设置 → 反馈问题」可以一键复制诊断信息（版本、系统、最近的错误），贴进反馈里能帮我们更快找到问题。

---

## 开发

技术栈：**Tauri v2 + Vue 3 + TypeScript**，Rust 负责与服务端通信、一键部署等本地能力。

```powershell
npm install
npm run tauri dev      # 开发运行
npm run tauri build    # 打包，产物在 src-tauri/target/release/bundle/
```

检查与测试：

```powershell
./node_modules/.bin/vue-tsc --noEmit   # 类型检查
npm test                               # 前端测试（指令构建器、版本目录、玻璃、Markdown）
cd src-tauri; cargo test --lib         # Rust 单元测试
node scripts/csp-check.mjs             # 用正式 CSP 跑一遍打包产物，列出被拦截的资源（需先 npm run build）
```

版本号有两套：`package.json` / `Cargo.toml` / `tauri.conf.json` 里是纯数字的内部版本（如 `5.0.0`，服务端按它判断新旧），
`src/logic/version.ts` 和 `src-tauri/installer/hooks.nsh` 里是给人看的显示版本（如 `5.0-rc1`），发版时一起改。

物品、方块、实体和粒子目录按版本读取，来源是官方数据生成器。1.20.5 还通过原版游戏过滤默认关闭的实验物品。切换到旧版时保留草稿中的物品，但生成前会提示不受支持的选择。

更新目录可运行 `node scripts/gen-version-catalogs.mjs`；设置 `SOUL_LANTERN_MC_CACHE` 可以把下载缓存放在其他磁盘。GitHub 的「Minecraft 官方版本目录」和「Minecraft 语法验证」工作流都只手动执行。服务端真实构建输出和数据读回检查在私有仓库运行。

AI 模式依赖的服务端不在本仓库内（见下方「许可与授权」），开发时连不上服务端不影响手动模式。

### 管理员本机调试

登录并完成管理员认证后，在管理页的「本机调试」选择缓存目录、阅读并同意 Minecraft EULA，再开启调试。AI 和手动模式的指令旁会出现「验证指令」。开关仅保留在当前会话内，登出、切换账号和退出程序都会停止 Java；普通用户修改界面不能绕过原生端的管理员会话检查。

首次验证自动准备缺少的 JDK 21、JDK 25 和对应版本的 Mojang 官方服务端；JDK 使用 Azul 官方下载，SHA-256 校验后解压到自有缓存，不修改系统 Java。服务端使用官方 SHA-1 校验。Windows 检测到 D 盘时默认把缓存放到 `D:\soul-lantern-build-cache\soul-lantern-debug`，也可以选择其他目录。已有且有效的文件会复用。

验证在独立临时世界进行，服务端仅监听本机、最多使用 1 GB Java 堆和两个 JVM 工作处理器，首次启动会有额外开销。同版本连续验证复用服务端，切换版本时重新启动；关闭调试后清理临时世界并保留下载缓存。版本分组测试组内最早的正式版，结果显示实际版本。

指令先作为临时函数由官方服务端完整解析，再执行一次；支持长指令。报告区分语法错误和语法通过，附执行反馈与日志。无人在线时 `give @s`、`@p` 等可能缺少玩家；语法通过不代表实现了全部预期效果，循环指令也只执行一次。基岩版不支持此调试功能。

「管理员本机调试验证」GitHub Actions 仅手动执行：云端在 Windows 和 Linux 验证 1.20.5 / Java 21 和 26.3 / Java 25，覆盖正常、错误、长指令、取消、重新开启与临时世界清理，并检查 Windows 原生代码和前端构建。

### 界面出问题时的应急开关

液态玻璃的边缘折射用的是 `backdrop-filter` 引用 SVG 滤镜，只有 Chromium 内核支持；
macOS 版用的是系统 WKWebView，会自动退回纯模糊。万一某台机器上玻璃显示异常，
在开发者工具的控制台里执行一行然后刷新即可：

```js
localStorage.setItem('soul-lantern-glass', 'off')   // 强制纯模糊（和 macOS 看到的一样）
localStorage.setItem('soul-lantern-glass', 'on')    // 强制开折射
localStorage.removeItem('soul-lantern-glass')       // 恢复自动判断
```

另有一个测试开关，用来在开发环境里走通「未登录点 AI 模式」那条分支（只会**多加**一道门禁，不会绕过任何鉴权）：

```js
localStorage.setItem('soul-lantern-gate', 'on')     // 强制显示登录门禁
localStorage.removeItem('soul-lantern-gate')        // 恢复正常
```

## 许可与授权

Copyright (C) 2026 ReQiao

本客户端按 **GNU Affero General Public License v3.0 或更高版本**（AGPL-3.0-or-later）授权，
完整条款见仓库根目录的 [`LICENSE`](./LICENSE)。你可以自由使用、修改和再分发它，
前提是保留版权声明、并以同一许可分发衍生作品。

### AI 模式依赖的服务端不在本仓库内

手动模式完全在本地运行，不联网。AI 模式和万灯集需要连接开发者自行运营的服务端——
**那个服务端是独立作品，不属于本仓库，也不以 AGPL 授权**。本仓库只包含客户端。

### 灵魂币与激活码买的是什么

灵魂币和激活码购买的是**AI 服务的调用额度**（用于支付上游大模型的真实调用成本），
**不是软件许可**。本客户端本身按 AGPL-3.0 免费授权，不出售、也无法出售。
不购买任何额度也可以完整使用手动模式。
