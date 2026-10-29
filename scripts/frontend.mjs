import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const viteBin = join(root, 'node_modules', 'vite', 'bin', 'vite.js')
const mode = process.argv[2] ?? 'dev'

if (!existsSync(viteBin)) {
  console.error('Dependencies are missing. Run `pnpm install` first.')
  process.exit(1)
}

const args = mode === 'build' ? ['build'] : ['--host', '127.0.0.1', '--port', '1420']
const child = spawn(process.execPath, [viteBin, ...args], {
  cwd: root,
  stdio: 'inherit',
  env: {
    ...process.env,
    TAURI_DEV_HOST: process.env.TAURI_DEV_HOST ?? '',
  },
})

child.on('error', (error) => {
  console.error(error)
  process.exit(1)
})

child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal)
  else process.exit(code ?? 0)
})
