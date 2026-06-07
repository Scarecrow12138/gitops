<script setup lang="ts">
import { ref, computed } from 'vue'
import { useAppStore } from '../../composables/useAppStore'
import type { ToolId, ToolDef } from '../../types'

const props = defineProps<{
  tabId: string
  tool: ToolDef
}>()

const emit = defineEmits<{
  execute: [tabId: string, toolId: ToolId, inputs: Record<string, string>]
}>()

const store = useAppStore()

const commitHash = ref('')
const hotfixBranch = ref(store.gitLabConfig.url ? (store.repos.find(r => r.id === store.activeTab?.repoId)?.alias || 'hotfix') : 'hotfix-licanzhang')

const isActive = computed(() => store.activeTab?.activeTool === props.tool.id)

function handleExecute() {
  emit('execute', props.tabId, props.tool.id, {
    commitHash: commitHash.value,
    hotfixBranch: hotfixBranch.value,
  })
}
</script>

<template>
  <div class="tool-card" :class="{ active: isActive }" @click="store.setActiveTool(tabId, tool.id)">
    <div class="tool-card-header">
      <div class="tool-card-title">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/>
        </svg>
        {{ tool.title }}
        <span class="badge">{{ tool.version }}</span>
      </div>
      <span class="tool-card-status" :class="tool.status === 'running' ? 'running' : tool.status === 'done' ? 'done' : tool.status === 'error' ? 'error' : 'ready'">
        {{ tool.status === 'ready' ? '待执行' : tool.status === 'running' ? '执行中...' : tool.status === 'done' ? '已完成' : '错误' }}
      </span>
    </div>

    <div class="tool-card-desc">{{ tool.description }}</div>

    <div v-if="isActive" class="tool-inputs">
      <div class="input-row">
        <span class="input-label">Commit Hash</span>
        <el-input
          v-model="commitHash"
          placeholder="留空则使用 HEAD（最新提交）"
          size="small"
          clearable
        />
      </div>
      <div class="input-row">
        <span class="input-label">Hotfix 分支</span>
        <el-input
          v-model="hotfixBranch"
          placeholder="如 hotfix-licanzhang"
          size="small"
          clearable
        />
      </div>
      <div class="input-hint">
        源分支：当前所在分支 &nbsp;|&nbsp;
        MR 目标：{{ store.gitLabConfig.url ? (store.flowTemplate.mrTargetBranch || 'main') : '（未配置）' }} &nbsp;|&nbsp;
        Cherry-pick 目标：{{ store.flowTemplate.cpTargetBranch || '（未配置）' }}
      </div>
      <el-button
        type="danger"
        size="small"
        :loading="tool.status === 'running'"
        :disabled="tool.status === 'running'"
        @click="handleExecute"
      >
        <template #icon>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5,3 19,12 5,21"/></svg>
        </template>
        执行 Hotfix
      </el-button>
    </div>

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
  font-size: 11px; background: #fef0f0; color: #f56c6c;
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
  white-space: nowrap; min-width: 85px;
}
.input-hint {
  font-size: 11px; color: #909399;
  padding: 4px 8px; background: #f5f7fa;
  border-radius: 4px; line-height: 1.5;
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
