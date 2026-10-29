import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isTauri } from './backend'
import { mockSetConnected } from './mockBackend'

export interface SessionInfo {
  open: boolean
  offline: boolean
  port: string
  version: string
}

export interface CommandOutput {
  status: number
  output: string
  durationMs: number
}

interface OutputChunk {
  chunk: string
}

export const MOCK_CLIENT_VERSION = 'Iceman/HEAD/v4.23346-227-gd596fe6b0'

let mockSession: SessionInfo = { open: false, offline: false, port: '', version: MOCK_CLIENT_VERSION }

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

function stripAnsi(value: string) {
  return value
    .replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, '')
    .replace(/\x1b\[[0-9;?]*[@-~]/g, '')
    .replace(/\r(?!\n)/g, '\n')
}

function mockOutput(command: string): string {
  const normalized = command.trim()
  if (normalized === 'hw version' || normalized === 'hw ver') {
    return [
      ' [ Proxmark3 RFID instrument ]',
      '',
      ' [ CLIENT ]',
      `  client: ${MOCK_CLIENT_VERSION}`,
      '  compiler: GCC 14.2.0 (MinGW-w64)',
      '  platform: x86_64 / Windows',
      '',
      ' [ ARM ]',
      '  bootrom: v4.23346-227-gd596fe6b0',
      '  firmware: v4.23346-227-gd596fe6b0',
      '  target: PM3 GENERIC',
      '  MCU: AT91SAM7S512',
      '  memory: 512 KB flash / 64 KB SRAM',
      '  protocol: 4.23346',
      '',
      ' [ Hardware ]',
      '  Proxmark3 with Bluetooth bridge',
      '  FPGA: Spartan-6 XC6SLX25',
      '  LF antenna: 125 kHz / 134 kHz',
      '  HF antenna: 13.56 MHz',
      '',
      ' [ Done ]',
    ].join('\n')
  }
  if (normalized === 'hw status') {
    return [
      '[=] --- Hardware ---',
      '[=]  Firmware................. PM3 GENERIC',
      '[=]  Bootrom.................. v4.23346',
      '[=]  Target................... PM3 GENERIC',
      '[=]  MCU...................... AT91SAM7S512',
      '[=]  FPGA..................... Spartan-6',
      '[=]  Protocol................. 4.23346',
      '[=]  Bluetooth................ present',
      '[+] Hardware status: OK',
    ].join('\n')
  }
  if (normalized === 'hw ping') return '[+] ping ok'
  if (normalized === 'help') {
    return [
      ' [ Proxmark3 command reference ]',
      '',
      '  General................. help, hw, data, exit, quit',
      '  High frequency.......... hf search, hf 14a, hf mf, hf iclass, hf mfu',
      '  Low frequency........... lf search, lf em, lf hid, lf t55xx',
      '  Scripting............... script run, lua, python',
      '  Traces.................. trace list, trace save, trace load',
      '',
      '  Use "help <command>" for detailed usage.',
    ].join('\n')
  }
  if (normalized === 'hf search') {
    return [
      '[=] Checking for known tags...',
      '[=] ',
      '[+] Valid ISO 14443-A tag found',
      '[+] UID: A1 B2 C3 D4',
      '[+] ATQA: 00 04',
      '[+] SAK: 08',
      '[+] TYPE: MIFARE Classic 1K',
      '[+] MANUFACTURER: NXP Semiconductors',
      '',
      '[+] Done!',
    ].join('\n')
  }
  if (normalized.startsWith('hf mf autopwn')) {
    return [
      '[=] Executing autopwn on MIFARE Classic 1K',
      '[=] Sector 0 key A... FFFFFFFFFFFF (found)',
      '[=] Sector 1 key A... A0A1A2A3A4A5 (found)',
      '[=] Sector 2 key A... D3F7D3F7D3F7 (found)',
      '[=] Sector 3 key B... FFFFFFFFFFFF (found)',
      '[+] Found 4/4 sector keys',
      '[+] Dump saved to hf-mf-A1B2C3D4-dump.bin',
    ].join('\n')
  }
  if (normalized === 'exit' || normalized === 'quit') return '[=] Session will close after this command.'
  if (normalized === '') return ''
  return `[!] unknown command: ${normalized}\n[=] type "help" for a list of commands`
}

function mockChunks(output: string): string[] {
  const lines = output.split('\n')
  const chunks: string[] = []
  for (const [index, line] of lines.entries()) {
    const prefix = line.startsWith('[+]')
      ? '\x1b[32m'
      : line.startsWith('[=]')
        ? '\x1b[36m'
        : line.startsWith('[!]')
          ? '\x1b[31m'
          : ''
    chunks.push(`${prefix}${line}\x1b[0m${index === lines.length - 1 ? '' : '\r\n'}`)
  }
  return chunks
}

export async function connect(port?: string | null): Promise<SessionInfo> {
  if (!isTauri) {
    await delay(900)
    if (!port) throw new Error('Select a serial port or Bluetooth device first.')
    mockSession = { open: true, offline: false, port, version: MOCK_CLIENT_VERSION }
    mockSetConnected(true)
    return { ...mockSession }
  }
  return invoke<SessionInfo>('plugin:proxmark3|connect', { port: port ?? null })
}

export async function disconnect(): Promise<SessionInfo> {
  if (!isTauri) {
    await delay(280)
    mockSession = { open: false, offline: false, port: '', version: MOCK_CLIENT_VERSION }
    mockSetConnected(false)
    return { ...mockSession }
  }
  return invoke<SessionInfo>('plugin:proxmark3|disconnect')
}

export async function execute(command: string): Promise<CommandOutput> {
  if (!isTauri) {
    await delay(320)
    if (!mockSession.open) throw new Error('No Proxmark3 session is open')
    const started = performance.now()
    const raw = mockOutput(command)
    return {
      status: raw.startsWith('[!]') ? 1 : 0,
      output: stripAnsi(raw),
      durationMs: Math.round(performance.now() - started),
    }
  }
  return invoke<CommandOutput>('plugin:proxmark3|execute', { command })
}

export async function executeStream(
  command: string,
  onChunk: (chunk: string) => void,
): Promise<CommandOutput> {
  if (!isTauri) {
    if (!mockSession.open) throw new Error('No Proxmark3 session is open')
    const started = performance.now()
    const raw = mockOutput(command)
    let collected = ''
    const chunks = mockChunks(raw)
    for (const [index, chunk] of chunks.entries()) {
      await delay(index === 0 ? 80 : 55)
      collected += chunk
      onChunk(chunk)
    }
    return {
      status: raw.startsWith('[!]') ? 1 : 0,
      output: stripAnsi(collected),
      durationMs: Math.round(performance.now() - started),
    }
  }

  const unlisten = await listen<OutputChunk>('proxmark3://output', (event) => {
    onChunk(event.payload.chunk)
  })
  try {
    return await invoke<CommandOutput>('plugin:proxmark3|execute_stream', { command })
  } finally {
    unlisten()
  }
}

export async function status(): Promise<SessionInfo> {
  if (!isTauri) return { ...mockSession }
  return invoke<SessionInfo>('plugin:proxmark3|status')
}

export async function version(): Promise<string> {
  if (!isTauri) return MOCK_CLIENT_VERSION
  return invoke<string>('plugin:proxmark3|version')
}
