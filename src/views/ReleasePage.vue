<script setup lang="ts">
import { computed, ref } from 'vue'
import { Channel, invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import Terminal from '../components/Terminal.vue'
import type { LogEntry, ReleaseAttempt, ReleaseAttemptPayload, ReleaseTask, ReleaseTaskPayload } from '../types'

type StepStatus = 'pending' | 'running' | 'success' | 'failed'

type ReleaseProgressEvent =
  | { event: 'step', data: { stepIndex: number; status: StepStatus; message: string } }
  | { event: 'log', data: { level: LogEntry['cls']; message: string } }
  | { event: 'retry', data: { stepIndex: number; api: string; retryNo: number; delaySeconds: number; reason: string } }

const task = ref<ReleaseTask | null>(null)
const attempts = ref<ReleaseAttempt[]>([])
const logs = ref<LogEntry[]>([])
const loading = ref(false)
const running = ref(false)

const steps = [
  '获取并校验合并到 prep-3.0 的 MR',
  '合并源分支到 prep-3.0',
  '创建 prep-3.0 到 main 的 MR',
  '备份 main 分支并合并主干',
  '触发 Jenkins 构建',
  '检查 Jenkins 构建结果',
  '完成发布回调',
]

const stepStates = ref<StepStatus[]>(steps.map(() => 'pending'))

const selectedAttempt = ref<ReleaseAttempt | null>(null)

const terminalLogs = computed(() => {
  if (selectedAttempt.value?.logOutput) {
    return selectedAttempt.value.logOutput.split('\n').map((text, index) => ({
      text,
      cls: text.includes('失败') ? 'error' : text.includes('成功') ? 'success' : 'info',
      timestamp: Date.now() + index,
    } satisfies LogEntry))
  }
  return logs.value
})

function pushLog(text: string, cls: LogEntry['cls'] = 'info') {
  logs.value.push({ text, cls, timestamp: Date.now() })
}

function resetStepStates() {
  stepStates.value = steps.map(() => 'pending')
}

function normalizeLogClass(level: string): LogEntry['cls'] {
  if (level === 'success') return 'success'
  if (level === 'warn') return 'warn'
  if (level === 'error') return 'error'
  return 'info'
}

function handleProgressEvent(message: ReleaseProgressEvent) {
  if (message.event === 'step') {
    stepStates.value[message.data.stepIndex] = message.data.status
    pushLog(message.data.message, message.data.status === 'failed' ? 'error' : message.data.status === 'success' ? 'success' : 'info')
    return
  }
  if (message.event === 'retry') {
    stepStates.value[message.data.stepIndex] = 'running'
    pushLog(`${message.data.api} 调用失败，${message.data.delaySeconds} 秒后第 ${message.data.retryNo} 次重试：${message.data.reason}`, 'warn')
    return
  }
  pushLog(message.data.message, normalizeLogClass(message.data.level))
}

function applyPayload(payload: ReleaseTaskPayload | ReleaseAttemptPayload, appendLogs = true) {
  task.value = payload.task ?? null
  attempts.value = payload.attempts
  if (appendLogs) {
    payload.logs.forEach((line) => pushLog(line, line.includes('失败') ? 'error' : 'info'))
  }
}

async function refreshTask() {
  loading.value = true
  selectedAttempt.value = null
  resetStepStates()
  try {
    const payload = await invoke<ReleaseTaskPayload>('refresh_release_task')
    logs.value = []
    applyPayload(payload)
    if (!payload.task) {
      ElMessage.info('暂无待发布任务')
    } else {
      ElMessage.success('发布任务已刷新')
    }
  } catch (e: any) {
    pushLog(String(e), 'error')
    ElMessage.error('刷新发布任务失败: ' + e)
  } finally {
    loading.value = false
  }
}

async function startRelease() {
  if (!task.value || running.value) return
  selectedAttempt.value = null
  logs.value = []
  resetStepStates()

  running.value = true
  try {
    const onEvent = new Channel<ReleaseProgressEvent>()
    onEvent.onmessage = handleProgressEvent
    const payload = await invoke<ReleaseAttemptPayload>('start_release_attempt', {
      taskId: task.value.id,
      onEvent,
    })
    applyPayload(payload, false)
    if (payload.task.status === 'success') {
      ElMessage.success('发布执行成功')
    } else {
      ElMessage.warning('发布执行结束，请查看日志')
    }
  } catch (e: any) {
    pushLog(String(e), 'error')
    ElMessage.error('发布执行失败: ' + e)
  } finally {
    running.value = false
  }
}

function statusTagType(status?: string) {
  if (status === 'success') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'running') return 'warning'
  return 'info'
}

function stepTagType(status: StepStatus) {
  if (status === 'success') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'running') return 'warning'
  return 'info'
}

function stepStatusLabel(status: StepStatus) {
  if (status === 'success') return '完成'
  if (status === 'failed') return '失败'
  if (status === 'running') return '执行中'
  return '待执行'
}

function selectAttempt(attempt: ReleaseAttempt) {
  selectedAttempt.value = attempt
}
</script>

