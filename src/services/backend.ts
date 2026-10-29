import { invoke } from '@tauri-apps/api/core'
import {
  mockCheckResources,
  mockCreateFolder,
  mockDeletePath,
  mockGetPaths,
  mockGetPlatformInfo,
  mockGetReleaseInfo,
  mockInstallResources,
  mockListDir,
  mockListSerialPorts,
  mockMovePath,
  mockReadFile,
  mockRevealPath,
} from './mockBackend'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
export const isAndroid = typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent)

export interface PlatformCapabilities {
  usbSerial: boolean
  ble: boolean
  spp: boolean
  share: boolean
  nativeFiles: boolean
  desktop: boolean
}

export interface PlatformInfo {
  os: string
  arch: string
  family: 'desktop' | 'mobile' | 'web'
  version: string
  tauriVersion: string
  capabilities: PlatformCapabilities
}

export interface PathInfo {
  workDir: string
  pm3Dir: string
  resourcesDir: string
}

export interface SerialPortInfo {
  path: string
  kind: 'usb' | 'bluetooth' | 'unknown'
  manufacturer?: string | null
  product?: string | null
  serialNumber?: string | null
  vid?: number | null
  pid?: number | null
}

export interface FileEntry {
  name: string
  path: string
  isDir: boolean
  size: number
  modified: number
  extension: string
}

export interface ListDirResult {
  path: string
  parent: string | null
  entries: FileEntry[]
}

export type ReadMode = 'text' | 'hex'

export interface ReadFileResult {
  name: string
  path: string
  size: number
  modified: number
  binary: boolean
  truncated: boolean
  mode: ReadMode
  content: string
}

export interface ResourceStatus {
  installed: boolean
  valid: boolean
  path: string
  expectedCommit: string
  commit?: string | null
  version?: string | null
  installedAt?: number | null
  fileCount: number
  expectedFiles: number
}

export interface ReleaseInfo {
  tag: string
  name: string
  publishedAt: string
  htmlUrl: string
  assetName: string
  assetSize: number
  checksum?: string | null
  downloadUrl: string
  manifestUrl: string
  clientVersion: string
  commit: string
}

export interface ResourceProgress {
  stage: 'fetching' | 'downloading' | 'verifying' | 'extracting' | 'done'
  downloaded: number
  total: number
  message?: string
}

export interface InstallResult {
  status: ResourceStatus
  release: ReleaseInfo
}

export interface BridgeInfo {
  port: number
}

function tauntUnavailable(): never {
  throw new Error('This action requires the Proxmark Studio desktop or mobile app.')
}

export async function getPlatformInfo(): Promise<PlatformInfo> {
  if (!isTauri) return mockGetPlatformInfo()
  return invoke<PlatformInfo>('get_platform_info')
}

export async function getPaths(): Promise<PathInfo> {
  if (!isTauri) return mockGetPaths()
  return invoke<PathInfo>('get_paths')
}

export async function listDir(path = ''): Promise<ListDirResult> {
  if (!isTauri) return mockListDir(path)
  return invoke<ListDirResult>('list_dir', { path })
}

export async function readFile(path: string, mode: ReadMode): Promise<ReadFileResult> {
  if (!isTauri) return mockReadFile(path, mode)
  return invoke<ReadFileResult>('read_file', { path, mode })
}

export async function deletePath(path: string): Promise<void> {
  if (!isTauri) return mockDeletePath(path)
  await invoke('delete_path', { path })
}

export async function movePath(source: string, target: string, name: string): Promise<string> {
  if (!isTauri) return mockMovePath(source, target, name)
  return invoke<string>('move_path', { source, target, name })
}

export async function createFolder(parent: string, name: string): Promise<string> {
  if (!isTauri) return mockCreateFolder(parent, name)
  return invoke<string>('create_folder', { parent, name })
}

export async function revealPath(path: string): Promise<void> {
  if (!isTauri) return mockRevealPath(path)
  await invoke('reveal_path', { path })
}

export async function listSerialPorts(): Promise<SerialPortInfo[]> {
  if (!isTauri) return mockListSerialPorts()
  if (isAndroid) {
    const [usbResult, sppResult] = await Promise.allSettled([
      invoke<{ devices: Array<{ path: string; name: string; manufacturer: string; vid: number; pid: number }> }>(
        'plugin:transport|list_usb',
      ),
      invoke<{ devices: Array<{ address: string; name: string }> }>('plugin:transport|list_spp'),
    ])
    const usb = usbResult.status === 'fulfilled' ? usbResult.value : { devices: [] }
    const spp = sppResult.status === 'fulfilled' ? sppResult.value : { devices: [] }
    return [
      ...usb.devices.map((device) => ({
        path: device.path,
        kind: 'usb' as const,
        manufacturer: device.manufacturer || null,
        product: device.name || null,
        serialNumber: null,
        vid: device.vid ?? null,
        pid: device.pid ?? null,
      })),
      ...spp.devices.map((device) => ({
        path: device.address,
        kind: 'bluetooth' as const,
        manufacturer: null,
        product: device.name || null,
        serialNumber: null,
        vid: null,
        pid: null,
      })),
    ]
  }
  return invoke<SerialPortInfo[]>('list_serial_ports')
}

export async function startBleBridge(): Promise<BridgeInfo> {
  if (!isTauri) tauntUnavailable()
  return invoke<BridgeInfo>('ble_bridge_start')
}

export async function stopBleBridge(): Promise<void> {
  if (!isTauri) return
  await invoke('ble_bridge_stop')
}

export async function writeBleBridge(data: string): Promise<void> {
  if (!isTauri) return
  await invoke('ble_bridge_write', { data })
}

export async function checkResources(): Promise<ResourceStatus> {
  if (!isTauri) return mockCheckResources()
  return invoke<ResourceStatus>('check_resources')
}

export async function getReleaseInfo(): Promise<ReleaseInfo | null> {
  if (!isTauri) return mockGetReleaseInfo()
  return invoke<ReleaseInfo | null>('get_release_info')
}

export async function installResources(): Promise<InstallResult> {
  if (!isTauri) return mockInstallResources()
  return invoke<InstallResult>('install_resources')
}

export async function getDiagnostics(): Promise<string> {
  if (!isTauri) {
    const info = await mockGetPlatformInfo()
    return [
      `Proxmark Studio ${info.version}`,
      `OS: ${info.os} (${info.arch})`,
      `Plugin client commit: ${(await mockCheckResources()).expectedCommit}`,
      `Workspace: ${(await mockGetPaths()).pm3Dir}`,
    ].join('\n')
  }
  return invoke<string>('get_diagnostics')
}

export async function openExternal(url: string): Promise<void> {
  if (!isTauri) {
    window.open(url, '_blank', 'noopener,noreferrer')
    return
  }
  const { openUrl } = await import('@tauri-apps/plugin-opener')
  await openUrl(url)
}
