#!/usr/bin/env node
// Version policy:
//   tag build      -> the tag without a leading "v"
//   CI main build  -> the newest reachable v* tag (development channel)
//   local build    -> a fixed, clearly non-release version
import { execFileSync } from 'node:child_process'

const semver = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/
const override = process.env.PROXMARK_STUDIO_VERSION?.trim()
if (override) {
  if (!semver.test(override)) throw new Error(`Invalid PROXMARK_STUDIO_VERSION: ${override}`)
  console.log(override)
  process.exit(0)
}

const ci = process.argv.includes('--ci')
if (!ci) {
  console.log('0.0.0-local')
  process.exit(0)
}

const ref = process.env.GITHUB_REF || ''
const refName = process.env.GITHUB_REF_NAME || ''
if (ref.startsWith('refs/tags/') || process.env.GITHUB_REF_TYPE === 'tag') {
  const version = (refName || ref.replace('refs/tags/', '')).replace(/^v/, '')
  if (!semver.test(version)) throw new Error(`Invalid release tag version: ${version}`)
  console.log(version)
  process.exit(0)
}

let latest = ''
try {
  latest = execFileSync('git', ['describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  }).trim()
} catch {
  // A repository without tags (for example a fresh fork) has no release base.
}
const version = (latest || 'v0.0.0').replace(/^v/, '')
if (!semver.test(version)) throw new Error(`Invalid version tag: ${latest}`)
console.log(version)
