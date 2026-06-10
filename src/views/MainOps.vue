<script setup lang="ts">
import { computed } from 'vue'
import { ElMessageBox, ElMessage } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../composables/useAppStore'
import { saveConfigToDatabase } from '../utils/configPersistence'
import Terminal from '../components/Terminal.vue'
import StandardCherryPick from '../components/tools/StandardCherryPick.vue'
import HotfixCherryPick from '../components/tools/HotfixCherryPick.vue'
import type { ToolId, LogEntry } from '../types'

const store = useAppStore()

const activeTab = computed(() => store.activeTab)

async function handleRemoveTab(tabId: string) {
  try {
    await ElMessageBox.confirm('确定要移除此仓库吗？', '提示', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    const beforeRepos = store.repos.map(repo => ({ ...repo }))
    if (!store.removeTab(tabId)) return

    try {
      await saveConfigToDatabase(store.repos, store.flowTemplate, store.gitLabConfig, store.settings, store.releaseConfig)
      ElMessage.success('仓库已删除并同步到数据库')
    } catch (e: any) {
      store.replaceRepos(beforeRepos)
      ElMessage.error('删除失败，数据库未同步: ' + e)
    }
  } catch {
    // user cancelled
  }
}

async function handleAddTab() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择 Git 仓库目录",
    })
    if (!selected) return

    // 校验是否为 Git 仓库
    try {
      await invoke("run_git_command", {
        repoPath: selected,
        args: ["rev-parse", "--is-inside-work-tree"],
      })
    } catch {
      ElMessage.error("所选目录不是 Git 仓库，请重新选择")
      return
    }

    const defaultAlias = selected.split("\\").pop()?.split("/").pop() || ""

    const { value: alias } = await ElMessageBox.prompt("", "仓库别名", {
      inputPlaceholder: "如 预发环境",
      inputValue: defaultAlias,
      confirmButtonText: "添加",
      cancelButtonText: "取消",
    })
    if (!alias) return

    store.addRepo({
      id: "repo-" + Date.now(),
      path: selected,
      alias: alias,
    })
    ElMessage.success("仓库添加成功")
  } catch {
    // user cancelled
  }
}

function addLog(entry: Omit<LogEntry, 'timestamp'>) {
  store.addLog(activeTab.value.id, { ...entry, timestamp: Date.now() })
}

// ====== 执行引擎 ======
async function handleExecute(tabId: string, toolId: ToolId, inputs: Record<string, string>) {
  const tab = store.repoTabs.find(t => t.id === tabId)
  if (!tab) return

  const tool = tab.tools.find(t => t.id === toolId)
  if (!tool) return

  // 重置状态
  store.setActiveTool(tabId, toolId)
  store.setToolStatus(tabId, toolId, 'running')
  store.clearLogs(tabId)
  tool.steps.forEach(s => { s.status = 'pending'; s.detail = undefined })

  if (toolId === 'standard-cp') {
    await runStandardCP(tabId, toolId, inputs)
  } else if (toolId === 'hotfix-mr') {
    await runHotfixMR(tabId, toolId, inputs)
  }
}

