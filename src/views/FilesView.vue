<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  AlertTriangle,
  ArrowLeft,
  ArrowUp,
  Check,
  ChevronRight,
  Copy,
  Eye,
  File,
  FileArchive,
  FileCode2,
  FileJson,
  FileText,
  FileUp,
  Folder,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Globe2,
  HardDrive,
  Home,
  Loader2,
  MapPin,
  MoreHorizontal,
  MoreVertical,
  RefreshCw,
  Search,
  Server,
  Share2,
  ShieldAlert,
  Trash2,
  Upload,
  WrapText,
  X,
} from 'lucide-vue-next'
import Modal from '@/components/Modal.vue'
import { useFilesStore } from '@/stores/files'
import { usePlatformStore } from '@/stores/platform'
import { useSettingsStore } from '@/stores/settings'
import { useToastStore } from '@/stores/toast'
import {
  exportWorkspaceZip,
  importFiles,
  isAndroid,
  isTauri,
  listDir,
  listNetworkInterfaces,
  openExternal,
  pickExportDirectory,
  readFile,
  revealPath,
  shareFile,
  startHttpServer,
  stopHttpServer,
  type FileEntry,
  type HttpInfo,
  type ImportFilePayload,
  type NetworkInterface,
  type ReadFileResult,
  type ReadMode,
} from '@/services/backend'
import { copyText, errorMessage, formatBytes, formatDate, shortPath } from '@/utils/format'

const files = useFilesStore()
const platform = usePlatformStore()
const settings = useSettingsStore()
const toast = useToastStore()

const selected = ref<FileEntry | null>(null)
const menu = ref<{ entry: FileEntry; x: number; y: number } | null>(null)

const viewerOpen = ref(false)
const viewerEntry = ref<FileEntry | null>(null)
const viewerLoading = ref(false)
const viewerMode = ref<ReadMode>('text')
const viewer = ref<ReadFileResult | null>(null)
const viewerError = ref('')
const wrapLines = ref(false)

const deleteOpen = ref(false)
const deleteEntry = ref<FileEntry | null>(null)
const deleteBusy = ref(false)

const moveOpen = ref(false)
const moveEntry = ref<FileEntry | null>(null)
const moveTarget = ref('')
const moveDirs = ref<FileEntry[]>([])
const moveLoading = ref(false)
const moveBusy = ref(false)
const moveName = ref('')
const moveError = ref('')

const folderOpen = ref(false)
const folderName = ref('')
const folderBusy = ref(false)
const folderError = ref('')

const shareSupported = computed(() => platform.info.capabilities.share)
const isRealApp = isTauri
const importInput = ref<HTMLInputElement | null>(null)
const importBusy = ref(false)
const moreOpen = ref(false)
const exportOpen = ref(false)
const exportBusy = ref(false)
const exportResult = ref('')
const exportProgress = ref({ done: 0, total: 0, file: '' })
const exportPercent = computed(() =>
  exportProgress.value.total
    ? Math.min(100, Math.round((exportProgress.value.done / exportProgress.value.total) * 100))
    : 0,
)
const httpConfirmOpen = ref(false)
const httpInfo = ref<HttpInfo | null>(null)
const httpOpen = ref(false)
const httpBusy = ref(false)
const httpError = ref('')
const httpInterfaces = ref<NetworkInterface[]>([])
const httpBindAddress = ref('0.0.0.0')
const httpPassword = ref('')

function absolutePath(relative: string): string {
  const root = platform.paths.pm3Dir
  if (!root) return relative
  const separator = root.includes('\\') ? '\\' : '/'
  return `${root.replace(/[\\/]+$/, '')}${separator}${relative.split('/').join(separator)}`
}

function dateLabel(value: number) {
  return formatDate(value, settings.resolvedLocale)
}

function iconFor(entry: FileEntry) {
  if (entry.isDir) return Folder
  switch (entry.extension) {
    case 'json':
      return FileJson
    case 'lua':
    case 'py':
    case 'cmd':
    case 'sh':
      return FileCode2
    case 'zip':
    case 'gz':
    case '7z':
    case 'tar':
      return FileArchive
    case 'txt':
    case 'log':
    case 'md':
    case 'dic':
      return FileText
    default:
      return File
  }
}

function accentFor(entry: FileEntry) {
  if (entry.isDir) return 'folder'
  switch (entry.extension) {
    case 'json':
      return 'json'
    case 'lua':
    case 'py':
    case 'cmd':
      return 'code'
    case 'txt':
    case 'log':
    case 'md':
      return 'text'
    case 'bin':
    case 'elf':
      return 'binary'
    default:
      return 'file'
  }
}

function isProbablyBinary(entry: FileEntry) {
  return ['bin', 'elf', 'zip', 'gz', '7z', 'tar', 'png', 'jpg', 'jpeg', 'webp', 'pdf'].includes(
    entry.extension,
  )
}

async function openEntry(entry: FileEntry) {
  selected.value = entry
  if (entry.isDir) {
    await files.load(entry.path)
    return
  }
  await openViewer(entry)
}

async function openViewer(entry: FileEntry) {
  viewerEntry.value = entry
  viewerOpen.value = true
  viewer.value = null
  viewerError.value = ''
  viewerMode.value = isProbablyBinary(entry) ? 'hex' : 'text'
  await loadViewer()
}

async function loadViewer() {
  if (!viewerEntry.value) return
  viewerLoading.value = true
  viewerError.value = ''
  try {
    const result = await readFile(viewerEntry.value.path, viewerMode.value, 'workspace')
    if (result.binary && viewerMode.value === 'text') {
      viewerMode.value = 'hex'
      const hex = await readFile(viewerEntry.value.path, 'hex', 'workspace')
      viewer.value = hex
    } else {
      viewer.value = result
    }
  } catch (error) {
    viewerError.value = errorMessage(error)
  } finally {
    viewerLoading.value = false
  }
}

async function switchViewerMode(mode: ReadMode) {
  if (viewerMode.value === mode) return
  viewerMode.value = mode
  await loadViewer()
}

function toggleMenu(event: MouseEvent, entry: FileEntry) {
  event.stopPropagation()
  const target = event.currentTarget as HTMLElement
  const rect = target.getBoundingClientRect()
  const width = 196
  const height = 218
  menu.value = {
    entry,
    x: Math.max(10, Math.min(rect.right - width, window.innerWidth - width - 10)),
    y: Math.max(10, Math.min(rect.bottom + 6, window.innerHeight - height - 10)),
  }
}

