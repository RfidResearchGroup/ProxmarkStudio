<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import {
  ArrowLeft,
  Clock,
  Copy,
  Expand,
  Minimize2,
  RefreshCw,
  RotateCcw,
  Square,
  Terminal as TerminalIcon,
  Unplug,
  Zap,
} from 'lucide-vue-next'
import BrandMark from '@/components/BrandMark.vue'
import { useConnectionStore } from '@/stores/connection'
import { useSettingsStore } from '@/stores/settings'
import { useToastStore } from '@/stores/toast'
import { executeStream } from '@/services/pm3'
import { copyText, errorMessage, formatDuration } from '@/utils/format'

const router = useRouter()
const connection = useConnectionStore()
const settings = useSettingsStore()
const toast = useToastStore()

const terminalHost = ref<HTMLDivElement | null>(null)
const terminalReady = ref(false)
const running = ref(false)
const lastCommand = ref('')
const historyCount = ref(0)
const fullscreen = ref(false)
const reconnecting = ref(false)
const now = ref(Date.now())

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let resizeObserver: ResizeObserver | null = null
let clockTimer: number | null = null
let lineBuffer = ''
let history: string[] = []
let historyIndex = -1

const prompt = '\x1b[38;2;76;240;200m pm3 \x1b[0m\x1b[38;2;86;102;127m›\x1b[0m '

const connectionAge = computed(() => {
  if (!connection.connectedAt) return ''
  return formatDuration(Math.max(0, Math.floor((now.value - connection.connectedAt) / 1000)))
})

const statusText = computed(() => {
  if (running.value) return settings.t('terminal.running')
  if (connection.isConnected) return settings.t('terminal.ready')
  return settings.t('terminal.disconnected')
})

function cssVar(name: string): string {
  if (typeof window === 'undefined') return '#4cf0c8'
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#4cf0c8'
}

function hexToRgbTriplet(hex: string, fallback = '76;240;200'): string {
  const normalized = hex.replace('#', '')
  if (normalized.length !== 6) return fallback
  const value = Number.parseInt(normalized, 16)
  if (Number.isNaN(value)) return fallback
  return `${(value >> 16) & 255};${(value >> 8) & 255};${value & 255}`
}

function terminalTheme() {
  return {
    background: '#050810',
    foreground: cssVar('--text'),
    cursor: cssVar('--accent'),
    cursorAccent: '#050810',
    selectionBackground: 'rgba(76, 240, 200, 0.24)',
    black: '#050810',
    red: cssVar('--danger'),
    green: cssVar('--success'),
    yellow: cssVar('--warning'),
    blue: cssVar('--secondary'),
    magenta: '#b18cff',
    cyan: cssVar('--accent'),
    white: '#e8eef8',
    brightBlack: '#56667f',
    brightRed: '#ff8a9d',
    brightGreen: '#8bffe6',
    brightYellow: '#ffd89a',
    brightBlue: '#93c1ff',
    brightMagenta: '#d0b8ff',
    brightCyan: '#8bffe6',
    brightWhite: '#ffffff',
  }
}

function writeWelcome() {
  if (!term) return
  const accent = hexToRgbTriplet(cssVar('--accent'))
  const muted = hexToRgbTriplet(cssVar('--faint'))
  const client = connection.clientVersion || connection.session?.version || 'Proxmark3 client'
  const port = connection.portLabel || '—'
  term.writeln('')
  term.writeln(`\x1b[1;38;2;${accent}m  P R O X M A R K   S T U D I O  \x1b[0m`)
  term.writeln(`\x1b[38;2;${muted}m  RFID WORKBENCH  /  CLIENT TERMINAL\x1b[0m`)
  term.writeln(`\x1b[38;2;${muted}m  ---------------------------------\x1b[0m`)
  term.writeln('')
  term.writeln(`\x1b[1m ${settings.t('terminal.welcome')}\x1b[0m`)
  term.writeln(`\x1b[38;2;${muted}m ${settings.t('terminal.welcomeClient', { version: client })}\x1b[0m`)
  term.writeln(`\x1b[38;2;${muted}m ${settings.t('terminal.welcomeSession', { port })}\x1b[0m`)
  term.writeln('')
  term.writeln(
    `\x1b[38;2;${muted}m ${settings.t('terminal.welcomeHint', { command: 'hw version' })}\x1b[0m`,
  )
  term.writeln('')
  if (connection.isConnected) {
    term.write(prompt)
  } else {
    term.writeln(`\x1b[38;2;${muted}m ${settings.t('terminal.notConnectedBanner')}\x1b[0m`)
  }
}

