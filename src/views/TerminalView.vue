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
import { executeStream, sendInput } from '@/services/pm3'
import { copyText, errorMessage } from '@/utils/format'

const router = useRouter()
const connection = useConnectionStore()
const settings = useSettingsStore()
const toast = useToastStore()

const terminalHost = ref<HTMLDivElement | null>(null)
const terminalPage = ref<HTMLDivElement | null>(null)
const terminalReady = ref(false)
const running = ref(false)
const lastCommand = ref('')
const historyCount = ref(0)
const fullscreen = ref(false)
const reconnecting = ref(false)
const fontSize = ref(window.innerWidth < 640 ? 14 : 13.5)
const ctrlArmed = ref(false)
const shiftArmed = ref(false)

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let resizeObserver: ResizeObserver | null = null
let viewportFrame: number | null = null
let lineBuffer = ''
let runningBuffer = ''
let colorPending = ''
let cursorPos = 0
let history: string[] = []
let historyIndex = -1

const OUTPUT_COLORS: Record<string, string> = {
  '+': '\x1b[38;2;76;240;200m',
  '=': '\x1b[38;2;106;168;255m',
  '!': '\x1b[38;2;255;107;129m',
  '#': '\x1b[38;2;125;155;195m',
  '*': '\x1b[38;2;255;196;107m',
  '-': '\x1b[38;2;255;196;107m',
  '?': '\x1b[38;2;177;140;255m',
}

