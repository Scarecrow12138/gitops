<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import type { LogEntry } from '../types'

const props = defineProps<{
  logs: LogEntry[]
  title?: string
  subtitle?: string
}>()

const termBody = ref<HTMLDivElement>()

watch(() => props.logs.length, async () => {
  await nextTick()
  if (termBody.value) {
    termBody.value.scrollTop = termBody.value.scrollHeight
  }
}, { flush: 'post' })
</script>

<template>
  <div class="terminal-panel">
    <div class="terminal-header">
      <div class="terminal-dots">
        <span class="terminal-dot red"></span>
        <span class="terminal-dot yellow"></span>
        <span class="terminal-dot green"></span>
      </div>
      <span class="terminal-title">{{ title || "执行日志" }}</span>
      <span v-if="subtitle" class="terminal-subtitle">{{ subtitle }}</span>
    </div>
    <div class="terminal-body" ref="termBody">
      <template v-if="logs.length === 0">
        <div class="terminal-line">
          <span class="prompt">[GitOps]</span>
          <span class="dim"> 等待执行——选择工具后点击「执行」开始</span>
        </div>
      </template>
      <div
        v-for="(entry, i) in logs"
        :key="i"
        class="terminal-line"
      >
        <span :class="entry.cls">{{ entry.text }}</span>
      </div>
      <div class="terminal-line terminal-cursor">&nbsp;</div>
    </div>
  </div>
</template>

<style scoped>
.terminal-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
  border-radius: 6px;
  overflow: hidden;
  min-width: 0;
}

.terminal-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  background: #2d2d2d;
  color: #999;
  font-size: 12px;
  flex-shrink: 0;
}

.terminal-dots { display: flex; gap: 6px; }
.terminal-dot {
  width: 10px; height: 10px;
  border-radius: 50%;
}
.terminal-dot.red { background: #ff5f56; }
.terminal-dot.yellow { background: #ffbd2e; }
.terminal-dot.green { background: #27c93f; }

.terminal-title { color: #ccc; font-size: 12px; flex: 1; }
.terminal-subtitle { color: #666; font-size: 11px; }

.terminal-body {
  flex: 1;
  padding: 16px;
  font-family: "Cascadia Code", "Fira Code", "Consolas", monospace;
  font-size: 13px;
  line-height: 1.7;
  overflow-y: auto;
  color: #d4d4d4;
}

.terminal-line { white-space: pre-wrap; word-break: break-all; }
.terminal-line .prompt { color: #6a9955; }
.terminal-line .cmd { color: #dcdcaa; }
.terminal-line .info { color: #569cd6; }
.terminal-line .success { color: #4ec9b0; }
.terminal-line .warn { color: #ce9178; }
.terminal-line .error { color: #f44747; }
.terminal-line .dim { color: #6a9955; }
.terminal-line .output { color: #d4d4d4; }

.terminal-cursor::after {
  content: "█";
  animation: blink 1s step-end infinite;
  color: #d4d4d4;
}
@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: #3a3a3a; border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: #555; }
</style>