function initialiseTerminal() {
  if (!terminalHost.value || term) return
  term = new Terminal({
    cursorBlink: true,
    cursorStyle: 'bar',
    fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim() || 'monospace',
    fontSize: window.innerWidth < 640 ? 11 : 12.5,
    lineHeight: 1.4,
    letterSpacing: 0.1,
    scrollback: 8000,
    convertEol: true,
    allowTransparency: true,
    theme: terminalTheme(),
  })
  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.open(terminalHost.value)
  fitAddon.fit()
  terminalReady.value = true
  writeWelcome()
  term.focus()
  term.onData(onTerminalData)
  term.attachCustomKeyEventHandler(onTerminalKey)
  resizeObserver = new ResizeObserver(() => {
    if (!terminalHost.value || !fitAddon) return
    requestAnimationFrame(() => {
      try {
        fitAddon?.fit()
      } catch {
        /* terminal may be disposing */
      }
    })
  })
  resizeObserver.observe(terminalHost.value)
}

function onTerminalKey(event: KeyboardEvent): boolean {
  if (event.type !== 'keydown') return true
  if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'c') {
    const selection = term?.getSelection()
    if (selection) void copyText(selection)
    return false
  }
  if (event.ctrlKey && event.key.toLowerCase() === 'v') {
    return true
  }
  return true
}

function writePrompt() {
  if (!term || !connection.isConnected) return
  term.write(prompt)
}

function replaceCurrentLine(next: string) {
  if (!term) return
  term.write('\r\x1b[2K')
  term.write(prompt)
  term.write(next)
  lineBuffer = next
}

async function onTerminalData(data: string) {
  if (!term || !connection.isConnected) return
  if (running.value) return

  if (data === '\r') {
    const command = lineBuffer.trim()
    lineBuffer = ''
    term.write('\r\n')
    if (!command) {
      writePrompt()
      return
    }
    await runCommand(command)
    return
  }

  if (data === '\u007F') {
    if (!lineBuffer.length) return
    lineBuffer = lineBuffer.slice(0, -1)
    term.write('\b \b')
    return
  }

  if (data === '\u0003') {
    lineBuffer = ''
    term.write('^C\r\n')
    writePrompt()
    return
  }

  if (data === '\u000C') {
    term.clear()
    writePrompt()
    return
  }

  if (data === '\u001b[A') {
    if (!history.length) return
    historyIndex = Math.min(history.length - 1, historyIndex + 1)
    replaceCurrentLine(history[history.length - 1 - historyIndex] ?? '')
    return
  }

  if (data === '\u001b[B') {
    if (historyIndex < 0) return
    historyIndex -= 1
    if (historyIndex < 0) replaceCurrentLine('')
    else replaceCurrentLine(history[history.length - 1 - historyIndex] ?? '')
    return
  }

  // Ignore remaining escape/control sequences. Printable input is appended.
  if (data.length > 1 && data.startsWith('\u001b')) return
  const printable = [...data].filter((char) => char >= ' ' && char !== '\u007f').join('')
  if (!printable) return
  lineBuffer += printable
  term.write(printable)
}

async function runCommand(command: string) {
  if (!term) return
  if (!connection.isConnected) {
    term.writeln(`\x1b[31m${settings.t('terminal.notConnectedBanner')}\x1b[0m`)
    return
  }
  if (running.value) {
    term.writeln(`\x1b[33m${settings.t('terminal.runningBlocked')}\x1b[0m`)
    writePrompt()
    return
  }

  const normalized = command.trim()
  if (normalized === 'clear' || normalized === 'cls') {
    term.clear()
    writePrompt()
    return
  }
  if (normalized === 'history') {
    if (!history.length) term.writeln(`\x1b[2m${settings.t('terminal.historyEmpty')}\x1b[0m`)
    else history.forEach((entry, index) => term?.writeln(`  ${String(index + 1).padStart(3, ' ')}  ${entry}`))
    writePrompt()
    return
  }

  lastCommand.value = normalized
  history = [...history.filter((entry) => entry !== normalized), normalized].slice(-200)
  historyCount.value = history.length
  historyIndex = -1
  running.value = true
  let endedWithNewline = false

  try {
    await executeStream(normalized, (chunk) => {
      if (!term) return
      endedWithNewline = /(?:\r?\n)$/.test(chunk)
      term.write(chunk)
    })
    if (!endedWithNewline) term.write('\r\n')
  } catch (error) {
    if (!endedWithNewline) term.write('\r\n')
    term.writeln(`\x1b[31m[!] ${settings.t('terminal.commandError')}: ${errorMessage(error)}\x1b[0m`)
    if (/not open|no proxmark|session/i.test(errorMessage(error))) {
      await connection.refreshSession()
    }
  } finally {
    running.value = false
    if (term && connection.isConnected) writePrompt()
  }
}