// The client disables its own ANSI colours when stdout is redirected. Restore
// the familiar pm3 severity colours from the leading [x] marker instead.
function colorizeLine(line: string): string {
  if (line.includes('\x1b[')) return line
  const match = line.match(/^(\s*)\[([+=\!#*?-])\](.*)$/)
  if (!match) return line
  const color = OUTPUT_COLORS[match[2]]
  if (!color) return line
  return `${match[1]}${color}[${match[2]}]${match[3]}\x1b[0m`
}

function writeOutput(chunk: string) {
  if (!term) return
  colorPending += chunk
  for (;;) {
    const index = colorPending.search(/[\r\n]/)
    if (index < 0) break
    const line = colorPending.slice(0, index)
    const isCrLf = colorPending[index] === '\r' && colorPending[index + 1] === '\n'
    const separator = colorPending[index]
    colorPending = colorPending.slice(index + (isCrLf ? 2 : 1))
    term.write(colorizeLine(line))
    term.write(isCrLf || separator === '\n' ? '\r\n' : '\r')
  }
}

function flushOutput() {
  if (!term || !colorPending) return
  term.write(colorizeLine(colorPending))
  colorPending = ''
}

const prompt = '\x1b[38;2;76;240;200m pm3 \x1b[0m\x1b[38;2;86;102;127m›\x1b[0m '

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
  if (connection.isConnected) term.write(prompt)
}

function renderLine() {
  if (!term) return
  term.write('\r\x1b[2K')
  term.write(prompt)
  term.write(lineBuffer)
  const back = lineBuffer.length - cursorPos
  if (back > 0) term.write(`\x1b[${back}D`)
}

function replaceCurrentLine(next: string) {
  lineBuffer = next
  cursorPos = next.length
  renderLine()
}

function insertText(text: string) {
  if (!text) return
  lineBuffer = lineBuffer.slice(0, cursorPos) + text + lineBuffer.slice(cursorPos)
  cursorPos += text.length
  renderLine()
}

function moveCursor(delta: number) {
  cursorPos = Math.max(0, Math.min(lineBuffer.length, cursorPos + delta))
  renderLine()
}

function setFontSize(next: number) {
  fontSize.value = Math.min(24, Math.max(10, next))
  if (term) term.options.fontSize = fontSize.value
  requestAnimationFrame(() => {
    fitAddon?.fit()
    requestAnimationFrame(() => {
      fitAddon?.fit()
      term?.scrollToBottom()
    })
  })
}

function syncViewport() {
  if (!terminalPage.value) return
  const viewport = window.visualViewport
  if (!viewport) {
    terminalPage.value.style.height = ''
  } else {
    const top = Math.max(0, terminalPage.value.getBoundingClientRect().top - viewport.offsetTop)
    terminalPage.value.style.height = `${Math.max(0, viewport.height - top)}px`
  }
  if (viewportFrame === null) {
    viewportFrame = window.requestAnimationFrame(() => {
      viewportFrame = null
      fitAddon?.fit()
    })
  }
}

function installViewportWatcher() {
  window.addEventListener('resize', syncViewport)
  window.visualViewport?.addEventListener('resize', syncViewport)
  window.visualViewport?.addEventListener('scroll', syncViewport)
  syncViewport()
}

function removeViewportWatcher() {
  window.removeEventListener('resize', syncViewport)
  window.visualViewport?.removeEventListener('resize', syncViewport)
  window.visualViewport?.removeEventListener('scroll', syncViewport)
  if (viewportFrame !== null) window.cancelAnimationFrame(viewportFrame)
  viewportFrame = null
  if (terminalPage.value) terminalPage.value.style.height = ''
}

function toolbarKey(key: string) {
  term?.focus()
  if (key === 'ctrl') {
    ctrlArmed.value = !ctrlArmed.value
    term?.focus()
    return
  }
  if (key === 'shift') {
    shiftArmed.value = !shiftArmed.value
    term?.focus()
    return
  }
  if (key === 'left') {
    moveCursor(-1)
    return
  }
  if (key === 'right') {
    moveCursor(1)
    return
  }
  if (key === 'up' || key === 'down') {
    void onTerminalData(key === 'up' ? '\u001b[A' : '\u001b[B')
    return
  }
  if (key === 'escape') {
    if (running.value) void sendInput('\x1b')
    else {
      lineBuffer = ''
      cursorPos = 0
      renderLine()
    }
    return
  }
  if (key === 'tab') {
    if (running.value) void sendInput('\t')
    else insertText('  ')
    return
  }
  if (key === 'pipe') insertText('|')
  if (key === 'tilde') insertText('~')
}

function initialiseTerminal() {
  if (!terminalHost.value || term) return
  term = new Terminal({
    cursorBlink: true,
    cursorStyle: 'bar',
    fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim() || 'monospace',
    fontSize: fontSize.value,
    lineHeight: 1.4,
    letterSpacing: 0.1,
    scrollback: 8000,
    scrollSensitivity: 3,
    smoothScrollDuration: 0,
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

async function onTerminalData(data: string) {
  if (!term) return

  if (data.length === 1 && ctrlArmed.value) {
    ctrlArmed.value = false
    const code = data.toUpperCase().charCodeAt(0)
    data = code >= 64 && code <= 95 ? String.fromCharCode(code - 64) : data
  } else if (data.length === 1 && shiftArmed.value) {
    shiftArmed.value = false
    data = data.toUpperCase()
  }

  // Keep accepting input while a command is running. Some client commands
  // (tune/sniff/simulation loops) only exit when they receive Enter, and a few
  // prompts read a short answer from stdin.
  if (running.value) {
    if (!connection.isConnected) return
    if (data === '\t') {
      void sendInput('\t')
      return
    }
    if (data === '\r') {
      const payload = `${runningBuffer}\n`
      runningBuffer = ''
      term.write('\r\n')
      void sendInput(payload).catch((error) => {
        toast.error(settings.t('terminal.commandError'), errorMessage(error))
      })
      return
    }
    if (data === '\u007F') {
      if (!runningBuffer.length) return
      runningBuffer = runningBuffer.slice(0, -1)
      term.write('\b \b')
      return
    }
    if (data === '\u0003') {
      runningBuffer = ''
      term.write('^C')
      void sendInput('\n').catch(() => {})
      return
    }
    if (data.startsWith('\u001b')) return
    const printable = [...data].filter((char) => char >= ' ' && char !== '\u007f').join('')
    if (!printable) return
    runningBuffer += printable
    term.write(printable)
    return
  }

  if (!connection.isConnected) return

  if (data === '\r') {
    const command = lineBuffer.trim()
    lineBuffer = ''
    cursorPos = 0
    term.write('\r\n')
    if (!command) {
      writePrompt()
      return
    }
    await runCommand(command)
    return
  }

  if (data === '\u007F') {
    if (!cursorPos) return
    lineBuffer = lineBuffer.slice(0, cursorPos - 1) + lineBuffer.slice(cursorPos)
    cursorPos -= 1
    renderLine()
    return
  }

  if (data === '\u0003') {
    lineBuffer = ''
    cursorPos = 0
    term.write('^C\r\n')
    writePrompt()
    return
  }

  if (data === '\u000C') {
    cursorPos = 0
    term.clear()
    writePrompt()
    return
  }

  if (data === '\u001b[D') {
    moveCursor(-1)
    return
  }

  if (data === '\u001b[C') {
    moveCursor(1)
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

  // Ignore remaining escape/control sequences. Printable input is inserted at
  // the current cursor position so toolbar arrows behave like a real shell.
  if (data.length > 1 && data.startsWith('\u001b')) return
  const printable = [...data].filter((char) => char >= ' ' && char !== '\u007f').join('')
  if (!printable) return
  insertText(printable)
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
  runningBuffer = ''
  colorPending = ''
  let endedWithNewline = false

  try {
    await executeStream(normalized, (chunk) => {
      endedWithNewline = chunk.endsWith('\n')
      writeOutput(chunk)
    })
    if (colorPending) {
      flushOutput()
      term.write('\r\n')
    } else if (!endedWithNewline) {
      term.write('\r\n')
    }
  } catch (error) {
    flushOutput()
    if (!endedWithNewline) term.write('\r\n')
    term.writeln(`\x1b[31m[!] ${settings.t('terminal.commandError')}: ${errorMessage(error)}\x1b[0m`)
    if (/not open|no proxmark|session/i.test(errorMessage(error))) {
      await connection.refreshSession()
    }
  } finally {
    running.value = false
    runningBuffer = ''
    colorPending = ''
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
      lineBuffer = ''
      cursorPos = 0
      historyIndex = -1
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
  document.addEventListener('fullscreenchange', onFullscreenChange)
  installViewportWatcher()
  if (connection.isConnected) {
    await nextTick()
    initialiseTerminal()
  }
})

onUnmounted(() => {
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  removeViewportWatcher()
  resizeObserver?.disconnect()
  term?.dispose()
  term = null
  fitAddon = null
})
</script>

<template>
  <div ref="terminalPage" class="terminal-page">
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

    <div v-if="connection.isConnected" class="terminal-toolbar" role="toolbar">
      <div class="terminal-toolbar__keys">
        <button class="terminal-toolbar__key" @click="toolbarKey('escape')">Esc</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('tab')">Tab</button>
        <button class="terminal-toolbar__key" :class="{ 'is-active': ctrlArmed }" @click="toolbarKey('ctrl')">Ctrl</button>
        <button class="terminal-toolbar__key" :class="{ 'is-active': shiftArmed }" @click="toolbarKey('shift')">Shift</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('left')">←</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('up')">↑</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('down')">↓</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('right')">→</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('pipe')">|</button>
        <button class="terminal-toolbar__key" @click="toolbarKey('tilde')">~</button>
      </div>
      <div class="terminal-toolbar__fontbar">
        <button class="terminal-toolbar__key terminal-toolbar__font" @click="setFontSize(fontSize - 1)">A−</button>
        <span class="terminal-toolbar__size mono">{{ Math.round(fontSize) }}</span>
        <button class="terminal-toolbar__key terminal-toolbar__font" @click="setFontSize(fontSize + 1)">A+</button>
      </div>
    </div>

    <div v-if="connection.isConnected" class="terminal-body">
      <div class="terminal-window">
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
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto minmax(0, 1fr) auto;
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
  grid-row: 1;
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

.terminal-toolbar {
  grid-row: 2;
  display: none;
  min-width: 0;
  width: 100%;
  align-items: flex-start;
  gap: 4px;
  flex: none;
  padding: 6px 8px;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
  background: rgba(8, 13, 22, 0.94);
}

.terminal-toolbar__keys {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-wrap: wrap;
  gap: 5px;
  overflow: visible;
}

.terminal-toolbar__fontbar {
  display: flex;
  flex: none;
  align-items: center;
  gap: 2px;
  padding-left: 4px;
  border-left: 1px solid var(--border);
}

.terminal-toolbar__key {
  flex: none;
  min-width: 34px;
  height: 30px;
  padding: 0 7px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--panel);
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 11px;
  touch-action: manipulation;
}

.terminal-toolbar__key:active,
.terminal-toolbar__key.is-active {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-faint);
}

.terminal-toolbar__font {
  min-width: 30px;
  font-size: 12px;
}

.terminal-toolbar__size {
  flex: none;
  min-width: 20px;
  color: var(--faint);
  font-size: 10px;
  text-align: center;
}

@media (max-width: 860px) {
  .terminal-toolbar {
    display: flex;
  }
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
  font-size: 10.5px;
  letter-spacing: 0.1em;
  color: var(--faint);
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
  grid-row: 3;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  padding: 0;
  touch-action: pan-y;
}

.terminal-window {
  display: flex;
  flex: 1;
  flex-direction: column;
  height: auto;
  min-height: 0;
  border: 0;
  border-radius: 0;
  background: #050810;
  overflow: hidden;
  touch-action: pan-y;
}

.terminal-host {
  flex: 1;
  min-height: 0;
  padding: 0;
  background:
    radial-gradient(900px 500px at 100% 0%, var(--accent-faint), transparent 60%),
    #050810;
}

.terminal-host :deep(.xterm) {
  height: 100%;
  padding: 13px 6px 10px 16px;
}

.terminal-host :deep(.xterm-viewport) {
  background: transparent !important;
  overscroll-behavior: contain;
  -webkit-overflow-scrolling: touch;
  scroll-behavior: auto;
  touch-action: pan-y;
}

.terminal-host :deep(.xterm),
.terminal-host :deep(.xterm-screen) {
  touch-action: pan-y;
}

.terminal-host :deep(.xterm-screen) {
  width: 100%;
}

.terminal-empty {
  grid-row: 3;
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
  grid-row: 4;
  display: flex;
  align-items: center;
  gap: 14px;
  height: 30px;
  padding: 0 17px;
  border-top: 1px solid var(--border);
  background: rgba(7, 12, 20, 0.9);
  font-size: 11.5px;
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
  font-size: 11px;
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
  font-size: 11px;
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

  .terminal-brand__text {
    display: none;
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
    padding: 0;
  }

  .terminal-host {
    padding: 0;
  }

  .terminal-host :deep(.xterm) {
    padding: 9px 2px 18px 10px;
  }

  .terminal-status {
    position: relative;
    z-index: 2;
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