function closeMenu() {
  menu.value = null
}

function menuAction(action: 'view' | 'share' | 'reveal' | 'move' | 'delete') {
  const entry = menu.value?.entry
  closeMenu()
  if (!entry) return
  if (action === 'view') void openViewer(entry)
  else if (action === 'share') void shareEntry(entry)
  else if (action === 'reveal') void revealEntry(entry)
  else if (action === 'move') void openMove(entry)
  else if (action === 'delete') openDelete(entry)
}

function mimeFor(entry: FileEntry): string {
  switch (entry.extension) {
    case 'json':
      return 'application/json'
    case 'txt':
    case 'md':
    case 'log':
    case 'lua':
    case 'py':
    case 'cmd':
      return 'text/plain'
    case 'zip':
      return 'application/zip'
    case 'pdf':
      return 'application/pdf'
    case 'png':
      return 'image/png'
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg'
    default:
      return 'application/octet-stream'
  }
}

async function shareEntry(entry: FileEntry) {
  if (!shareSupported.value) {
    await revealEntry(entry)
    return
  }
  try {
    await shareFile(absolutePath(entry.path), mimeFor(entry), entry.name)
    toast.info(settings.t('toast.shareOpened'))
  } catch (error) {
    toast.error(settings.t('common.error'), errorMessage(error))
  }
}

async function revealEntry(entry: FileEntry) {
  try {
    await revealPath(entry.path)
    toast.info(settings.t('toast.revealed'))
  } catch (error) {
    toast.error(settings.t('common.error'), errorMessage(error))
  }
}

async function copyViewerContent() {
  if (!viewer.value) return
  try {
    await copyText(viewer.value.content)
    toast.success(settings.t('toast.copied'))
  } catch {
    toast.error(settings.t('toast.copyFailed'))
  }
}

function openDelete(entry: FileEntry) {
  deleteEntry.value = entry
  deleteOpen.value = true
}

async function confirmDelete() {
  if (!deleteEntry.value) return
  deleteBusy.value = true
  try {
    await files.remove(deleteEntry.value)
    if (viewerEntry.value?.path === deleteEntry.value.path) viewerOpen.value = false
    deleteOpen.value = false
  } catch {
    /* store reports */
  } finally {
    deleteBusy.value = false
  }
}

function parentOf(path: string) {
  return path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : ''
}

async function openMove(entry: FileEntry) {
  moveEntry.value = entry
  moveName.value = entry.name
  moveError.value = ''
  moveOpen.value = true
  await loadMoveDir(parentOf(entry.path))
}

async function loadMoveDir(path: string) {
  moveLoading.value = true
  moveTarget.value = path
  try {
    const result = await listDir(path)
    moveDirs.value = result.entries.filter((entry) => entry.isDir)
  } catch (error) {
    moveError.value = errorMessage(error)
  } finally {
    moveLoading.value = false
  }
}

function enterMoveDir(entry: FileEntry) {
  if (moveEntry.value?.isDir && (entry.path === moveEntry.value.path || entry.path.startsWith(`${moveEntry.value.path}/`))) {
    return
  }
  void loadMoveDir(entry.path)
}

const moveBreadcrumb = computed(() => {
  const crumbs = [{ name: settings.t('files.root'), path: '' }]
  if (!moveTarget.value) return crumbs
  let accumulated = ''
  for (const part of moveTarget.value.split('/')) {
    accumulated = accumulated ? `${accumulated}/${part}` : part
    crumbs.push({ name: part, path: accumulated })
  }
  return crumbs
})

const moveDisabled = computed(() => {
  if (!moveEntry.value || moveBusy.value) return true
  if (!moveName.value.trim() || moveName.value.includes('/') || moveName.value.includes('\\')) return true
  if (moveEntry.value.isDir) {
    if (moveTarget.value === moveEntry.value.path) return true
    if (moveTarget.value.startsWith(`${moveEntry.value.path}/`)) return true
  }
  return false
})

async function confirmMove() {
  if (!moveEntry.value || moveDisabled.value) return
  moveBusy.value = true
  moveError.value = ''
  try {
    await files.move(moveEntry.value, moveTarget.value, moveName.value.trim())
    moveOpen.value = false
  } catch (error) {
    moveError.value = errorMessage(error)
  } finally {
    moveBusy.value = false
  }
}

function openImport() {
  importInput.value?.click()
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(reader.error ?? new Error('Could not read the selected file.'))
    reader.onload = () => {
      const result = String(reader.result ?? '')
      resolve(result.includes(',') ? result.slice(result.indexOf(',') + 1) : result)
    }
    reader.readAsDataURL(file)
  })
}

async function onImportChange(event: Event) {
  const input = event.target as HTMLInputElement
  const selectedFiles = [...(input.files ?? [])]
  input.value = ''
  if (!selectedFiles.length || importBusy.value) return
  importBusy.value = true
  try {
    const existing = new Set(files.entries.map((entry) => entry.name.toLowerCase()))
    const conflicts = selectedFiles.filter((file) => existing.has(file.name.toLowerCase()))
    let overwrite = false
    if (conflicts.length) {
      overwrite = window.confirm(
        settings.t('files.importConflict', { names: conflicts.map((file) => file.name).join(', ') }),
      )
      if (!overwrite) {
        const keep = selectedFiles.filter((file) => !existing.has(file.name.toLowerCase()))
        if (!keep.length) return
        selectedFiles.splice(0, selectedFiles.length, ...keep)
      }
    }
    const payloads: ImportFilePayload[] = []
    for (const file of selectedFiles) {
      payloads.push({ name: file.name, data: await fileToBase64(file) })
    }
    const result = await importFiles('workspace', files.path, payloads, overwrite)
    await files.refresh()
    toast.success(settings.t('files.importSuccess', { count: result.imported }))
  } catch (error) {
    toast.error(settings.t('files.importFailed'), errorMessage(error))
  } finally {
    importBusy.value = false
  }
}

function closeExport() {
  if (!exportBusy.value) exportOpen.value = false
}

