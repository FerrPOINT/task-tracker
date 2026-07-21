const UNITS: { regex: RegExp; seconds: number }[] = [
  { regex: /(\d+(?:\.\d+)?)\s*d/i, seconds: 28800 },
  { regex: /(\d+(?:\.\d+)?)\s*h/i, seconds: 3600 },
  { regex: /(\d+)\s*m/i, seconds: 60 },
  { regex: /(\d+)\s*s/i, seconds: 1 },
]

export function parseDuration(input: string): number | null {
  const normalized = input.trim().replace(/,/g, '.').replace(/\s+/g, ' ')
  if (!normalized) return null

  let total = 0
  let matched = false
  let remaining = normalized

  for (const { regex, seconds } of UNITS) {
    const match = remaining.match(regex)
    if (match) {
      const value = parseFloat(match[1] ?? '0')
      if (Number.isNaN(value) || value < 0) return null
      total += value * seconds
      remaining = remaining.replace(match[0], '').trim()
      matched = true
    }
  }

  if (!matched || remaining.length > 0) return null
  return Math.round(total)
}

export function formatDuration(totalSeconds: number): string {
  if (totalSeconds <= 0) return '0m'
  if (totalSeconds < 60) return `${totalSeconds}s`

  const days = totalSeconds > 28800 ? Math.floor(totalSeconds / 28800) : 0
  let remainder = totalSeconds - days * 28800
  const hours = Math.floor(remainder / 3600)
  remainder %= 3600
  const minutes = Math.round(remainder / 60)

  const parts: string[] = []
  if (days > 0) parts.push(`${days}d`)
  if (hours > 0 || (days > 0 && minutes > 0)) parts.push(`${hours}h`)
  if (minutes > 0 || parts.length === 0) parts.push(`${minutes}m`)

  return parts.join(' ')
}

export function formatDurationShort(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.round((totalSeconds % 3600) / 60)
  if (hours > 0 && minutes > 0) return `${hours}h ${minutes}m`
  if (hours > 0) return `${hours}h`
  return `${minutes}m`
}
