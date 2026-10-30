import type {
  FileEntry,
  InstallResult,
  ListDirResult,
  PathInfo,
  PlatformInfo,
  ReadFileResult,
  ReadMode,
  ReleaseInfo,
  ResourceStatus,
  SerialPortInfo,
} from './backend'

const MOCK_ROOT = 'C:\\Users\\rfid\\AppData\\Roaming\\cn.forestweave.proxmarkstudio\\.proxmark3'
const RELEASE_TAG = 'v0.2.1'
const EXPECTED_COMMIT = 'd596fe6b04fb90356f04433eb82ff1ae354ada40'

const now = Math.floor(Date.now() / 1000)

const mockDirs = new Set<string>([
  '',
  'cmdscripts',
  'cmdscripts/hf',
  'cmdscripts/lf',
  'dictionaries',
  'lualibs',
  'luascripts',
  'pyscripts',
  'resources',
  'resources/fonts',
  'traces',
])

const mockFiles = new Map<string, { bytes: Uint8Array; modified: number }>()

function seeded(seed: number) {
  let state = seed >>> 0
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0
    return state
  }
}

function binaryFile(size: number, seed: number) {
  const bytes = new Uint8Array(size)
  const random = seeded(seed)
  for (let i = 0; i < size; i++) bytes[i] = random() & 0xff
  return bytes
}

function textFile(text: string) {
  return new TextEncoder().encode(text)
}

const mfDump = binaryFile(4096, 0x51f3)
for (let i = 0; i < 64; i++) {
  mfDump[i] = i
}

mockFiles.set('hf_mf_1k_dump.bin', { bytes: mfDump, modified: now - 60 * 42 })
mockFiles.set('hf_mf_1k_dump.json', {
  bytes: textFile(
    JSON.stringify(
      {
        file: { generated: new Date((now - 2500) * 1000).toISOString(), version: '2.0' },
        Card: { UID: 'A1 B2 C3 D4', ATQA: '00 04', SAK: '08', Type: 'MIFARE Classic 1K' },
        blocks: Object.fromEntries(
          Array.from({ length: 16 }, (_, sector) => [
            String(sector),
            {
              blocks: Array.from({ length: 4 }, (_, block) => ({
                block: block,
                data: 'FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF',
              })),
            },
          ]),
        ),
      },
      null,
      2,
    ),
  ),
  modified: now - 60 * 40,
})
mockFiles.set('hf_14a_trace-20261028-2214.log', {
  bytes: textFile(
    [
      '[=] Downloading trace...',
      '[+] Trace length: 182 bytes',
      '[+] saved 182 bytes to hf_14a_trace-20261028-2214',
      '',
      '      Start |        End | Src | Data (! denotes parity error)                                           | CRC | Annotation',
      '------------+------------+-----+---------------------------------------------------------------------+-----+--------------------',
      '          0 |        128 | Rdr | 52(7)                                                               |  ok | WUPA',
      '       2368 |       4532 | Tag | 04  00                                                              |  ok |',
      '       7040 |       9184 | Rdr | 93(7)  70  2d  90                                                      |  ok | SELECT',
      '      11648 |      13824 | Tag | 08  b6  dd                                                           |  ok |',
      '      16384 |      18560 | Rdr | 40(7)  a4  42  18  68  14  c3                                                  |  ok |',
    ].join('\n'),
  ),
  modified: now - 60 * 90,
})
mockFiles.set('notes.md', {
  bytes: textFile(
    `# Capture notes\n\n- Reader: Proxmark3\n- Antenna: long-range\n- Card: MIFARE Classic 1K\n- Keys recovered with \`hf mf autopwn\`\n\nRemember to rotate the default keys before deploying.\n`,
  ),
  modified: now - 60 * 60 * 24 * 3,
})
mockFiles.set('dictionaries/mfc_default_keys.dic', {
  bytes: textFile('FFFFFFFFFFFF\nA0A1A2A3A4A5\nD3F7D3F7D3F7\n000000000000\n'),
  modified: now - 60 * 60 * 24 * 20,
})
mockFiles.set('cmdscripts/hf/hf_mf_autopwn.cmd', {
  bytes: textFile('hf mf autopwn\nhf mf list\n'),
  modified: now - 60 * 60 * 24 * 12,
})
mockFiles.set('luascripts/find_keys.lua', {
  bytes: textFile('-- minimal key probe\nlocal keys = {"FFFFFFFFFFFF", "A0A1A2A3A4A5"}\n'),
  modified: now - 60 * 60 * 24 * 8,
})
mockFiles.set('resources/fonts/README.txt', {
  bytes: textFile('Fonts used by the embedded client rendering layer.\n'),
  modified: now - 60 * 60 * 24 * 32,
})
mockFiles.set('traces/hf_14a_trace-20261021-1902.log', {
  bytes: textFile('[+] trace saved\n[=] 1204 bytes\n'),
  modified: now - 60 * 60 * 24 * 8,
})