async function exportZip() {
  if (exportBusy.value) return
  let destination: string | null = null
  if (!isAndroid) {
    destination = await pickExportDirectory()
    if (!destination) return
  }
  exportOpen.value = true
  exportBusy.value = true
  exportResult.value = ''
  exportProgress.value = { done: 0, total: 0, file: '' }
  const unlisten = await listen<{ done: number; total: number; file: string }>(
    'zip-progress',
    (event) => {
      exportProgress.value = event.payload
    },
  )
  try {
    const result = await exportWorkspaceZip(destination ?? undefined)
    exportResult.value = `${result.name} · ${formatBytes(result.size)}`
    if (isAndroid) {
      await shareFile(result.path, 'application/zip', result.name)
    } else {
      toast.success(settings.t('files.exportSuccess'), result.path)
    }
  } catch (error) {
    exportOpen.value = false
    toast.error(settings.t('files.exportFailed'), errorMessage(error))
  } finally {
    unlisten()
    exportBusy.value = false
  }
}

function randomPassword(): string {
  const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'
  const values = new Uint32Array(4)
  crypto.getRandomValues(values)
  return [...values].map((value) => alphabet[value % alphabet.length]).join('')
}

async function openHttpConfirm() {
  httpError.value = ''
  httpPassword.value = randomPassword()
  httpBindAddress.value = '0.0.0.0'
  httpInterfaces.value = []
  httpConfirmOpen.value = true
  try {
    httpInterfaces.value = await listNetworkInterfaces()
  } catch {
    httpInterfaces.value = []
  }
}

async function startHttp() {
  if (httpBusy.value) return
  if (!/^[A-Za-z0-9]{4,12}$/.test(httpPassword.value)) {
    httpError.value = settings.t('files.httpPasswordInvalid')
    return
  }
  httpBusy.value = true
  httpError.value = ''
  try {
    httpInfo.value = await startHttpServer(
      httpPassword.value,
      httpBindAddress.value === '0.0.0.0' ? undefined : httpBindAddress.value,
    )
    httpConfirmOpen.value = false
    httpOpen.value = true
  } catch (error) {
    httpError.value = errorMessage(error)
  } finally {
    httpBusy.value = false
  }
}

async function closeHttp() {
  if (httpBusy.value) return
  httpBusy.value = true
  try {
    await stopHttpServer()
    httpOpen.value = false
    httpInfo.value = null
  } catch (error) {
    toast.error(settings.t('files.httpStopFailed'), errorMessage(error))
  } finally {
    httpBusy.value = false
  }
}

async function copyHttp() {
  if (!httpInfo.value) return
  await copyText(httpInfo.value.url)
  toast.success(settings.t('toast.copied'))
}

async function openHttp() {
  if (!httpInfo.value) return
  await openExternal(httpInfo.value.url)
}

function openFolderDialog() {
  folderName.value = ''
  folderError.value = ''
  folderOpen.value = true
  nextTick(() => {
    const input = document.querySelector<HTMLInputElement>('.folder-name-input')
    input?.focus()
  })
}

async function confirmFolder() {
  if (!folderName.value.trim()) return
  if (folderName.value.includes('/') || folderName.value.includes('\\')) {
    folderError.value = settings.t('files.errorFolder')
    return
  }
  folderBusy.value = true
  folderError.value = ''
  try {
    await files.makeFolder(folderName.value.trim())
    folderOpen.value = false
  } catch (error) {
    folderError.value = errorMessage(error)
  } finally {
    folderBusy.value = false
  }
}

function onWindowClick() {
  closeMenu()
  moreOpen.value = false
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    closeMenu()
    moreOpen.value = false
  }
}

watch(viewerMode, () => {
  /* mode changes are driven explicitly by switchViewerMode */
})