async function runStandardCP(tabId: string, toolId: ToolId, inputs: Record<string, string>) {
  const { commitMessage, targetBranch } = inputs
  const repo = store.repos.find(r => r.id === store.activeTab?.repoId)
  const repoPath = repo?.path || ''
  const normalizedCommitMessage = commitMessage?.trim() || ''
  let sourceCommit = ''

  const steps = [
    { msg: '同步远程分支信息', cmd: 'fetch', args: ['fetch', 'origin'] },
    { msg: '暂存并提交本地变更', cmd: 'add-commit', args: ['add', '.'] },
    { msg: '推送源分支', cmd: 'push-source', args: ['push'] },
    { msg: '切换到目标分支', cmd: 'checkout-target', args: ['checkout', targetBranch] },
    { msg: '拉取目标分支最新代码', cmd: 'pull-target', args: ['pull'] },
    { msg: '执行 cherry-pick', cmd: 'cherry-pick', args: ['cherry-pick'] },
    { msg: '推送目标分支', cmd: 'push-target', args: ['push'] },
    { msg: '切回源分支', cmd: 'checkout-back', args: ['checkout', '-'] },
  ]

  for (let i = 0; i < steps.length; i++) {
    const step = steps[i]
    store.setStepStatus(tabId, toolId, i, 'running')

    try {
      if (step.cmd === 'add-commit') {
        if (!normalizedCommitMessage) {
          addLog({ text: '未填写提交信息，跳过 add/commit，后续将使用当前 HEAD', cls: 'info' })
        } else {
          addLog({ text: '$ git add .', cls: 'cmd' })
          const addResult = await invoke<string>('run_git_command', {
            repoPath,
            args: ['add', '.'],
          })
          addResult.trim().split('\n').filter(Boolean).forEach(line => {
            addLog({ text: line, cls: line.startsWith('error') || line.startsWith('fatal') ? 'error' : 'output' })
          })

          addLog({ text: `$ git commit -m "${normalizedCommitMessage}"`, cls: 'cmd' })
          const commitResult = await invoke<string>('run_git_command', {
            repoPath,
            args: ['commit', '-m', normalizedCommitMessage],
          })
          commitResult.trim().split('\n').filter(Boolean).forEach(line => {
            addLog({ text: line, cls: line.startsWith('error') || line.startsWith('fatal') ? 'error' : 'output' })
          })
        }
      } else if (step.cmd === 'push-source') {
        addLog({ text: `$ git ${step.args.join(' ')}`, cls: 'cmd' })
        const result = await invoke<string>('run_git_command', { repoPath, args: step.args })
        result.trim().split('\n').filter(Boolean).forEach(line => {
          addLog({ text: line, cls: line.startsWith('error') || line.startsWith('fatal') ? 'error' : 'output' })
        })

        sourceCommit = (await invoke<string>('run_git_command', {
          repoPath,
          args: ['rev-parse', 'HEAD'],
        })).trim()
        addLog({ text: `源分支提交: ${sourceCommit}`, cls: 'dim' })
      } else if (step.cmd === 'cherry-pick') {
        if (!sourceCommit) {
          sourceCommit = (await invoke<string>('run_git_command', {
            repoPath,
            args: ['rev-parse', 'HEAD'],
          })).trim()
        }
        const cherryPickArgs = ['cherry-pick', sourceCommit]
        addLog({ text: `$ git ${cherryPickArgs.join(' ')}`, cls: 'cmd' })
        const result = await invoke<string>('run_git_command', { repoPath, args: cherryPickArgs })
        result.trim().split('\n').filter(Boolean).forEach(line => {
          addLog({ text: line, cls: line.startsWith('error') || line.startsWith('fatal') ? 'error' : 'output' })
        })
      } else {
        addLog({ text: `$ git ${step.args.join(' ')}`, cls: 'cmd' })
        const result = await invoke<string>('run_git_command', { repoPath, args: step.args })
        result.trim().split('\n').filter(Boolean).forEach(line => {
          addLog({ text: line, cls: line.startsWith('error') || line.startsWith('fatal') ? 'error' : 'output' })
        })
      }
      store.setStepStatus(tabId, toolId, i, 'done')
      addLog({ text: `✔ ${step.msg} 成功`, cls: 'success' })
    } catch (e: any) {
      addLog({ text: `✗ ${step.msg} 失败：${e}`, cls: 'error' })
      store.setStepStatus(tabId, toolId, i, 'error')
      store.setToolStatus(tabId, toolId, 'error')
      return
    }

    // 短延时让 UI 更新
    await new Promise(r => setTimeout(r, 200))
  }

  store.setToolStatus(tabId, toolId, 'done')
  addLog({ text: '', cls: 'dim' })
  addLog({ text: '✅ 普通 Cherry-Pick 流程执行完毕', cls: 'success' })
}

