<script setup lang="ts">
/**
 * 「管理员动了你的余额」通知弹窗。
 *
 * 走 EULA 那一路的形态（居中卡片 + 遮罩 + 必须点掉），不做成 toast——
 * 这是别人对你的账户做的事，飘一下就消失的提示很容易被错过，而错过了就
 * 再也看不到了：服务端那批通知是**一次性**的（`/v1/auth/me` 读到即清空），
 * 客户端也不会二次持久化。
 *
 * 多条通知排队逐个弹（管理员可能连着调了几笔），点"知道啦"出队一条。
 */
import { computed } from "vue";
import type { BalanceNotice } from "../logic/auth";

const props = defineProps<{ notices: BalanceNotice[] }>();
const emit = defineEmits<{ dismiss: [] }>();

/** 队首那条。父组件弹一条出队一条。 */
const current = computed<BalanceNotice | undefined>(() => props.notices[0]);
const isGain = computed(() => (current.value?.delta ?? 0) > 0);
/** 界面上一律显示绝对值——正负由文案和颜色表达，再带个负号就成了"扣了 -200"。 */
const amount = computed(() => Math.abs(current.value?.delta ?? 0));
</script>

<template>
  <Teleport to="body">
    <Transition name="modal-fade">
      <!-- 不给点遮罩关闭：这条消息只会出现一次，误触关掉就没了。
           必须点那个按钮。 -->
      <div v-if="current" class="modal-overlay notice-overlay">
        <div class="modal-card notice-card" :class="isGain ? 'notice-gain' : 'notice-loss'">
          <div class="notice-face">{{ isGain ? "╰(*°▽°*)╯" : "o(╥﹏╥)o" }}</div>

          <p v-if="isGain" class="notice-line">
            🎉管理员向你发送了 <strong>{{ amount }}</strong> 灵魂币🎉
          </p>
          <p v-else class="notice-line">
            管理员给你余额扣了 <strong>{{ amount }}</strong> 灵魂币😭
          </p>

          <p class="notice-sub">{{ isGain ? "快去看看 AI 模式吧！" : "回家种地去吧" }}</p>

          <p class="notice-balance">当前余额：{{ current.balanceAfter }} 灵魂币</p>

          <!-- 还有几条在排队时说一声，否则用户点一下又冒一个会以为界面卡了 -->
          <p v-if="notices.length > 1" class="notice-more">
            还有 {{ notices.length - 1 }} 条
          </p>

          <button class="primary-btn notice-ok" type="button" @click="emit('dismiss')">
            知道啦
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