onMounted(async () => {
  if (!platform.ready) await platform.init()
  await files.load('', 'workspace')
  window.addEventListener('click', onWindowClick)
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('click', onWindowClick)
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="page files-page">
    <header class="page-head">
      <div>
        <span class="eyebrow">{{ settings.t('files.eyebrow') }}</span>
        <h1 class="page-title">{{ settings.t('files.title') }}</h1>
        <p class="page-subtitle">{{ settings.t('files.subtitle') }}</p>
      </div>
      <div class="head-actions">
        <input ref="importInput" class="hidden-file-input" type="file" multiple @change="onImportChange" />
        <div class="head-actions__buttons">
          <button v-if="isAndroid" class="btn btn--outline btn--sm" :disabled="importBusy" @click="openImport">
            <Loader2 v-if="importBusy" :size="14" class="spinning" />
            <FileUp v-else :size="14" />
            {{ settings.t('files.import') }}
          </button>
          <button class="btn btn--outline btn--sm" @click="openHttpConfirm">
            <Server :size="14" />
            {{ settings.t('files.httpServer') }}
          </button>
          <button class="btn btn--outline btn--sm" @click="exportZip">
            <Upload :size="14" />
            {{ settings.t('files.export') }}
          </button>
          <button class="btn btn--outline btn--sm" @click="files.refresh()">
            <RefreshCw :size="14" :class="{ spinning: files.loading }" />
            {{ settings.t('files.refresh') }}
          </button>
          <button class="btn btn--primary btn--sm" @click="openFolderDialog">
            <FolderPlus :size="14" />
            {{ settings.t('files.newFolder') }}
          </button>
        </div>
        <div class="head-actions__more-wrap">
          <button class="icon-btn icon-btn--bordered head-actions__more" title="More actions" @click.stop="moreOpen = !moreOpen">
            <MoreVertical :size="17" />
          </button>
          <div v-if="moreOpen" class="head-actions__menu" @click.stop>
            <button v-if="isAndroid" class="head-actions__menu-item" @click="moreOpen = false; openImport()">
              <FileUp :size="15" /> {{ settings.t('files.import') }}
            </button>
            <button class="head-actions__menu-item" @click="moreOpen = false; openHttpConfirm()">
              <Server :size="15" /> {{ settings.t('files.httpServer') }}
            </button>
            <button class="head-actions__menu-item" @click="moreOpen = false; exportZip()">
              <Upload :size="15" /> {{ settings.t('files.export') }}
            </button>
            <button class="head-actions__menu-item" @click="moreOpen = false; files.refresh()">
              <RefreshCw :size="15" /> {{ settings.t('files.refresh') }}
            </button>
            <button class="head-actions__menu-item" @click="moreOpen = false; openFolderDialog()">
              <FolderPlus :size="15" /> {{ settings.t('files.newFolder') }}
            </button>
          </div>
        </div>
      </div>
    </header>

    <section class="workspace-head panel">
      <div class="workspace-head__path">
        <span class="workspace-head__icon"><HardDrive :size="18" /></span>
        <div>
          <span class="workspace-head__label">{{ settings.t('home.workdir') }}</span>
          <div
            class="workspace-head__value mono"
            :title="platform.paths.pm3Dir"
          >
            {{ shortPath(platform.paths.pm3Dir || '—', 74) }}
          </div>
        </div>
      </div>
      <div class="workspace-head__stats">
        <span><strong>{{ files.fileCount }}</strong> {{ settings.t('files.filesShort') }}</span>
        <span class="dot" />
        <span><strong>{{ files.folderCount }}</strong> {{ settings.t('files.foldersShort') }}</span>
        <span class="dot" />
        <span class="mono">{{ formatBytes(files.totalSize) }}</span>
      </div>
    </section>

    <div class="browser">
      <div class="browser__toolbar">
        <div class="breadcrumbs">
          <button class="crumb crumb--icon" :disabled="!files.path" @click="files.load('')">
            <Home :size="14" />
          </button>
          <template v-for="(crumb, index) in files.breadcrumbs" :key="crumb.path">
            <ChevronRight v-if="index > 0" :size="12" class="crumb-sep" />
            <button
              class="crumb"
              :class="{ 'crumb--current': index === files.breadcrumbs.length - 1 }"
              @click="files.load(crumb.path)"
            >
              {{ crumb.name }}
            </button>
          </template>
        </div>

        <div class="browser__tools">
          <label class="input-with-icon search">
            <Search :size="15" />
            <input v-model="files.query" class="input" :placeholder="settings.t('files.searchPlaceholder')" />
            <button v-if="files.query" class="search__clear" @click="files.query = ''">
              <X :size="13" />
            </button>
          </label>
          <button
            class="icon-btn icon-btn--bordered up-button"
            :disabled="files.parent === null"
            :title="settings.t('files.parent')"
            @click="files.up()"
          >
            <ArrowUp :size="16" />
          </button>
        </div>
      </div>

      <div class="file-head">
        <button class="file-head__name" @click="files.setSort('name')">
          {{ settings.t('common.name') }}
          <span v-if="files.sortField === 'name'" class="sort-indicator">{{ files.sortDirection === 'asc' ? '↑' : '↓' }}</span>
        </button>
        <button class="file-head__size" @click="files.setSort('size')">
          {{ settings.t('common.size') }}
          <span v-if="files.sortField === 'size'" class="sort-indicator">{{ files.sortDirection === 'asc' ? '↑' : '↓' }}</span>
        </button>
        <button class="file-head__date" @click="files.setSort('modified')">
          {{ settings.t('common.modified') }}
          <span v-if="files.sortField === 'modified'" class="sort-indicator">{{ files.sortDirection === 'asc' ? '↑' : '↓' }}</span>
        </button>
        <span />
      </div>

      <div v-if="files.loading && !files.entries.length" class="file-skeletons">
        <div v-for="index in 7" :key="index" class="skeleton file-skeleton" />
      </div>

      <div v-else-if="files.error" class="empty-state">
        <span class="empty-icon"><AlertTriangle :size="22" /></span>
        <span class="empty-title">{{ settings.t('files.errorLoading') }}</span>
        <p class="empty-body">{{ files.error }}</p>
        <button class="btn btn--sm empty-action" @click="files.refresh()">
          <RefreshCw :size="13" />
          {{ settings.t('common.retry') }}
        </button>
      </div>

      <div v-else-if="files.isEmpty" class="empty-state">
        <span class="empty-icon"><FolderOpen :size="23" /></span>
        <span class="empty-title">
          {{ settings.t('files.emptyTitle') }}
        </span>
        <p class="empty-body">
          {{ settings.t('files.emptyBody') }}
        </p>
        <button
          class="btn btn--sm empty-action"
          @click="openFolderDialog"
        >
          <FolderPlus :size="13" />
          {{ settings.t('files.newFolder') }}
        </button>
      </div>

      <div v-else-if="files.isFilteredEmpty" class="empty-state">
        <span class="empty-icon"><Search :size="22" /></span>
        <span class="empty-title">{{ settings.t('files.noResults', { query: files.query }) }}</span>
        <p class="empty-body">{{ settings.t('files.noResultsBody') }}</p>
        <button class="btn btn--sm empty-action" @click="files.query = ''">
          {{ settings.t('common.close') }}
        </button>
      </div>

      <div v-else class="file-list">
        <div
          v-for="entry in files.filtered"
          :key="entry.path"
          class="file-row"
          :class="{ 'file-row--selected': selected?.path === entry.path }"
          @click="selected = entry"
          @dblclick="openEntry(entry)"
        >
          <button class="file-row__name" @click.stop="openEntry(entry)">
            <span class="file-icon" :class="`file-icon--${accentFor(entry)}`">
              <component :is="iconFor(entry)" :size="17" />
            </span>
            <span class="file-row__name-text">
              <strong>{{ entry.name }}</strong>
              <em class="file-row__meta-mobile">
                {{ entry.isDir ? settings.t('files.folder') : formatBytes(entry.size) }} ·
                {{ dateLabel(entry.modified) }}
              </em>
            </span>
          </button>
          <span class="file-row__size mono">{{ entry.isDir ? '—' : formatBytes(entry.size) }}</span>
          <span class="file-row__date">{{ dateLabel(entry.modified) }}</span>
          <div  class="file-row__actions">
            <button class="icon-btn row-action" :title="settings.t('files.moreActions')" @click="toggleMenu($event, entry)">
              <MoreHorizontal :size="17" />
            </button>
          </div>
        </div>
      </div>

      <footer class="browser__foot">
        <span class="mono">{{ settings.t('files.fileCount', { count: files.fileCount }) }}</span>
        <span>·</span>
        <span class="mono">{{ formatBytes(files.totalSize) }}</span>
        <span class="browser__foot-spacer" />
        <span v-if="files.loading" class="loading-note">
          <span class="spinner spinner--sm" />
          {{ settings.t('common.loading') }}
        </span>
      </footer>
    </div>

    <Teleport to="body">
      <div v-if="menu" class="menu-overlay" @click="closeMenu" @contextmenu.prevent="closeMenu">
        <div class="context-menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" @click.stop>
          <button class="context-menu__item" @click="menuAction('view')">
            <Eye :size="15" />
            {{ settings.t('files.view') }}
          </button>
          <button v-if="shareSupported" class="context-menu__item" @click="menuAction('share')">
            <Share2 :size="15" />
            {{ settings.t('files.share') }}
          </button>
          <button v-else class="context-menu__item" @click="menuAction('reveal')">
            <MapPin :size="15" />
            {{ settings.t('files.reveal') }}
          </button>
          <button class="context-menu__item" @click="menuAction('move')">
            <FolderInput :size="15" />
            {{ settings.t('files.move') }}
          </button>
          <div class="context-menu__sep" />
          <button class="context-menu__item context-menu__item--danger" @click="menuAction('delete')">
            <Trash2 :size="15" />
            {{ settings.t('files.delete') }}
          </button>
        </div>
      </div>
    </Teleport>

    <Modal
      :open="viewerOpen"
      :title="viewerEntry?.name"
      :subtitle="viewer ? settings.t('files.viewerSubtitle', { size: formatBytes(viewer.size), date: dateLabel(viewer.modified) }) : ''"
      full
      flush
      @close="viewerOpen = false"
    >
      <div class="viewer">
        <div class="viewer__bar">
          <div class="segmented">
            <button class="segmented__item" :class="{ 'segmented__item--active': viewerMode === 'text' }" @click="switchViewerMode('text')">
              {{ settings.t('files.text') }}
            </button>
            <button class="segmented__item" :class="{ 'segmented__item--active': viewerMode === 'hex' }" @click="switchViewerMode('hex')">
              {{ settings.t('files.hex') }}
            </button>
          </div>
          <div class="viewer__bar-right">
            <span v-if="viewer?.binary" class="badge">{{ settings.t('files.binary') }}</span>
            <button class="btn btn--ghost btn--sm" @click="wrapLines = !wrapLines">
              <WrapText :size="14" />
              {{ settings.t('files.wrap') }}
            </button>
            <button class="btn btn--ghost btn--sm" @click="copyViewerContent">
              <Copy :size="14" />
              {{ settings.t('files.copyContent') }}
            </button>
          </div>
        </div>

        <div v-if="viewer?.truncated" class="viewer__notice">
          <AlertTriangle :size="14" />
          {{ settings.t('files.truncated', { size: formatBytes(viewer.content.length) }) }}
        </div>

        <div v-if="viewerLoading" class="viewer__loading">
          <span class="spinner" />
          <span>{{ settings.t('files.loadingFile') }}</span>
        </div>
        <div v-else-if="viewerError" class="empty-state">
          <span class="empty-icon"><AlertTriangle :size="22" /></span>
          <span class="empty-title">{{ settings.t('files.errorReading') }}</span>
          <p class="empty-body">{{ viewerError }}</p>
        </div>
        <pre
          v-else-if="viewer"
          class="viewer__content"
          :class="{ 'viewer__content--wrap': wrapLines, 'viewer__content--empty': !viewer.content }"
        >{{ viewer.content || settings.t('files.emptyFile') }}</pre>
        <div class="viewer__status">
          <span class="mono">{{ viewer?.path }}</span>
          <span class="viewer__status-right">
            <span>{{ viewerMode.toUpperCase() }}</span>
            <span>·</span>
            <span>{{ settings.t('files.readOnlyNotice') }}</span>
          </span>
        </div>
      </div>
    </Modal>

    <Modal
      :open="deleteOpen"
      :title="settings.t('files.deleteTitle', { name: deleteEntry?.name ?? '' })"
      @close="deleteOpen = false"
    >
      <div class="danger-callout">
        <span class="danger-callout__icon"><AlertTriangle :size="20" /></span>
        <p>{{ deleteEntry?.isDir ? settings.t('files.deleteFolderBody') : settings.t('files.deleteBody') }}</p>
      </div>
      <template #foot>
        <button class="btn btn--outline" @click="deleteOpen = false">{{ settings.t('common.cancel') }}</button>
        <button class="btn btn--danger" :disabled="deleteBusy" @click="confirmDelete">
          <span v-if="deleteBusy" class="spinner spinner--sm" />
          <Trash2 v-else :size="15" />
          {{ settings.t('files.deleteConfirm') }}
        </button>
      </template>
    </Modal>

    <Modal
      :open="moveOpen"
      :title="settings.t('files.moveTitle', { name: moveEntry?.name ?? '' })"
      :subtitle="settings.t('files.moveBody')"
      wide
      @close="moveOpen = false"
    >
      <div class="move-form">
        <label class="field">
          <span class="field-label">{{ settings.t('files.newName') }}</span>
          <input v-model="moveName" class="input" spellcheck="false" />
        </label>
        <div class="move-picker">
          <div class="move-picker__head">
            <span class="field-label">{{ settings.t('files.moveDestination') }}</span>
            <div class="move-picker__crumbs">
              <button class="crumb" @click="loadMoveDir('')">{{ settings.t('files.root') }}</button>
              <template v-for="crumb in moveBreadcrumb.slice(1)" :key="crumb.path">
                <ChevronRight :size="11" class="crumb-sep" />
                <button class="crumb" @click="loadMoveDir(crumb.path)">{{ crumb.name }}</button>
              </template>
            </div>
          </div>
          <div class="move-picker__list">
            <button class="move-picker__up" :disabled="!moveTarget" @click="loadMoveDir(parentOf(moveTarget))">
              <ArrowLeft :size="14" />
              {{ settings.t('files.parent') }}
            </button>
            <div v-if="moveLoading" class="move-picker__empty">
              <span class="spinner spinner--sm" />
            </div>
            <button
              v-for="dir in moveDirs"
              :key="dir.path"
              class="move-picker__item"
              :disabled="Boolean(moveEntry?.isDir && (dir.path === moveEntry.path || dir.path.startsWith(`${moveEntry.path}/`)))"
              @click="enterMoveDir(dir)"
            >
              <Folder :size="16" />
              <span>{{ dir.name }}</span>
              <ChevronRight :size="13" />
            </button>
            <div v-if="!moveLoading && !moveDirs.length" class="move-picker__empty">
              {{ settings.t('files.emptyTitle') }}
            </div>
          </div>
          <div class="move-picker__target">
            <span>{{ settings.t('files.moveHere') }}</span>
            <span class="mono">{{ moveTarget || settings.t('files.root') }}</span>
          </div>
        </div>
        <p v-if="moveError" class="form-error">{{ moveError }}</p>
      </div>
      <template #foot>
        <span class="move-target-note mono">{{ moveTarget || '/ ' }} / {{ moveName }}</span>
        <div class="foot-group">
          <button class="btn btn--outline" @click="moveOpen = false">{{ settings.t('common.cancel') }}</button>
          <button class="btn btn--primary" :disabled="moveDisabled" @click="confirmMove">
            <span v-if="moveBusy" class="spinner spinner--sm" />
            <FolderInput v-else :size="15" />
            {{ settings.t('files.moveConfirm') }}
          </button>
        </div>
      </template>
    </Modal>

    <Modal :open="folderOpen" :title="settings.t('files.folderTitle')" @close="folderOpen = false">
      <label class="field">
        <span class="field-label">{{ settings.t('files.folderName') }}</span>
        <input
          v-model="folderName"
          class="input folder-name-input"
          :placeholder="settings.t('files.folderPlaceholder')"
          spellcheck="false"
          @keydown.enter="confirmFolder"
        />
      </label>
      <p v-if="folderError" class="form-error">{{ folderError }}</p>
      <template #foot>
        <button class="btn btn--outline" @click="folderOpen = false">{{ settings.t('common.cancel') }}</button>
        <button class="btn btn--primary" :disabled="!folderName.trim() || folderBusy" @click="confirmFolder">
          <span v-if="folderBusy" class="spinner spinner--sm" />
          <FolderPlus v-else :size="15" />
          {{ settings.t('files.createFolder') }}
        </button>
      </template>
    </Modal>

    <Modal
      :open="httpConfirmOpen"
      :title="settings.t('files.httpTitle')"
      @close="httpConfirmOpen = false"
    >
      <div class="danger-callout">
        <span class="danger-callout__icon"><ShieldAlert :size="20" /></span>
        <p>{{ settings.t('files.httpWarningBody') }}</p>
      </div>
      <div class="http-config">
        <label class="field">
          <span class="field-label">{{ settings.t('files.httpInterface') }}</span>
          <select v-model="httpBindAddress" class="input">
            <option value="0.0.0.0">{{ settings.t('files.httpAllInterfaces') }}</option>
            <option v-for="item in httpInterfaces" :key="item.address" :value="item.address">
              {{ item.address }}
            </option>
          </select>
        </label>
        <label class="field">
          <span class="field-label">{{ settings.t('files.httpPassword') }}</span>
          <input
            v-model="httpPassword"
            class="input mono"
            maxlength="12"
            minlength="4"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
      </div>
      <p v-if="httpError" class="form-error">{{ httpError }}</p>
      <template #foot>
        <button class="btn btn--outline" @click="httpConfirmOpen = false">{{ settings.t('common.cancel') }}</button>
        <button class="btn btn--primary" :disabled="httpBusy" @click="startHttp">
          <span v-if="httpBusy" class="spinner spinner--sm" />
          <Server v-else :size="15" />
          {{ settings.t('files.httpStart') }}
        </button>
      </template>
    </Modal>

    <Modal
      :open="httpOpen"
      :title="settings.t('files.httpRunning')"
      :close-on-backdrop="false"
      :close-on-escape="false"
      @close="closeHttp"
    >
      <div v-if="httpInfo" class="http-info">
        <div class="http-info__row http-info__row--address">
          <div>
            <span>{{ settings.t('files.httpAddress') }}</span>
            <strong class="mono">{{ httpInfo.url }}</strong>
          </div>
          <button class="icon-btn icon-btn--bordered" :title="settings.t('files.httpCopy')" @click="copyHttp">
            <Copy :size="14" />
          </button>
        </div>
        <div class="http-info__row">
          <span>{{ settings.t('files.httpToken') }}</span>
          <strong class="mono">{{ httpInfo.token }}</strong>
        </div>
        <p>{{ settings.t('files.httpWarningBody') }}</p>
      </div>
      <template #foot>
        <button class="btn btn--primary" :disabled="httpBusy" @click="openHttp">
          <Globe2 :size="14" />
          {{ settings.t('files.httpOpen') }}
        </button>
        <button class="btn btn--danger" :disabled="httpBusy" @click="closeHttp">
          <span v-if="httpBusy" class="spinner spinner--sm" />
          <X v-else :size="14" />
          {{ settings.t('files.httpStop') }}
        </button>
      </template>
    </Modal>

    <Modal
      :open="exportOpen"
      :title="settings.t('files.export')"
      @close="closeExport"
    >
      <div class="export-progress">
        <span v-if="exportBusy" class="spinner" />
        <Check v-else :size="22" />
        <div>
          <strong>{{ exportBusy ? settings.t('files.exporting') : settings.t('files.exportSuccess') }}</strong>
          <p v-if="exportBusy && exportProgress.total" class="export-progress__count">
            {{ exportProgress.done }} / {{ exportProgress.total }} · {{ exportPercent }}%
          </p>
          <p v-if="exportBusy && exportProgress.file" class="mono export-progress__file">{{ exportProgress.file }}</p>
          <p v-if="exportResult" class="mono">{{ exportResult }}</p>
        </div>
      </div>
      <div v-if="exportBusy" class="export-bar">
        <span :style="{ width: `${exportPercent}%` }" />
      </div>
      <template #foot>
        <button class="btn btn--primary" :disabled="exportBusy" @click="exportOpen = false">
          {{ settings.t('common.close') }}
        </button>
      </template>
    </Modal>
  </div>
</template>

<style scoped>
.files-page {
  padding-bottom: 72px;
}

.hidden-file-input {
  display: none;
}

.head-actions__buttons {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.head-actions__more-wrap {
  position: relative;
  display: none;
}

.head-actions__more {
  display: grid;
}

.head-actions__menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 30;
  min-width: 190px;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--panel);
  box-shadow: var(--shadow-lg);
}

.head-actions__menu-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 9px 10px;
  border: 0;
  border-radius: 7px;
  background: none;
  color: var(--text-soft);
  font-size: 12px;
  text-align: left;
}