async function runHotfixMR(tabId: string, toolId: ToolId, inputs: Record<string, string>) {
  const { commitHash, hotfixBranch } = inputs
  const repo = store.repos.find(r => r.id === store.activeTab?.repoId)
  const repoPath = repo?.path || ''
  const config = store.gitLabConfig
  const flow = store.flowTemplate

  let actualHash = commitHash?.trim() || ''
  const mrTarget = flow.mrTargetBranch || 'main'

  const steps = [
    { msg: '同步远程分支信息', cmd: 'fetch', args: ['fetch', 'origin'] },
    { msg: '检查/创建 hotfix 分支', cmd: 'check-hotfix', args: ['checkout', hotfixBranch || 'hotfix-licanzhang'] },
    { msg: `同步 ${mrTarget} 到 hotfix`, cmd: 'sync-main', args: ['pull'] },
    { msg: '执行 cherry-pick', cmd: 'cherry-pick', args: ['cherry-pick'] },
    { msg: '推送 hotfix 分支', cmd: 'push-hotfix', args: ['push'] },
    { msg: '创建 GitLab Merge Request', cmd: 'mr', args: [] },
    { msg: '切回源分支', cmd: 'checkout-back', args: ['checkout', '-'] },
  ]

  for (let i = 0; i < steps.length; i++) {
    const step = steps[i]
    store.setStepStatus(tabId, toolId, i, 'running')

    if (step.cmd === 'mr') {
      addLog({ text: '[GitLab API] 创建 Merge Request...', cls: 'info' })
      try {
        const mrUrl = await invoke<string>('create_gitlab_mr', {
          gitlabUrl: config.url,
          token: config.token,
          projectId: Object.values(config.projects)[0] || '',
          sourceBranch: hotfixBranch || 'hotfix-licanzhang',
          targetBranch: mrTarget,
          title: flow.name,
        })
        addLog({ text: `✔ Merge Request 创建成功`, cls: 'success' })
        addLog({ text: `   URL: ${mrUrl}`, cls: 'dim' })
        store.setStepStatus(tabId, toolId, i, 'done')
      } catch (e: any) {
        addLog({ text: `✗ MR 创建失败：${e}`, cls: 'error' })
        store.setStepStatus(tabId, toolId, i, 'error')
      }
    } else if (step.cmd === 'check-hotfix') {
      addLog({ text: `$ git ${step.args.join(' ')}`, cls: 'cmd' })
      try {
        await invoke<string>('run_git_command', { repoPath, args: step.args })
        addLog({ text: `  已切换到 ${step.args[1]}`, cls: 'output' })
        store.setStepStatus(tabId, toolId, i, 'done')
        addLog({ text: `✔ ${step.msg} 成功`, cls: 'success' })
      } catch (e: any) {
        // 分支不存在，从 mrTarget 创建
        addLog({ text: `  分支 ${step.args[1]} 不存在，尝试从 ${mrTarget} 创建...`, cls: 'warn' })
        try {
          await invoke('run_git_command', { repoPath, args: ['checkout', mrTarget] })
          await invoke('run_git_command', { repoPath, args: ['pull', 'origin', mrTarget] })
          await invoke('run_git_command', { repoPath, args: ['checkout', '-b', step.args[1]] })
          await invoke('run_git_command', { repoPath, args: ['push', '-u', 'origin', step.args[1]] })
          addLog({ text: `  ✔ 新分支 ${step.args[1]} 已从 ${mrTarget} 创建`, cls: 'success' })
          store.setStepStatus(tabId, toolId, i, 'done')
        } catch (e2: any) {
          addLog({ text: `✗ 创建分支失败：${e2}`, cls: 'error' })
          store.setStepStatus(tabId, toolId, i, 'error')
          store.setToolStatus(tabId, toolId, 'error')
          return
        }
      }
    } else {
      try {
        let args = step.args
        if (step.cmd === 'fetch' && !actualHash) {
          addLog({ text: `$ git ${args.join(' ')}`, cls: 'cmd' })
          const result = await invoke<string>('run_git_command', { repoPath, args })
          result.trim().split('\n').filter(Boolean).forEach(line => {
            addLog({ text: line, cls: 'output' })
          })

          actualHash = (await invoke<string>('run_git_command', {
            repoPath,
            args: ['rev-parse', 'HEAD'],
          })).trim()
          addLog({ text: `源分支提交: ${actualHash}`, cls: 'dim' })
          store.setStepStatus(tabId, toolId, i, 'done')
          addLog({ text: `✔ ${step.msg} 成功`, cls: 'success' })
          await new Promise(r => setTimeout(r, 200))
          continue
        }

        if (step.cmd === 'cherry-pick') {
          if (!actualHash) {
            actualHash = (await invoke<string>('run_git_command', {
              repoPath,
              args: ['rev-parse', 'HEAD'],
            })).trim()
          }
          args = ['cherry-pick', actualHash]
        }

        addLog({ text: `$ git ${args.join(' ')}`, cls: 'cmd' })
        const result = await invoke<string>('run_git_command', { repoPath, args })
        result.trim().split('\n').filter(Boolean).forEach(line => {
          addLog({ text: line, cls: 'output' })
        })
        store.setStepStatus(tabId, toolId, i, 'done')
        addLog({ text: `✔ ${step.msg} 成功`, cls: 'success' })
      } catch (e: any) {
        addLog({ text: `✗ ${step.msg} 失败：${e}`, cls: 'error' })
        store.setStepStatus(tabId, toolId, i, 'error')
        store.setToolStatus(tabId, toolId, 'error')
        return
      }
    }
    await new Promise(r => setTimeout(r, 200))
  }

  store.setToolStatus(tabId, toolId, 'done')
  addLog({ text: '', cls: 'dim' })
  addLog({ text: '✅ Hotfix + GitLab MR 流程执行完毕', cls: 'success' })
}

