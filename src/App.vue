<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from './composables/useAppStore'
import MainOps from './views/MainOps.vue'
import ConfigPage from './views/ConfigPage.vue'
import HelpPage from './views/HelpPage.vue'

const store = useAppStore()
const currentPage = computed(() => store.currentPage)
</script>

<template>
  <el-container class="app-container">
    <!-- 侧边栏 -->
    <el-aside width="220px" class="sidebar">
      <div class="logo-area">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#409eff" stroke-width="2">
          <circle cx="12" cy="12" r="3"/><path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83"/>
        </svg>
        <span class="logo-text">GitOps</span>
      </div>

      <el-menu
        :default-active="currentPage"
        class="sidebar-menu"
        background-color="#304156"
        text-color="#bfcbd9"
        active-text-color="#409eff"
        @select="(index: string) => store.setPage(index as any)"
      >
        <el-menu-item index="main">
          <el-icon><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg></el-icon>
          <span>主操作页</span>
        </el-menu-item>
        <el-menu-item index="config">
          <el-icon><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg></el-icon>
          <span>配置管理</span>
        </el-menu-item>
        <el-menu-item index="help">
          <el-icon><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg></el-icon>
          <span>帮助</span>
        </el-menu-item>
      </el-menu>
    </el-aside>

    <!-- 主内容区 -->
    <el-main class="main-area">
      <MainOps v-if="currentPage === 'main'" />
      <ConfigPage v-else-if="currentPage === 'config'" />
      <HelpPage v-else-if="currentPage === 'help'" />
    </el-main>
  </el-container>
</template>

<style scoped>
.app-container {
  height: 100vh;
  width: 100vw;
}

.sidebar {
  background-color: #304156;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-menu {
  flex: 1;
  border-right: none;
}

.logo-area {
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: rgba(0, 0, 0, 0.2);
}

.logo-text {
  color: #fff;
  font-size: 20px;
  font-weight: 600;
  white-space: nowrap;
}

.main-area {
  background-color: #f0f2f5;
  padding: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
