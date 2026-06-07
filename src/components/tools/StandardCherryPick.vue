<script setup lang="ts">
import { ref, computed } from 'vue'
import { useAppStore } from '../../composables/useAppStore'
import type { LogEntry, ToolId, ToolDef } from '../../types'

const props = defineProps<{
  tabId: string
  tool: ToolDef
}>()

const emit = defineEmits<{
  execute: [tabId: string, toolId: ToolId, inputs: Record<string, string>]
}>()

const store = useAppStore()

// 从 flowTemplate 获取默认值
const commitMessage = ref('')
const targetBranch = ref(store.flowTemplate.cpTargetBranch || '')

const isActive = computed(() => store.activeTab?.activeTool === props.tool.id)

function handleExecute() {
  emit('execute', props.tabId, props.tool.id, {
    commitMessage: commitMessage.value,
    targetBranch: targetBranch.value,
  })
}
</script>

<template>
  <div class="tool-card" :class="{ active: isActive }" @click="store.setActiveTool(tabId, tool.id)">
    <div class="tool-card-header">
      <div class="tool-card-title">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
        </svg>
        {{ tool.title }}
        <span class="badge">{{ tool.version }}</span>
      </div>
      <span class="tool-card-status" :class="tool.status === 'running' ? 'running' : tool.status === 'done' ? 'done' : tool.status === 'error' ? 'error' : 'ready'">
        {{ tool.status === 'ready' ? '待执行' : tool.status === 'running' ? '执行中...' : tool.status === 'done' ? '已完成' : '错误' }}
      </span>
    </div>

    <div class="tool-card-desc">{{ tool.description }}</div>

    <!-- 输入区域（展开时显示） -->
    <div v-if="isActive" class="tool-inputs">
      <div class="input-row">
        <span class="input-label">提交信息</span>
        <el-input
          v-model="commitMessage"
          placeholder="请输入 commit message（留空则只 push 已有 commit）"
          size="small"
          clearable
        />
      </div>
      <div class="input-row">
        <span class="input-label">目标分支</span>
        <el-input
          v-model="targetBranch"
          placeholder="如 dev-3.7"
          size="small"
          clearable
        />
      </div>
      <el-button
        type="primary"
        size="small"
        :loading="tool.status === 'running'"
        :disabled="tool.status === 'running'"
        @click="handleExecute"
      >
        <template #icon>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5,3 19,12 5,21"/></svg>
        </template>
        执行
      </el-button>
    </div>

    <!-- 步骤列表（始终显示） -->
    <ul class="tool-steps">
      <li
        v-for="(step, i) in tool.steps"
        :key="i"
        :class="step.status"
      >
        {{ step.label }}
        <span v-if="step.detail" class="step-detail">{{ step.detail }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.tool-card {
  background: #fff;
  border-radius: 6px;
  padding: 16px;
  border: 1px solid #ebeef5;
  cursor: pointer;
  transition: all 0.2s;
}
.tool-card:hover { border-color: #409eff; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.tool-card.active { border-color: #409eff; box-shadow: 0 0 0 2px rgba(64,158,255,0.2); }

.tool-card-header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 10px;
}
.tool-card-title {
  font-size: 14px; font-weight: 600; color: #303133;
  display: flex; align-items: center; gap: 6px;
}
.tool-card-title svg { flex-shrink: 0; }
.badge {
  font-size: 11px; background: #e6f7ff; color: #1890ff;
  padding: 0 6px; border-radius: 3px;
}
.tool-card-status {
  font-size: 12px; padding: 2px 8px; border-radius: 4px;
}
.tool-card-status.ready { background: #f5f5f5; color: #909399; }
.tool-card-status.running { background: #ecf5ff; color: #409eff; animation: pulse 1.5s infinite; }
.tool-card-status.done { background: #f0f9eb; color: #67c23a; }
.tool-card-status.error { background: #fef0f0; color: #f56c6c; }
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.tool-card-desc { font-size: 13px; color: #909399; line-height: 1.6; margin-bottom: 10px; }

.tool-inputs {
  margin-bottom: 12px;
  display: flex; flex-direction: column; gap: 8px;
}
.input-row {
  display: flex; align-items: center; gap: 8px;
}
.input-label {
  font-size: 12px; color: #606266;
  white-space: nowrap; min-width: 70px;
}

.tool-steps { list-style: none; padding: 0; }
.tool-steps li {
  font-size: 12px; color: #909399;
  padding: 4px 0 4px 16px;
  position: relative; line-height: 1.5;
}
.tool-steps li::before {
  content: "";
  position: absolute; left: 0; top: 10px;
  width: 6px; height: 6px; border-radius: 50%;
  background: #dcdfe6;
}
.tool-steps li.done { color: #67c23a; }
.tool-steps li.done::before { background: #67c23a; }
.tool-steps li.running { color: #409eff; font-weight: 500; }
.tool-steps li.running::before { background: #409eff; box-shadow: 0 0 0 2px rgba(64,158,255,0.3); }
.tool-steps li.error { color: #f56c6c; }
.tool-steps li.error::before { background: #f56c6c; }
.step-detail { display: block; font-size: 11px; color: #909399; }
</style>