function clearTerminal() {
  term?.clear()
  writePrompt()
}

function focusTerminal() {
  term?.focus()
}

async function copySelection() {
  const selection = term?.getSelection()
  if (!selection) return
  try {
    await copyText(selection)
    toast.success(settings.t('toast.copied'))
  } catch {
    toast.error(settings.t('toast.copyFailed'))
  }
}

async function reconnect() {
  if (reconnecting.value) return
  reconnecting.value = true
  try {
    const session = await connection.reconnect()
    if (!session) {
      toast.warning(settings.t('terminal.connectFirst'), settings.t('terminal.connectFirstBody'))
    } else {
      await nextTick()
      resetTerminal()
    }
  } catch {
    /* store reports */
  } finally {
    reconnecting.value = false
  }
}

function resetTerminal() {
  if (!term) {
    initialiseTerminal()
    return
  }
  term.reset()
  term.options.theme = terminalTheme()
  writeWelcome()
  term.focus()
}

async function toggleFullscreen() {
  try {
    if (!document.fullscreenElement) {
      await document.documentElement.requestFullscreen()
      fullscreen.value = true
    } else {
      await document.exitFullscreen()
      fullscreen.value = false
    }
    setTimeout(() => fitAddon?.fit(), 120)
  } catch {
    /* fullscreen can be rejected by the host */
  }
}

function onFullscreenChange() {
  fullscreen.value = Boolean(document.fullscreenElement)
  setTimeout(() => fitAddon?.fit(), 120)
}

function goHome() {
  void router.push('/')
}

watch(
  () => connection.isConnected,
  async (connected) => {
    if (connected) {
      await nextTick()
      if (!term) initialiseTerminal()
      else {
        term.reset()
        writeWelcome()
        term.focus()
      }
    } else if (term) {
      term.writeln('')
      term.writeln(`\x1b[33m${settings.t('terminal.sessionEnded')}\x1b[0m`)
      term.dispose()
      term = null
      fitAddon = null
      terminalReady.value = false
      resizeObserver?.disconnect()
      resizeObserver = null
    }
  },
)

watch(
  () => settings.theme,
  () => {
    if (term) term.options.theme = terminalTheme()
  },
)

onMounted(async () => {
  clockTimer = window.setInterval(() => {
    now.value = Date.now()
  }, 5000)
  document.addEventListener('fullscreenchange', onFullscreenChange)
  if (connection.isConnected) {
    await nextTick()
    initialiseTerminal()
  }
})

onUnmounted(() => {
  if (clockTimer) window.clearInterval(clockTimer)
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  resizeObserver?.disconnect()
  term?.dispose()
  term = null
  fitAddon = null
})
</script>