function getRepoAlias(path: string): string {
  const repo = store.repos.find(r => r.path === path)
  return repo?.alias || ''
}
</script>

<template>
  <div class="ops-container">
    <!-- Tab 栏 -->
    <div class="tab-bar">
      <div
        v-for="tab in store.repoTabs"
        :key="tab.id"
        class="tab-item"
        :class="{ active: store.activeTabId === tab.id }"
        @click="store.switchTab(tab.id)"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <span class="tab-label">{{ tab.alias }}</span>
        <span class="tab-path">{{ tab.path }}</span>
        <span class="tab-close" @click.stop="handleRemoveTab(tab.id)">✕</span>
      </div>
    </div>

    <!-- 内容区 -->
    <div v-if="activeTab" class="tab-content">
      <!-- 左侧：工具列表 -->
      <div class="tools-panel">
        <template v-for="tool in activeTab.tools" :key="tool.id">
          <StandardCherryPick
            v-if="tool.id === 'standard-cp'"
            :tab-id="activeTab.id"
            :tool="tool"
            @execute="handleExecute"
          />
          <HotfixCherryPick
            v-else-if="tool.id === 'hotfix-mr'"
            :tab-id="activeTab.id"
            :tool="tool"
            @execute="handleExecute"
          />
        </template>

        <!-- 空状态 -->
        <div v-if="activeTab.tools.length === 0" class="empty-tools">
          <p>该仓库暂无可用工具</p>
          <p class="sub">请先在「配置管理」中添加工具</p>
        </div>
      </div>

      <!-- 右侧：终端 -->
      <Terminal
        :logs="activeTab.logs"
        :title="activeTab.activeTool ? (activeTab.tools.find(t => t.id === activeTab.activeTool)?.title || '') : '执行日志'"
        :subtitle="activeTab.alias"
      />

    </div>

    <!-- 底部按钮 -->
    <div class="bottom-actions" v-if="activeTab && activeTab.activeTool">
      <el-button
        type="primary"
        @click="store.clearLogs(activeTab.id)"
        size="small"
        plain
      >
        清空日志
      </el-button>
    </div>
  </div>
</template>

<style scoped>
.ops-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.tab-bar {
  display: flex;
  align-items: center;
  background: #fff;
  border-bottom: 1px solid #e4e7ed;
  padding: 0 8px;
  min-height: 40px;
  flex-shrink: 0;
  overflow-x: auto;
}

.tab-item, .tab-add {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  font-size: 16px;
  color: #c0c4cc;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}
.tab-add:hover { color: #409eff; }

.tab-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  font-size: 13px;
  color: #606266;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}
.tab-item:hover { color: #409eff; }
.tab-item.active {
  color: #409eff;
  border-bottom-color: #409eff;
}
.tab-item .tab-path {
  font-size: 11px; color: #c0c4cc; max-width: 150px;
  overflow: hidden; text-overflow: ellipsis;
}
.tab-item .tab-close {
  width: 14px; height: 14px;
  border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  font-size: 10px;
  opacity: 0;
  transition: opacity 0.2s;
  margin-left: 4px;
}
.tab-item:hover .tab-close { opacity: 0.6; }
.tab-item .tab-close:hover { background: #e4e7ed; opacity: 1; }

.tab-content {
  flex: 1;
  display: flex;
  padding: 12px;
  gap: 12px;
  overflow: hidden;
}

.tools-panel {
  width: 340px;
  min-width: 280px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  flex-shrink: 0;
}

.empty-tools {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: #c0c4cc;
  gap: 4px;
}
.empty-tools .sub { font-size: 12px; }

.bottom-actions {
  padding: 8px 12px;
  display: flex;
  gap: 10px;
  flex-shrink: 0;
  border-top: 1px solid #e4e7ed;
  background: #fff;
}

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-thumb { background: #c0c4cc; border-radius: 3px; }

@media (max-width: 900px) {
  .tab-content { flex-direction: column; }
  .tools-panel { width: 100%; min-width: unset; max-height: 40%; }
}
</style>