let installed = true
let installedAt = now - 60 * 60 * 24 * 4
let mockSerialConnected = false

function entry(path: string, isDir: boolean): FileEntry {
  const name = path.split('/').pop() ?? path
  if (isDir) {
    return { name, path, isDir: true, size: 0, modified: now - 60 * 60 * 24 * 3, extension: '' }
  }
  const file = mockFiles.get(path)
  return {
    name,
    path,
    isDir: false,
    size: file?.bytes.length ?? 0,
    modified: file?.modified ?? now,
    extension: name.includes('.') ? name.split('.').pop()!.toLowerCase() : '',
  }
}

function childrenOf(path: string): FileEntry[] {
  const prefix = path ? `${path}/` : ''
  const result = new Map<string, FileEntry>()
  for (const dir of mockDirs) {
    if (dir === path || dir === '') continue
    if (!dir.startsWith(prefix)) continue
    const remainder = dir.slice(prefix.length)
    if (remainder.includes('/')) continue
    result.set(remainder, entry(dir, true))
  }
  for (const filePath of mockFiles.keys()) {
    if (!filePath.startsWith(prefix)) continue
    const remainder = filePath.slice(prefix.length)
    if (remainder.includes('/')) continue
    result.set(remainder, entry(filePath, false))
  }
  return [...result.values()]
}

function hexDump(bytes: Uint8Array, limit = 262144) {
  const slice = bytes.slice(0, limit)
  const lines: string[] = []
  for (let offset = 0; offset < slice.length; offset += 16) {
    const chunk = slice.slice(offset, offset + 16)
    const hex = [...chunk].map((byte) => byte.toString(16).padStart(2, '0').toUpperCase()).join(' ')
    const ascii = [...chunk].map((byte) => (byte >= 32 && byte < 127 ? String.fromCharCode(byte) : '.')).join('')
    lines.push(`${offset.toString(16).padStart(8, '0')}  ${hex.padEnd(47, ' ')}  ${ascii}`)
  }
  return lines.join('\n')
}

function isBinary(bytes: Uint8Array) {
  const sample = bytes.slice(0, 4096)
  return sample.some((byte) => byte === 0)
}

export async function mockGetPlatformInfo(): Promise<PlatformInfo> {
  return {
    os: 'windows',
    arch: 'x86_64',
    family: 'desktop',
    version: '0.0.0-local',
    tauriVersion: '2.12.4',
    capabilities: {
      usbSerial: true,
      ble: true,
      spp: true,
      share: false,
      nativeFiles: true,
      desktop: true,
    },
  }
}

export async function mockGetPaths(): Promise<PathInfo> {
  return {
    workDir: MOCK_ROOT,
    pm3Dir: MOCK_ROOT,
    resourcesDir: `${MOCK_ROOT}\\share\\proxmark3`,
  }
}

export async function mockListDir(path: string): Promise<ListDirResult> {
  await new Promise((resolve) => setTimeout(resolve, 120))
  const normalized = path.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
  if (normalized && !mockDirs.has(normalized)) {
    throw new Error(`Directory not found: ${normalized}`)
  }
  const parent = normalized ? (normalized.includes('/') ? normalized.slice(0, normalized.lastIndexOf('/')) : '') : null
  return {
    path: normalized,
    parent,
    entries: childrenOf(normalized).sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name)),
  }
}

export async function mockReadFile(path: string, mode: ReadMode): Promise<ReadFileResult> {
  await new Promise((resolve) => setTimeout(resolve, 180))
  const normalized = path.replace(/\\/g, '/').replace(/^\/+/g, '')
  const file = mockFiles.get(normalized)
  if (!file) throw new Error(`File not found: ${normalized}`)
  const name = normalized.split('/').pop() ?? normalized
  const binary = isBinary(file.bytes)
  const limit = mode === 'text' ? 524288 : 262144
  const truncated = file.bytes.length > limit
  const bytes = file.bytes.slice(0, limit)
  const content = mode === 'hex' ? hexDump(bytes, limit) : new TextDecoder('utf-8').decode(bytes)
  return {
    name,
    path: normalized,
    size: file.bytes.length,
    modified: file.modified,
    binary,
    truncated,
    mode,
    content,
  }
}

