<script setup lang="ts">
/**
 * 数据包模式 —— 和「手动模式」「AI 模式」「万灯集」并列。
 *
 * 现在还是空壳：只占住入口和页面骨架，具体功能（进度、战利品表、配方、函数等）
 * 后面再往里填。先上入口是为了让用户知道这件事在做，也方便收集想要哪些功能的反馈。
 */
import InfoTip from "./InfoTip.vue";

const PLANNED = [
  { name: "函数", desc: "把多条指令打包成一个 .mcfunction，一键调用" },
  { name: "进度", desc: "自定义成就：达成条件、图标、奖励" },
  { name: "战利品表", desc: "改箱子、生物掉落、钓鱼的产出" },
  { name: "配方", desc: "自定义合成、熔炼、锻造配方" },
];
</script>

<template>
  <section class="card datapack-card">
    <div class="datapack-head">
      <h2>
        数据包
        <InfoTip text="用表单生成原版数据包里的各类文件，打包后放进存档就能用。正在开发中。" />
      </h2>
    </div>

    <div class="datapack-empty">
      <p class="datapack-soon">正在开发中，敬请期待</p>
      <p class="datapack-hint">计划支持：</p>
      <ul class="datapack-planned">
        <li v-for="p in PLANNED" :key="p.name">
          <strong>{{ p.name }}</strong>
          <span>{{ p.desc }}</span>
        </li>
      </ul>
      <p class="datapack-hint">想要哪个功能先做，欢迎在「设置 → 反馈问题」里告诉我们。</p>
    </div>
  </section>
</template>