.head-actions__menu-item:hover {
  background: var(--panel-hover);
  color: var(--text);
}

@media (max-width: 1180px) {
  .head-actions__buttons {
    display: none;
  }

  .head-actions__more-wrap {
    display: block;
  }
}

.export-progress {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 70px;
}

.export-progress strong {
  display: block;
  font-size: 13px;
  color: var(--text);
}

.export-progress p {
  margin: 5px 0 0;
  color: var(--muted);
  font-size: 11px;
  word-break: break-all;
}

.export-progress__count {
  font-variant-numeric: tabular-nums;
}

.export-progress__file {
  max-width: 420px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.export-bar {
  height: 5px;
  margin-top: 12px;
  overflow: hidden;
  border-radius: 4px;
  background: var(--panel-inset);
}

.export-bar > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 120ms linear;
}

.http-info {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.http-config {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 12px;
  margin-top: 14px;
}

.http-config select {
  width: 100%;
}

.http-info__row--address {
  flex-direction: row;
  align-items: center;
  gap: 10px;
}

.http-info__row--address > div {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}

.http-info__row--address strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.http-info__row {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--panel-inset);
}

.http-info__row > span {
  color: var(--faint);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.http-info__row > strong {
  color: var(--accent);
  font-size: 11px;
  word-break: break-all;
}

.http-info > p {
  margin: 0;
  color: var(--muted);
  font-size: 11.5px;
  line-height: 1.55;
}

.spinning {
  animation: spin 0.9s linear infinite;
}

.workspace-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 17px 19px;
  margin-bottom: 14px;
}

