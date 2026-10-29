import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  createFolder,
  deletePath,
  listDir,
  movePath,
  type FileEntry,
} from '@/services/backend'
import { useSettingsStore } from './settings'
import { useToastStore } from './toast'
import { basename, errorMessage } from '@/utils/format'

export type SortField = 'name' | 'size' | 'modified'

export interface Breadcrumb {
  name: string
  path: string
}

export const useFilesStore = defineStore('files', () => {
  const settings = useSettingsStore()
  const toast = useToastStore()

  const path = ref('')
  const parent = ref<string | null>(null)
  const entries = ref<FileEntry[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref('')
  const query = ref('')
  const sortField = ref<SortField>('name')
  const sortDirection = ref<'asc' | 'desc'>('asc')

  const filtered = computed(() => {
    const needle = query.value.trim().toLowerCase()
    const items = needle
      ? entries.value.filter((entry) => entry.name.toLowerCase().includes(needle))
      : [...entries.value]
    items.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
      let result = 0
      if (sortField.value === 'size') result = a.size - b.size
      else if (sortField.value === 'modified') result = a.modified - b.modified
      else result = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
      return sortDirection.value === 'asc' ? result : -result
    })
    return items
  })

  const fileCount = computed(() => entries.value.filter((entry) => !entry.isDir).length)
  const folderCount = computed(() => entries.value.filter((entry) => entry.isDir).length)
  const totalSize = computed(() =>
    entries.value.filter((entry) => !entry.isDir).reduce((sum, entry) => sum + entry.size, 0),
  )
  const breadcrumbs = computed<Breadcrumb[]>(() => {
    const crumbs: Breadcrumb[] = [{ name: settings.t('files.root'), path: '' }]
    if (!path.value) return crumbs
    const parts = path.value.split('/')
    let accumulated = ''
    for (const part of parts) {
      accumulated = accumulated ? `${accumulated}/${part}` : part
      crumbs.push({ name: part, path: accumulated })
    }
    return crumbs
  })
  const currentName = computed(() => (path.value ? basename(path.value) : settings.t('files.root')))
  const isEmpty = computed(() => loaded.value && !loading.value && entries.value.length === 0)
  const isFilteredEmpty = computed(() => loaded.value && !loading.value && entries.value.length > 0 && filtered.value.length === 0)

  async function load(target = '') {
    loading.value = true
    error.value = ''
    try {
      const result = await listDir(target)
      path.value = result.path
      parent.value = result.parent
      entries.value = result.entries
      loaded.value = true
    } catch (caught) {
      error.value = errorMessage(caught)
    } finally {
      loading.value = false
    }
  }

  async function refresh() {
    await load(path.value)
  }

  async function up() {
    if (parent.value === null) return
    await load(parent.value)
  }

  async function open(entry: FileEntry) {
    if (entry.isDir) await load(entry.path)
  }

  function setSort(field: SortField) {
    if (sortField.value === field) {
      sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc'
    } else {
      sortField.value = field
      sortDirection.value = 'asc'
    }
  }

  async function remove(entry: FileEntry) {
    try {
      await deletePath(entry.path)
      toast.success(settings.t('files.deleted', { name: entry.name }))
      await refresh()
    } catch (caught) {
      toast.error(settings.t('files.errorDelete'), errorMessage(caught))
      throw caught
    }
  }

  async function move(entry: FileEntry, target: string, name: string) {
    try {
      const destination = await movePath(entry.path, target, name)
      toast.success(settings.t('files.moveSuccess', { path: destination }))
      await refresh()
      return destination
    } catch (caught) {
      toast.error(settings.t('files.errorMove'), errorMessage(caught))
      throw caught
    }
  }

  async function makeFolder(name: string) {
    try {
      await createFolder(path.value, name)
      toast.success(settings.t('files.folderCreated', { name }))
      await refresh()
    } catch (caught) {
      toast.error(settings.t('files.errorFolder'), errorMessage(caught))
      throw caught
    }
  }

  function reset() {
    path.value = ''
    parent.value = null
    entries.value = []
    loaded.value = false
    error.value = ''
    query.value = ''
  }

  return {
    path,
    parent,
    entries,
    loading,
    loaded,
    error,
    query,
    sortField,
    sortDirection,
    filtered,
    fileCount,
    folderCount,
    totalSize,
    breadcrumbs,
    currentName,
    isEmpty,
    isFilteredEmpty,
    load,
    refresh,
    up,
    open,
    setSort,
    remove,
    move,
    makeFolder,
    reset,
  }
})