<template>
  <div class="terminal-page">
    <header class="terminal-bar">
      <button class="terminal-back" :title="settings.t('common.back')" @click="goHome">
        <ArrowLeft :size="18" />
      </button>
      <div class="terminal-brand">
        <BrandMark :size="28" />
        <div class="terminal-brand__text">
          <span>{{ settings.t('terminal.title') }}</span>
          <em>{{ settings.t('terminal.eyebrow') }}</em>
        </div>
      </div>

      <div class="terminal-conn" :class="{ 'terminal-conn--offline': !connection.isConnected }">
        <span class="status-dot" :class="connection.isConnected ? 'status-dot--online' : 'status-dot--error'" />
        <span class="terminal-conn__text">
          {{ connection.isConnected ? connection.portLabel : settings.t('terminal.disconnected') }}
        </span>
        <span v-if="connection.isConnected" class="terminal-conn__sep">·</span>
        <span v-if="connection.isConnected" class="terminal-conn__age mono">{{ connectionAge }}</span>
      </div>

      <div class="terminal-actions">
        <button class="icon-btn icon-btn--bordered" :title="settings.t('terminal.reconnect')" :disabled="reconnecting" @click="reconnect">
          <RefreshCw :size="16" :class="{ spinning: reconnecting }" />
        </button>
        <button class="icon-btn icon-btn--bordered" :title="settings.t('terminal.copySelection')" @click="copySelection">
          <Copy :size="16" />
        </button>
        <button class="icon-btn icon-btn--bordered" :title="settings.t('terminal.clear')" @click="clearTerminal">
          <RotateCcw :size="16" />
        </button>
        <button class="icon-btn icon-btn--bordered" :title="settings.t('terminal.fullscreen')" @click="toggleFullscreen">
          <Minimize2 v-if="fullscreen" :size="16" />
          <Expand v-else :size="16" />
        </button>
      </div>
    </header>

    <div v-if="connection.isConnected" class="terminal-body">
      <div class="terminal-window">
        <div class="terminal-window__chrome">
          <span class="chrome-dot chrome-dot--red" />
          <span class="chrome-dot chrome-dot--amber" />
          <span class="chrome-dot chrome-dot--green" />
          <span class="terminal-window__title mono">
            {{ connection.portLabel }} — pm3
          </span>
          <span v-if="running" class="terminal-window__running">
            <span class="spinner spinner--sm" />
            {{ settings.t('terminal.running') }}
          </span>
        </div>
        <div ref="terminalHost" class="terminal-host" @click="focusTerminal" />
      </div>
    </div>

    <div v-else class="terminal-empty">
      <div class="terminal-empty__card">
        <span class="terminal-empty__icon"><Unplug :size="26" /></span>
        <span class="eyebrow">{{ settings.t('terminal.eyebrow') }}</span>
        <h1>{{ settings.t('terminal.connectFirst') }}</h1>
        <p>{{ settings.t('terminal.connectFirstBody') }}</p>
        <div class="terminal-empty__actions">
          <button class="btn btn--outline" @click="reconnect">
            <RefreshCw :size="15" />
            {{ settings.t('terminal.reconnect') }}
          </button>
          <button class="btn btn--primary" @click="goHome">
            <ArrowLeft :size="15" />
            {{ settings.t('terminal.goHome') }}
          </button>
        </div>
      </div>
    </div>

    <footer class="terminal-status">
      <div class="terminal-status__left">
        <span class="status-text" :class="{ 'status-text--running': running }">
          <span class="status-dot" :class="running ? 'status-dot--busy' : connection.isConnected ? 'status-dot--online' : 'status-dot--error'" />
          {{ statusText }}
        </span>
        <span v-if="lastCommand" class="terminal-status__command mono">{{ lastCommand }}</span>
      </div>
      <div class="terminal-status__hint">
        <span class="kbd">Enter</span> {{ settings.t('terminal.hint') }}
      </div>
      <div class="terminal-status__right mono">
        <span>xterm.js</span>
        <span>·</span>
        <span>{{ connection.clientVersion || 'pm3' }}</span>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.terminal-page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  height: 100dvh;
  padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom)
    env(safe-area-inset-left);
  background:
    radial-gradient(900px 500px at 80% -10%, var(--accent-faint), transparent 62%),
    var(--bg-deep);
  overflow: hidden;
}

.terminal-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  height: 58px;
  padding: 0 14px;
  border-bottom: 1px solid var(--border);
  background: rgba(8, 13, 22, 0.8);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  flex: none;
}

.terminal-back {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 9px;
  border: 1px solid var(--border);
  color: var(--muted);
  flex: none;
}

.terminal-back:hover {
  color: var(--text);
  border-color: var(--border-strong);
  background: var(--panel-hover);
}

.terminal-brand {
  display: flex;
  align-items: center;
  gap: 9px;
  flex: none;
}

.terminal-brand__text {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
}

.terminal-brand__text span {
  font-size: 12px;
  font-weight: 650;
  color: var(--text);
}

.terminal-brand__text em {
  margin-top: 2px;
  font-family: var(--font-mono);
  font-style: normal;
  font-size: 7.5px;
  letter-spacing: 0.14em;
  color: var(--faint);
}

.terminal-conn {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  margin-left: 10px;
  padding: 5px 10px;
  border-radius: 99px;
  border: 1px solid var(--border-accent);
  background: var(--accent-faint);
  font-size: 10px;
  color: var(--accent);
}

.terminal-conn--offline {
  border-color: var(--border);
  background: var(--panel-inset);
  color: var(--muted);
}

.terminal-conn__text {
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-conn__sep {
  color: var(--faint);
}

.terminal-conn__age {
  font-size: 9px;
  color: var(--muted);
}

.terminal-actions {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
}

.terminal-actions .icon-btn {
  width: 33px;
  height: 33px;
}

.terminal-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  padding: 12px 14px 0;
}