.scope-switch {
  display: inline-flex;
  gap: 3px;
  margin-bottom: 14px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--panel-inset);
}

.scope-switch__item {
  height: 30px;
  padding: 0 14px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
}

.scope-switch__item--active {
  color: var(--accent);
  background: var(--accent-dim);
}

.workspace-head__path {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
}

.workspace-head__icon {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: var(--accent-dim);
  color: var(--accent);
  flex: none;
}

.workspace-head__label {
  display: block;
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--faint);
}

.workspace-head__value {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--text-soft);
  word-break: break-all;
}

.workspace-head__stats {
  display: flex;
  align-items: center;
  gap: 7px;
  flex: none;
  font-size: 11px;
  color: var(--muted);
}

.workspace-head__stats strong {
  color: var(--text-soft);
  font-family: var(--font-mono);
  font-size: 11px;
}

.workspace-head__stats .dot {
  width: 2px;
  height: 2px;
  border-radius: 50%;
  background: var(--faint);
}

.browser {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--panel);
  box-shadow: var(--shadow-panel);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}

.browser__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 13px 15px;
  border-bottom: 1px solid var(--border);
}

.breadcrumbs {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.breadcrumbs::-webkit-scrollbar {
  display: none;
}

.crumb {
  height: 27px;
  padding: 0 7px;
  border-radius: 6px;
  color: var(--muted);
  font-size: 11px;
  font-weight: 550;
  white-space: nowrap;
}

.crumb:hover {
  color: var(--text);
  background: var(--panel-hover);
}

.crumb--current {
  color: var(--text);
}

.crumb--icon {
  display: grid;
  place-items: center;
  width: 27px;
  padding: 0;
}

.crumb-sep {
  color: var(--faint);
  flex: none;
}

.browser__tools {
  display: flex;
  align-items: center;
  gap: 7px;
  flex: none;
}

.search {
  width: 230px;
}

.search .input {
  height: 34px;
  padding-right: 27px;
  font-size: 11px;
}

.search__clear {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 5px;
  color: var(--faint);
}

.search__clear:hover {
  color: var(--text);
  background: var(--panel-hover);
}

.up-button {
  width: 34px;
  height: 34px;
}

.file-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 100px 160px 52px;
  gap: 10px;
  align-items: center;
  padding: 9px 16px;
  border-bottom: 1px solid var(--border);
  background: rgba(0, 0, 0, 0.1);
}

