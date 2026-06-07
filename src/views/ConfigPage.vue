<script setup lang="ts">
import { reactive, ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../composables/useAppStore'
import { ElMessage } from 'element-plus'

const store = useAppStore()

// 本地编辑副本
const editRepos = reactive(store.repos.map(r => ({ ...r })))
const editFlow = reactive({ ...store.flowTemplate })
const editGitLab = reactive({ ...store.gitLabConfig })
const projectList = reactive(
  Object.entries(store.gitLabConfig.projects).map(([name, id]) => ({ name, id }))
)

function syncProjectsToStore() {
  const obj: Record<string, string> = {}
  projectList.forEach(p => { if (p.name) obj[p.name] = p.id })
  editGitLab.projects = obj
}

function addProject() {
  projectList.push({ name: '', id: '' })
}

function removeProject(index: number) {
  projectList.splice(index, 1)
  syncProjectsToStore()
}
const editSettings = reactive({ ...store.settings })

function saveConfig() {
  syncProjectsToStore()
  // 更新 Store
  store.repos.splice(0, store.repos.length, ...editRepos)
  store.updateFlowTemplate(editFlow)
  store.updateGitLabConfig(editGitLab)
  store.updateSettings(editSettings)
  if (dbConnected.value) {
    invoke("save_all_config", { data: { repositories: editRepos, flow_templates: [editFlow], gitlab_configs: [editGitLab], project_mappings: projectList, global_settings: [editSettings], repo_tools: [] } }).catch((e: any) => console.error("DB save failed:", e))
  }
  ElMessage.success("配置已保存")
}

const dbUrl = ref("")
const dbConnected = ref(false)
const dbConnecting = ref(false)

onMounted(async () => {
  try {
    const saved = await invoke("load_saved_db_url")
    dbUrl.value = saved
  } catch { /* no saved url */ }
})

async function connectDatabase() {
  if (!dbUrl.value) { ElMessage.warning("请输入 Neon 连接字符串"); return }
  dbConnecting.value = true
  try {
    const result = await invoke("configure_database", { url: dbUrl.value })
    await invoke("save_db_url", { url: dbUrl.value })
    dbConnected.value = true
    ElMessage.success(result)
  } catch (e: any) { ElMessage.error("连接失败: " + e) }
  finally { dbConnecting.value = false }
}

async function loadFromDb() {
  try {
    await invoke("load_all_config")
    ElMessage.success("已从数据库加载配置")
  } catch (e: any) { ElMessage.error("加载失败: " + e) }
}

</script>

<template>
  <div class="config-page">
    <!-- 仓库管理 -->
    <div class="config-section">
      <h3>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        仓库管理
      </h3>
      <el-table :data="editRepos" stripe size="small" style="width:100%">
        <el-table-column type="index" label="#" width="50" />
        <el-table-column prop="path" label="仓库路径" min-width="250">
          <template #default="{ row }">
            <el-input v-model="row.path" size="small" />
          </template>
        </el-table-column>
        <el-table-column prop="alias" label="别名" width="150">
          <template #default="{ row }">
            <el-tag :type="row.alias === '预发环境' ? 'primary' : row.alias === 'AI 服务' ? 'success' : row.alias === '运维' ? 'warning' : 'info'" size="small" effect="plain">
              {{ row.alias }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120">
          <template #default="{ $index }">
            <el-button type="danger" link size="small" @click="editRepos.splice($index, 1); ElMessage.info('已删除')">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
      <div class="section-actions">
        <el-button size="small" type="primary" plain @click="editRepos.push({ id: `repo-${Date.now()}`, path: '', alias: '' })">
          ＋ 添加仓库
        </el-button>
      </div>
    </div>

    <!-- 提交流程模板 -->
    <div class="config-section">
      <h3>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="3" y1="3" x2="9" y2="9"/></svg>
        提交流程模板
      </h3>
      <div class="config-form">
        <div class="form-row">
          <span class="form-label">流程名称</span>
          <el-input v-model="editFlow.name" size="small" style="width:300px" />
        </div>
        <div class="form-row">
          <span class="form-label">源分支模式</span>
          <el-input v-model="editFlow.sourceBranch" size="small" style="width:200px" placeholder="如 prep-{username}" />
        </div>
        <div class="form-row">
          <span class="form-label">MR 目标分支</span>
          <el-input v-model="editFlow.mrTargetBranch" size="small" style="width:200px" placeholder="如 prep-3.0" />
        </div>
        <div class="form-row">
          <span class="form-label">Cherry-pick 目标</span>
          <el-input v-model="editFlow.cpTargetBranch" size="small" style="width:200px" placeholder="如 dev-3.7" />
        </div>
      </div>
    </div>

    <!-- GitLab 配置 -->
    <div class="config-section">
      <h3>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
        GitLab 配置
      </h3>
      <div class="config-form">
        <div class="form-row">
          <span class="form-label">GitLab 地址</span>
          <el-input v-model="editGitLab.url" size="small" style="width:400px" placeholder="https://gitlab.example.com" />
        </div>
        <div class="form-row">
          <span class="form-label">Access Token</span>
          <el-input v-model="editGitLab.token" size="small" style="width:350px" type="password" show-password placeholder="glpat-xxx" />
        </div>
        <div class="form-row" style="align-items:flex-start;">
          <span class="form-label">项目映射</span>
          <div style="flex:1">
            <div v-for="(project, idx) in projectList" :key="idx" class="project-row">
              <el-input v-model="project.name" size="small" style="width:180px" placeholder="项目名如 lasen-rear" />
              <span style="color:#909399;font-size:12px;"> → ID: </span>
              <el-input v-model="project.id" size="small" style="width:80px" placeholder="ID" />
              <el-button type="danger" link size="small" @click="removeProject(idx)">删除</el-button>
            </div>
            <div style="margin-top:8px">
              <el-button size="small" type="primary" link @click="addProject()">＋ 添加项目</el-button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 数据库连接 -->
    <div class="config-section">
      <h3>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
        Neon 数据库连接
      </h3>
      <div class="config-form">
        <div class="form-row">
          <span class="form-label">连接字符串</span>
          <el-input v-model="dbUrl" size="small" style="width:500px" type="password" show-password placeholder="postgresql://..." />
          <el-button type="primary" size="small" :loading="dbConnecting" @click="connectDatabase">
            {{ dbConnected ? "已连接" : "连接并初始化" }}
          </el-button>
        </div>
        <div v-if="dbConnected" class="form-row">
          <span class="form-label">操作</span>
          <el-button size="small" @click="loadFromDb">从数据库加载</el-button>
          <span style="font-size:12px;color:#67c23a;margin-left:8px;">✓ 数据库已连接</span>
        </div>
      </div>
    </div>

    <!-- 全局设置 -->
    <div class="config-section">
      <h3>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
        全局设置
      </h3>
      <div class="config-form">
        <div class="form-row">
          <span class="form-label">默认 Shell</span>
          <el-select v-model="editSettings.shellType" size="small" style="width:200px">
            <el-option label="PowerShell 7" value="pwsh7" />
            <el-option label="PowerShell 5.1 (Windows)" value="pwsh5" />
            <el-option label="Git Bash" value="gitbash" />
            <el-option label="CMD" value="cmd" />
          </el-select>
        </div>
        <div class="form-row">
          <span class="form-label">Git 路径</span>
          <el-input v-model="editSettings.gitPath" size="small" style="width:400px" placeholder="留空则使用系统 PATH 中的 git" />
        </div>
        <div class="form-row">
          <span class="form-label">日志保留行数</span>
          <el-input-number v-model="editSettings.logRetention" size="small" :min="100" :max="5000" :step="100" />
        </div>
      </div>
    </div>

    <!-- 保存按钮 -->
    <div style="padding:16px 0;display:flex;gap:10px;">
      <el-button type="primary" @click="saveConfig">保存全部配置</el-button>
    </div>
  </div>
</template>

<style scoped>
.config-page {
  flex: 1;
  padding: 24px;
  overflow-y: auto;
}

.config-section {
  background: #fff;
  border-radius: 6px;
  padding: 20px;
  margin-bottom: 16px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.06);
}

.config-section h3 {
  font-size: 15px;
  font-weight: 600;
  color: #303133;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #ebeef5;
  display: flex;
  align-items: center;
  gap: 8px;
}

.config-form { display: flex; flex-direction: column; gap: 12px; }

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.form-label {
  width: 120px;
  font-size: 13px;
  color: #606266;
  flex-shrink: 0;
}

.project-row {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 6px;
}

.section-actions {
  margin-top: 12px;
}

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-thumb { background: #c0c4cc; border-radius: 3px; }
</style>





