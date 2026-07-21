import { expect, describe, it } from 'vitest'
import { parseDuration, formatDuration, formatDurationShort } from './time'

describe('time helpers', () => {
  describe('parseDuration', () => {
    it('parses hours and minutes', () => {
      expect(parseDuration('1h 30m')).toBe(5400)
    })

    it('parses days', () => {
      expect(parseDuration('2d')).toBe(57600)
    })

    it('parses mixed units', () => {
      expect(parseDuration('1d 2h 30m')).toBe(37800)
    })

    it('rejects invalid strings', () => {
      expect(parseDuration('abc')).toBeNull()
      expect(parseDuration('1x')).toBeNull()
      expect(parseDuration('')).toBeNull()
    })

    it('parses seconds', () => {
      expect(parseDuration('90s')).toBe(90)
    })

    it('rejects negative numbers', () => {
      expect(parseDuration('-1h')).toBeNull()
    })

    it('allows fractional hours', () => {
      expect(parseDuration('1.5h')).toBe(5400)
    })
  })

  describe('formatDuration', () => {
    it('round trips parsed values', () => {
      expect(formatDuration(5400)).toBe('1h 30m')
      expect(formatDuration(57600)).toBe('2d')
      expect(formatDuration(37800)).toBe('1d 2h 30m')
    })

    it('formats zero', () => {
      expect(formatDuration(0)).toBe('0m')
    })

    it('formats seconds', () => {
      expect(formatDuration(45)).toBe('45s')
    })
  })

  describe('formatDurationShort', () => {
    it('shows hours and minutes', () => {
      expect(formatDurationShort(5400)).toBe('1h 30m')
      expect(formatDurationShort(3600)).toBe('1h')
      expect(formatDurationShort(90)).toBe('2m')
    })
  })
})