.file-head button {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0;
  color: var(--faint);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.11em;
  text-transform: uppercase;
  text-align: left;
}

.file-head button:hover {
  color: var(--text-soft);
}

.file-head__size {
  justify-content: flex-end;
}

.file-head__date {
  justify-content: flex-start;
}

.sort-indicator {
  color: var(--accent);
}

.file-list {
  display: flex;
  flex-direction: column;
}

.file-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 100px 160px 52px;
  gap: 10px;
  align-items: center;
  min-height: 45px;
  padding: 3px 16px;
  border-bottom: 1px solid rgba(139, 171, 214, 0.055);
  transition: background var(--transition);
}

.file-row:hover {
  background: rgba(255, 255, 255, 0.02);
}

.file-row--selected {
  background: var(--accent-faint);
}

.file-row__name {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 5px 0;
  color: var(--text-soft);
  text-align: left;
}

.file-row__name-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.file-row__name strong {
  font-size: 12px;
  font-weight: 550;
  color: var(--text-soft);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-row__meta-mobile {
  display: none;
  font-style: normal;
  font-size: 11.5px;
  color: var(--faint);
}

.file-icon {
  display: grid;
  place-items: center;
  width: 29px;
  height: 29px;
  border-radius: 7px;
  flex: none;
  background: rgba(255, 255, 255, 0.035);
  color: var(--muted);
}

.file-icon--folder {
  background: var(--warning-dim);
  color: var(--warning);
}
.file-icon--json {
  background: var(--secondary-dim);
  color: var(--secondary);
}
.file-icon--code {
  background: var(--accent-dim);
  color: var(--accent);
}
.file-icon--text {
  background: rgba(255, 255, 255, 0.045);
  color: var(--text-soft);
}
.file-icon--binary {
  background: rgba(177, 140, 255, 0.13);
  color: #b18cff;
}

.file-row__size {
  font-size: 11.5px;
  color: var(--faint);
  text-align: right;
}

.file-row__date {
  font-size: 11.5px;
  color: var(--faint);
}

.file-row__actions {
  display: flex;
  justify-content: flex-end;
}

.row-action {
  width: 30px;
  height: 30px;
  border-radius: 7px;
}

.file-skeletons {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 8px 0;
}

.file-skeleton {
  height: 43px;
  margin: 0 16px;
  border-radius: 7px;
}

.browser__foot {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 16px;
  border-top: 1px solid var(--border);
  color: var(--faint);
  font-size: 11.5px;
}

.browser__foot-spacer {
  flex: 1;
}

.loading-note {
  display: flex;
  align-items: center;
  gap: 6px;
}

.menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
}