.terminal-window {
  display: flex;
  flex: 1;
  flex-direction: column;
  height: auto;
  min-height: 0;
  border: 1px solid var(--border);
  border-bottom: 0;
  border-radius: 14px 14px 0 0;
  background: #050810;
  overflow: hidden;
  box-shadow: 0 -10px 50px rgba(0, 0, 0, 0.28);
}

.terminal-window__chrome {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 13px;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, rgba(19, 29, 46, 0.72), rgba(11, 18, 30, 0.72));
  flex: none;
}

.chrome-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.chrome-dot--red {
  background: #ff6b81;
  opacity: 0.7;
}
.chrome-dot--amber {
  background: #ffc46b;
  opacity: 0.7;
}
.chrome-dot--green {
  background: #4cf0c8;
  opacity: 0.7;
}

.terminal-window__title {
  margin-left: 7px;
  font-size: 8.5px;
  color: var(--faint);
}

.terminal-window__running {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  font-size: 8.5px;
  color: var(--warning);
}

.terminal-host {
  flex: 1;
  min-height: 0;
  padding: 13px 6px 10px 16px;
  background:
    radial-gradient(900px 500px at 100% 0%, rgba(76, 240, 200, 0.035), transparent 60%),
    #050810;
}

.terminal-host :deep(.xterm) {
  height: 100%;
  padding: 0;
}

.terminal-host :deep(.xterm-viewport) {
  background: transparent !important;
}

.terminal-host :deep(.xterm-screen) {
  width: 100%;
}

.terminal-empty {
  display: grid;
  place-items: center;
  flex: 1;
  padding: 24px;
}

.terminal-empty__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 440px;
  padding: 34px;
  border: 1px solid var(--border);
  border-radius: 20px;
  background: var(--panel);
  text-align: center;
  box-shadow: var(--shadow-panel);
  backdrop-filter: blur(18px);
}

.terminal-empty__icon {
  display: grid;
  place-items: center;
  width: 58px;
  height: 58px;
  margin-bottom: 18px;
  border-radius: 16px;
  border: 1px solid var(--border);
  background: var(--panel-inset);
  color: var(--faint);
}

.terminal-empty__card h1 {
  margin-top: 11px;
  font-size: 24px;
  font-weight: 650;
  letter-spacing: -0.04em;
}

.terminal-empty__card p {
  margin-top: 9px;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--muted);
}

.terminal-empty__actions {
  display: flex;
  gap: 8px;
  margin-top: 22px;
}

.terminal-status {
  display: flex;
  align-items: center;
  gap: 14px;
  height: 30px;
  padding: 0 17px;
  border-top: 1px solid var(--border);
  background: rgba(7, 12, 20, 0.9);
  font-size: 9px;
  color: var(--faint);
  flex: none;
}

.terminal-status__left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.status-text {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--muted);
  white-space: nowrap;
}

.status-text--running {
  color: var(--warning);
}

.terminal-status__command {
  max-width: 280px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--panel-inset);
  color: var(--muted);
  font-size: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-status__hint {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  white-space: nowrap;
}

.terminal-status__right {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 8px;
  white-space: nowrap;
}

.spinning {
  animation: spin 0.9s linear infinite;
}

@media (max-width: 860px) {
  .terminal-bar {
    gap: 8px;
    padding-inline: 9px;
    height: 54px;
  }

  .terminal-brand__text,
  .terminal-conn__sep,
  .terminal-conn__age {
    display: none;
  }

  .terminal-conn {
    margin-left: 0;
    padding: 4px 8px;
  }

  .terminal-conn__text {
    max-width: 120px;
  }

  .terminal-actions {
    gap: 2px;
  }

  .terminal-actions .icon-btn {
    width: 30px;
    height: 30px;
  }

  .terminal-actions .icon-btn:nth-child(2) {
    display: none;
  }

  .terminal-body {
    padding: 6px 6px 0;
  }

  .terminal-window {
    border-radius: 10px 10px 0 0;
  }

  .terminal-host {
    padding: 9px 2px 6px 10px;
  }

  .terminal-status {
    padding: 0 10px;
    gap: 8px;
  }

  .terminal-status__hint {
    display: none;
  }

  .terminal-status__right {
    margin-left: auto;
  }

  .terminal-status__command {
    max-width: 140px;
  }

  .terminal-empty__card {
    padding: 26px 22px;
  }

  .terminal-empty__actions {
    flex-direction: column;
    width: 100%;
  }
}
</style>