export async function mockDeletePath(path: string) {
  await new Promise((resolve) => setTimeout(resolve, 160))
  const normalized = path.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
  if (mockDirs.has(normalized)) {
    for (const dir of [...mockDirs]) {
      if (dir === normalized || dir.startsWith(`${normalized}/`)) mockDirs.delete(dir)
    }
    for (const file of [...mockFiles.keys()]) {
      if (file.startsWith(`${normalized}/`)) mockFiles.delete(file)
    }
    return
  }
  mockFiles.delete(normalized)
}

export async function mockMovePath(source: string, target: string, name: string): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 220))
  const from = source.replace(/\\/g, '/').replace(/^\/+/g, '')
  const toDir = target.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
  const destination = `${toDir ? `${toDir}/` : ''}${name}`
  const file = mockFiles.get(from)
  if (!file) throw new Error('Only files can be moved in the preview')
  mockFiles.delete(from)
  mockFiles.set(destination, file)
  return destination
}

export async function mockCreateFolder(parent: string, name: string): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 150))
  const base = parent.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
  const destination = `${base ? `${base}/` : ''}${name}`
  if (mockDirs.has(destination) || mockFiles.has(destination)) throw new Error('A file or folder with this name already exists')
  mockDirs.add(destination)
  return destination
}

export async function mockRevealPath(_path: string) {
  await new Promise((resolve) => setTimeout(resolve, 100))
}

export async function mockListSerialPorts(): Promise<SerialPortInfo[]> {
  await new Promise((resolve) => setTimeout(resolve, 250))
  return [
    {
      path: 'COM7',
      kind: 'usb',
      manufacturer: 'Proxmark',
      product: 'Proxmark3',
      serialNumber: 'PM3-0421',
      vid: 0x9ac4,
      pid: 0x4b8f,
    },
    {
      path: 'COM11',
      kind: 'bluetooth',
      manufacturer: 'Microsoft',
      product: 'Bluetooth SPP · PM3 BT',
      serialNumber: null,
      vid: null,
      pid: null,
    },
    {
      path: 'COM3',
      kind: 'unknown',
      manufacturer: 'wch.cn',
      product: 'USB-SERIAL CH340',
      serialNumber: null,
      vid: 0x1a86,
      pid: 0x7523,
    },
  ]
}

export function mockIsConnected() {
  return mockSerialConnected
}

export function mockSetConnected(value: boolean) {
  mockSerialConnected = value
}

export async function mockCheckResources(): Promise<ResourceStatus> {
  await new Promise((resolve) => setTimeout(resolve, 120))
  const fileCount = [...mockFiles.keys()].filter((file) => !file.startsWith('traces/')).length
  return {
    installed,
    valid: installed,
    path: MOCK_ROOT,
    expectedCommit: EXPECTED_COMMIT,
    commit: installed ? EXPECTED_COMMIT : null,
    version: installed ? 'Iceman/HEAD/v4.23346-227-gd596fe6b0' : null,
    installedAt: installed ? installedAt : null,
    fileCount: installed ? 187 : 0,
    expectedFiles: 6,
  }
}

export async function mockGetReleaseInfo(): Promise<ReleaseInfo | null> {
  await new Promise((resolve) => setTimeout(resolve, 450))
  return {
    tag: RELEASE_TAG,
    name: 'Proxmark3 client resources',
    publishedAt: new Date((now - 60 * 60 * 24 * 4) * 1000).toISOString(),
    htmlUrl: 'https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/tag/v0.2.1',
    assetName: 'proxmark3-client-resources-d596fe6b04.zip',
    assetSize: 9128035,
    checksum: 'f6217ee66cfe531db219e2e9ec16c79efdbfd06faad550f95c0843e6e69885db',
    downloadUrl:
      'https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v0.2.1/proxmark3-client-resources-d596fe6b04.zip',
    manifestUrl:
      'https://github.com/xianglin1998/tauri-plugin-proxmark3/releases/download/v0.2.1/manifest.json',
    clientVersion: 'Iceman/HEAD/v4.23346-227-gd596fe6b0',
    commit: EXPECTED_COMMIT,
  }
}

export async function mockInstallResources(): Promise<InstallResult> {
  await new Promise((resolve) => setTimeout(resolve, 900))
  installed = true
  installedAt = Math.floor(Date.now() / 1000)
  return {
    status: await mockCheckResources(),
    release: (await mockGetReleaseInfo())!,
  }
}