.context-menu {
  position: fixed;
  z-index: 151;
  width: 196px;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: 11px;
  background: var(--panel-solid);
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.55);
  animation: menu-in 140ms var(--ease-out) both;
}

@keyframes menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px) scale(0.98);
  }
}

.context-menu__item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 32px;
  padding: 0 9px;
  border-radius: 6px;
  color: var(--text-soft);
  font-size: 11.5px;
  text-align: left;
}

.context-menu__item svg {
  color: var(--faint);
}

.context-menu__item:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.context-menu__item--danger {
  color: var(--danger);
}

.context-menu__item--danger svg {
  color: var(--danger);
}

.context-menu__item--danger:hover {
  background: var(--danger-dim);
}

.context-menu__sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--border);
}

.viewer {
  display: flex;
  flex-direction: column;
  height: min(76vh, 760px);
}

.viewer__bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.viewer__bar-right {
  display: flex;
  align-items: center;
  gap: 5px;
}

.viewer__notice {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border);
  background: var(--warning-dim);
  color: var(--warning);
  font-size: 11px;
}

.viewer__content {
  flex: 1;
  margin: 0;
  padding: 18px;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-soft);
  white-space: pre;
  tab-size: 4;
  background:
    linear-gradient(90deg, rgba(255, 255, 255, 0.015) 1px, transparent 1px) 0 0 / 64px 100%,
    var(--bg-deep);
}

.viewer__content--wrap {
  white-space: pre-wrap;
  word-break: break-word;
}

.viewer__content--empty {
  color: var(--faint);
  display: grid;
  place-items: center;
  font-family: var(--font-sans);
  font-size: 13px;
}

.viewer__loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--muted);
  font-size: 11px;
}

.viewer__status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 16px;
  border-top: 1px solid var(--border);
  color: var(--faint);
  font-size: 11px;
}

.viewer__status .mono {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.viewer__status-right {
  display: flex;
  gap: 6px;
  flex: none;
}

.danger-callout {
  display: flex;
  gap: 12px;
  padding: 14px;
  border: 1px solid rgba(255, 107, 129, 0.22);
  border-radius: 11px;
  background: var(--danger-dim);
}

.danger-callout__icon {
  color: var(--danger);
  flex: none;
}

.danger-callout p {
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-soft);
}

.move-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.move-picker {
  border: 1px solid var(--border);
  border-radius: 11px;
  overflow: hidden;
}

.move-picker__head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--border);
  background: rgba(0, 0, 0, 0.08);
}

.move-picker__head .field-label {
  flex: none;
}

.move-picker__crumbs {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
}

.move-picker__list {
  max-height: 220px;
  min-height: 100px;
  overflow: auto;
  padding: 5px;
}

.move-picker__up,
.move-picker__item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 34px;
  padding: 0 9px;
  border-radius: 7px;
  color: var(--muted);
  font-size: 11px;
  text-align: left;
}

.move-picker__up:hover:not(:disabled),
.move-picker__item:hover:not(:disabled) {
  color: var(--text-soft);
  background: var(--panel-hover);
}

.move-picker__up {
  border-bottom: 1px dashed var(--border);
  border-radius: 0;
  color: var(--faint);
  font-size: 11px;
}

.move-picker__up:disabled,
.move-picker__item:disabled {
  opacity: 0.36;
  cursor: not-allowed;
}

.move-picker__item svg:first-child {
  color: var(--warning);
}

.move-picker__item svg:last-child {
  margin-left: auto;
  color: var(--faint);
}

.move-picker__empty {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 84px;
  font-size: 11.5px;
  color: var(--faint);
}

.move-picker__target {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  border-top: 1px solid var(--border);
  background: var(--accent-faint);
  font-size: 11.5px;
  color: var(--muted);
}

.move-picker__target .mono {
  font-size: 11.5px;
  color: var(--accent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.form-error {
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid rgba(255, 107, 129, 0.24);
  background: var(--danger-dim);
  color: var(--danger);
  font-size: 11.5px;
  line-height: 1.5;
}

.move-target-note {
  margin-right: auto;
  font-size: 11px;
  color: var(--faint);
  max-width: 48%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.foot-group {
  display: flex;
  gap: 8px;
}

@media (max-width: 900px) {
  .file-head,
  .file-row {
    grid-template-columns: minmax(0, 1fr) 80px 130px 44px;
  }

  .workspace-head {
    flex-direction: column;
    align-items: flex-start;
    gap: 9px;
  }
}

@media (max-width: 640px) {
  .files-page {
    padding-bottom: 92px;
  }

  .head-actions .btn {
    flex: 1;
  }

  .http-config {
    grid-template-columns: 1fr;
  }

  .workspace-head {
    padding: 14px;
  }

  .workspace-head__stats {
    flex-wrap: wrap;
  }

  .browser__toolbar {
    flex-direction: column;
    align-items: stretch;
    gap: 9px;
    padding: 10px 12px;
  }

  .browser__tools {
    width: 100%;
  }

  .search {
    flex: 1;
    width: auto;
  }

  .file-head {
    display: none;
  }

  .file-row {
    grid-template-columns: minmax(0, 1fr) 34px;
    gap: 6px;
    padding: 5px 10px;
    min-height: 50px;
  }

  .file-row__size,
  .file-row__date {
    display: none;
  }

  .file-row__meta-mobile {
    display: block;
    margin-top: 1px;
  }

  .file-icon {
    width: 31px;
    height: 31px;
  }

  .browser__foot {
    padding-inline: 12px;
  }

  .viewer {
    height: 78vh;
    height: 78dvh;
  }

  .viewer__bar {
    flex-wrap: wrap;
    padding: 10px 12px;
  }

  .viewer__bar-right {
    width: 100%;
    justify-content: space-between;
  }

  .viewer__content {
    padding: 12px;
    font-size: 11.5px;
  }

  .viewer__status {
    padding: 7px 12px;
  }

  .move-picker__head {
    align-items: flex-start;
    flex-direction: column;
    gap: 5px;
  }

  .modal__foot .move-target-note {
    display: none;
  }
}
</style>
