export interface DeviceInfo {
  firmware: string
  bootrom: string
  target: string
  hardware: string
  mcu: string
  protocol: string
  fpga: string
  bluetooth: string
  client: string
  raw: string
}

const EMPTY: DeviceInfo = {
  firmware: '',
  bootrom: '',
  target: '',
  hardware: '',
  mcu: '',
  protocol: '',
  fpga: '',
  bluetooth: '',
  client: '',
  raw: '',
}

function classify(label: string): keyof Omit<DeviceInfo, 'raw'> | null {
  const normalized = label.toLowerCase()
  if (normalized === 'device' || normalized.includes('hardware')) return 'hardware'
  if (normalized.includes('firmware') || normalized === 'os') return 'firmware'
  if (normalized.includes('bootrom') || normalized.includes('boot')) return 'bootrom'
  if (normalized.includes('target')) return 'target'
  if (normalized.includes('mcu') || normalized.includes('microcontroller')) return 'mcu'
  if (normalized.includes('protocol')) return 'protocol'
  if (normalized.includes('fpga')) return 'fpga'
  if (normalized.includes('bluetooth') || normalized.includes('ble')) return 'bluetooth'
  if (normalized.includes('client')) return 'client'
  return null
}

function cleanLine(line: string): string {
  return line
    .replace(/^[\s\u2502\u251c\u2514\u2500]+/u, '')
    .replace(/^\[[=+!*]\]\s*/, '')
    .trim()
}

const MODEL_CODE = /^(?:PM3\s+)?(?:RDV4|GENERIC|ULTIMATE|ICOPYX|PM5(?:_BWM)?|PROXSPACE)[A-Z0-9_ -]*$/i

/**
 * The client prints human-oriented identification output. Rather than pinning
 * one firmware format, scan for labelled values and keep the first useful hit.
 */
export function parseDeviceInfo(versionOutput: string, statusOutput: string): DeviceInfo {
  const info: DeviceInfo = { ...EMPTY, raw: [versionOutput, statusOutput].filter(Boolean).join('\n\n') }
  const lines = info.raw.split('\n')
  let section = ''

  for (const rawLine of lines) {
    const line = cleanLine(rawLine)
    if (!line) continue

    const heading = line.match(/^\[([^\]]+)\]$/)
    if (heading) {
      section = heading[1].trim().toLowerCase()
      continue
    }

    const labelled = line.match(/^([A-Za-z][A-Za-z0-9 /_().-]{1,40}?)\s*(?:\.{2,}|:)\s*(.+)$/)
    if (labelled) {
      const key = classify(labelled[1].trim())
      const value = labelled[2].trim()
      // `[ Model ] Firmware: RDV4` describes the hardware family, not the
      // client/ARM firmware version. The version comes from `[ ARM ] OS`.
      if (key === 'firmware' && section === 'model' && MODEL_CODE.test(value)) {
        if (!info.hardware) info.hardware = value
        continue
      }
      if (key && value && !info[key]) info[key] = value
      continue
    }

    if (!info.target) {
      const target = line.match(/\b(PM3\s?(?:RDV4|GENERIC|ULTIMATE|ICOPYX|PM5(?:_BWM)?|PROXSPACE)[A-Z0-9_ -]*)/i)
      if (target) info.target = target[1].trim()
    }
    if (!info.hardware) {
      const hardware = line.match(/^(?:Proxmark3|PM3)\s+(.+)$/i)
      if (hardware && !/command/i.test(line)) info.hardware = `Proxmark3 ${hardware[1]}`.trim()
    }
  }

  const pm3Model = info.raw.match(/\bPM3\s+(RDV4|GENERIC|ULTIMATE|ICOPYX|PM5(?:_BWM)?)\b/i)
  if (!info.target && pm3Model) info.target = `PM3 ${pm3Model[1].toUpperCase()}`
  if (!info.hardware && pm3Model) info.hardware = `Proxmark3 ${pm3Model[1].toUpperCase()}`

  return info
}

export function hasDeviceDetails(info: DeviceInfo | null): boolean {
  if (!info) return false
  return Boolean(
    info.firmware || info.bootrom || info.target || info.hardware || info.protocol || info.mcu,
  )
}
