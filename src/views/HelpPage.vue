<script setup lang="ts">
import { useAppStore } from '../composables/useAppStore'
const store = useAppStore()
</script>

<template>
  <div class="help-page">
    <!-- 工具说明 -->
    <div class="help-section">
      <h3>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
        1. 普通 Cherry-Pick（Standard）
      </h3>
      <p class="help-desc">将当前分支的本地变更提交并推送后，自动 cherry-pick 到目标分支。</p>

      <h4>使用场景</h4>
      <ul>
        <li>功能在 <code>feature/xxx</code> 开发完成，需要合入 <code>dev-3.7</code></li>
        <li>Bugfix 在 <code>fix/xxx</code> 修复后，需要同步到 <code>develop</code></li>
      </ul>

      <h4>执行流程</h4>
      <div class="flow-steps">
        <div class="flow-step"><span class="step-num">1</span> <code>git fetch origin</code> — 同步远程分支信息</div>
        <div class="flow-step"><span class="step-num">2</span> <code>git add . → git commit</code> — 暂存并提交本地变更</div>
        <div class="flow-step"><span class="step-num">3</span> <code>git push</code> — 推送源分支到远端</div>
        <div class="flow-step"><span class="step-num">4</span> <code>git checkout &lt;target&gt;</code> — 切换到目标分支</div>
        <div class="flow-step"><span class="step-num">5</span> <code>git pull</code> — 拉取目标分支最新代码</div>
        <div class="flow-step"><span class="step-num">6</span> <code>git cherry-pick HEAD</code> — 获取最新提交</div>
        <div class="flow-step"><span class="step-num">7</span> <code>git push</code> — 推送目标分支</div>
        <div class="flow-step"><span class="step-num">8</span> <code>git checkout -</code> — 切回源分支</div>
      </div>

      <h4>参数说明</h4>
      <el-table :data="[
        { name: '提交信息', req: '推荐填写', desc: '留空则只 push 已有 commit，不创建新提交' },
        { name: '目标分支', req: '必填', desc: 'Cherry-pick 的目标分支，如 dev-3.7' }
      ]" stripe size="small">
        <el-table-column prop="name" label="参数" width="120" />
        <el-table-column prop="req" label="必填" width="100" />
        <el-table-column prop="desc" label="说明" />
      </el-table>

      <h4>风险提示</h4>
      <el-alert type="warning" :closable="false" show-icon>
        <template #title>
          执行前请确保工作区没有未暂存的变更。Cherry-pick 冲突时会停留在目标分支，需要手动解决后执行 <code>git cherry-pick --continue</code>。
        </template>
      </el-alert>
    </div>

    <!-- Hotfix -->
    <div class="help-section">
      <h3>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
        2. Hotfix + GitLab MR
      </h3>
      <p class="help-desc">将指定提交 cherry-pick 到 hotfix 分支，推送后自动调用 GitLab API 创建合并请求。</p>

      <h4>使用场景</h4>
      <ul>
        <li>线上紧急 bug 修复，从 <code>prep-licanzhang</code> 合入 <code>hotfix-xxx</code></li>
        <li>修复完成后自动创建 MR：<code>hotfix-xxx → main</code></li>
      </ul>

      <h4>执行流程</h4>
      <div class="flow-steps">
        <div class="flow-step"><span class="step-num">1</span> <code>git fetch origin</code> — 同步远程分支信息</div>
        <div class="flow-step"><span class="step-num">2</span> 检查 hotfix 分支是否存在，不存在则从 MR 目标分支创建</div>
        <div class="flow-step"><span class="step-num">3</span> 如果分支已存在：合并 MR 目标分支到 hotfix 保持同步</div>
        <div class="flow-step"><span class="step-num">4</span> <code>git cherry-pick &lt;commit&gt;</code> — 执行 cherry-pick</div>
        <div class="flow-step"><span class="step-num">5</span> <code>git push</code> — 推送 hotfix 分支</div>
        <div class="flow-step"><span class="step-num">6</span> <code>GitLab API → POST /merge_requests</code> — 创建 MR</div>
        <div class="flow-step"><span class="step-num">7</span> <code>git checkout -</code> — 切回源分支</div>
      </div>

      <h4>参数说明</h4>
      <el-table :data="[
        { name: 'Commit Hash', req: '可选', desc: '要 cherry-pick 的提交 hash；留空使用 HEAD（最新提交）' },
        { name: 'Hotfix 分支', req: '推荐填写', desc: '目标 hotfix 分支名，如 hotfix-licanzhang；留空使用配置中的默认值' }
      ]" stripe size="small">
        <el-table-column prop="name" label="参数" width="120" />
        <el-table-column prop="req" label="必填" width="100" />
        <el-table-column prop="desc" label="说明" />
      </el-table>

      <h4>配置要求</h4>
      <p class="help-desc">使用此工具前，请先在「配置管理」中填写以下信息：</p>
      <ul>
        <li><strong>GitLab URL</strong> — 如 http://gitlab.5codemonkey.com:2818</li>
        <li><strong>Access Token</strong> — GitLab Personal Access Token</li>
        <li><strong>项目映射</strong> — 仓库目录名 → GitLab 项目 ID 的映射</li>
        <li><strong>提交流程模板</strong> — MR 目标分支、Cherry-pick 目标分支</li>
      </ul>
    </div>

    <!-- 配置管理 -->
    <div class="help-section">
      <h3>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
        3. 配置管理
      </h3>
      <p class="help-desc">在「配置管理」页面可以管理：</p>
      <ul>
        <li><strong>仓库列表</strong> — 添加/删除要管理的 Git 仓库路径和别名</li>
        <li><strong>提交流程模板</strong> — 配置默认的 MR 目标分支、Cherry-pick 目标分支</li>
        <li><strong>GitLab 连接</strong> — 配置 GitLab 地址、Token、项目 ID 映射</li>
        <li><strong>全局设置</strong> — Shell 类型、Git 路径、日志保留行数</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.help-page {
  flex: 1;
  padding: 24px 32px;
  overflow-y: auto;
  max-width: 900px;
}

.help-section {
  background: #fff;
  border-radius: 6px;
  padding: 24px;
  margin-bottom: 20px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.06);
}

.help-section h3 {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
  margin-bottom: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.help-section h4 {
  font-size: 14px;
  font-weight: 500;
  color: #606266;
  margin: 16px 0 8px;
}

.help-desc {
  font-size: 13px;
  color: #606266;
  line-height: 1.6;
  margin-bottom: 8px;
}

ul {
  list-style: none;
  padding: 0;
}
ul li {
  font-size: 13px;
  color: #606266;
  padding: 4px 0 4px 16px;
  position: relative;
  line-height: 1.6;
}
ul li::before {
  content: "•";
  position: absolute;
  left: 4px;
  color: #409eff;
}

code {
  background: #f4f4f5;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 12px;
  color: #409eff;
}

.flow-steps {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.flow-step {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: #606266;
  padding: 6px 10px;
  background: #fafafa;
  border-radius: 4px;
}

.step-num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: #409eff;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  flex-shrink: 0;
}

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-thumb { background: #c0c4cc; border-radius: 3px; }
</style>