<template>
  <div class="release-page">
    <div class="release-sidebar">
      <div class="side-header">
        <div>
          <h3>发布任务</h3>
          <p>当前待发布记录与执行尝试</p>
        </div>
        <el-button size="small" :loading="loading" @click="refreshTask">刷新</el-button>
      </div>

      <div v-if="task" class="task-card active">
        <div class="card-title">当前待发布任务</div>
        <div class="card-meta">
          <span>服务：{{ task.services.length }}</span>
          <span>已执行：{{ task.attemptsCount }} 次</span>
        </div>
        <div class="card-tags">
          <el-tag size="small" :type="statusTagType(task.status)" effect="plain">{{ task.status }}</el-tag>
          <el-tag size="small" effect="plain">prep-3.0 → main</el-tag>
        </div>
      </div>

      <el-empty v-else description="暂无待发布任务" :image-size="96" />

      <div v-if="attempts.length" class="attempt-list">
        <div class="attempt-title">执行记录</div>
        <div
          v-for="attempt in attempts"
          :key="attempt.id"
          class="attempt-item"
          :class="{ active: selectedAttempt?.id === attempt.id }"
          @click="selectAttempt(attempt)"
        >
          <div>
            <strong>第 {{ attempt.attemptNo }} 次执行</strong>
            <p>{{ attempt.startedAt || '-' }}</p>
          </div>
          <el-tag size="small" :type="statusTagType(attempt.status)" effect="plain">{{ attempt.status }}</el-tag>
        </div>
      </div>
    </div>

    <div class="release-main">
      <div class="main-header">
        <div>
          <h2>发布执行台</h2>
          <p>失败后可基于同一发布任务重跑。</p>
        </div>
        <div class="actions">
          <el-button type="primary" :disabled="!task" :loading="running" @click="startRelease">开始发布</el-button>
        </div>
      </div>

      <div class="metrics">
        <div class="metric">
          <span>待发布服务</span>
          <strong>{{ task?.services.length || 0 }}</strong>
        </div>
        <div class="metric">
          <span>执行次数</span>
          <strong>{{ task?.attemptsCount || 0 }}</strong>
        </div>
      </div>

      <el-tabs>
        <el-tab-pane label="流程">
          <div class="step-list">
            <div v-for="(step, index) in steps" :key="step" class="step-item" :class="stepStates[index]">
              <span class="step-index">{{ index + 1 }}</span>
              <span>{{ step }}</span>
              <el-tag size="small" :type="stepTagType(stepStates[index])" effect="plain">
                {{ stepStatusLabel(stepStates[index]) }}
              </el-tag>
            </div>
          </div>
        </el-tab-pane>
        <el-tab-pane label="服务">
          <el-table :data="task?.services || []" size="small" stripe>
            <el-table-column label="#" type="index" width="60" />
            <el-table-column label="Jenkins Job">
              <template #default="{ row }">{{ row }}</template>
            </el-table-column>
          </el-table>
        </el-tab-pane>
      </el-tabs>
    </div>

    <div class="release-log">
      <Terminal :logs="terminalLogs" title="发布日志" :subtitle="selectedAttempt ? `第 ${selectedAttempt.attemptNo} 次执行` : '当前执行'" />
    </div>
  </div>
</template>

<style scoped>
.release-page {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 330px minmax(420px, 1fr) 420px;
  gap: 12px;
  padding: 12px;
  overflow: hidden;
}

.release-sidebar,
.release-main,
.release-log {
  min-height: 0;
  background: #fff;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  overflow: hidden;
}

.release-sidebar {
  padding: 16px;
  overflow-y: auto;
}

.side-header,
.main-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

h2,
h3 {
  color: #303133;
  font-weight: 600;
}

p {
  margin-top: 4px;
  color: #909399;
  font-size: 12px;
}

.task-card,
.attempt-item {
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  background: #fff;
}

.task-card {
  padding: 12px;
}

.task-card.active {
  border-color: #409eff;
  box-shadow: 0 0 0 2px rgba(64,158,255,0.1);
}

.card-title {
  font-weight: 600;
  margin-bottom: 8px;
}

.card-meta,
.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  color: #606266;
  font-size: 12px;
}

.card-tags {
  margin-top: 10px;
}

.attempt-list {
  margin-top: 16px;
}

.attempt-title {
  margin-bottom: 8px;
  color: #606266;
  font-size: 13px;
  font-weight: 600;
}

.attempt-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px;
  margin-bottom: 8px;
  cursor: pointer;
}

.attempt-item.active {
  border-color: #409eff;
}

.release-main {
  display: flex;
  flex-direction: column;
  padding: 16px;
  overflow-y: auto;
}

.actions {
  display: flex;
  gap: 8px;
}

.metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 12px;
}

.metric {
  min-height: 70px;
  padding: 12px;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
}

.metric span {
  display: block;
  color: #909399;
  font-size: 12px;
  margin-bottom: 8px;
}

.metric strong {
  color: #409eff;
  font-size: 24px;
}

.step-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.step-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 44px;
  padding: 10px;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
}

.step-item > span:nth-child(2) {
  flex: 1;
}

.step-item.running {
  border-color: #e6a23c;
  background: #fdf6ec;
}

.step-item.success {
  border-color: #67c23a;
  background: #f0f9eb;
}

.step-item.failed {
  border-color: #f56c6c;
  background: #fef0f0;
}

.step-index {
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 50%;
  color: #fff;
  background: #409eff;
  font-size: 12px;
  font-weight: 600;
}

.release-log {
  display: flex;
}

@media (max-width: 1180px) {
  .release-page {
    grid-template-columns: 300px minmax(420px, 1fr);
  }

  .release-log {
    display: none;
  }
}
</style>
