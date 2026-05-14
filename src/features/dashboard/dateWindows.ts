import { todayInputValue } from '../../app/format'
import type { OverviewBucket } from '../../app/types'

export const CALENDAR_BUCKETS: OverviewBucket[] = [
  'day',
  'week',
  'month',
  'year',
  'custom',
  'total',
]
export const LIVE_QUOTA_BUCKETS: OverviewBucket[] = ['five_hour', 'seven_day']

export type CalendarAnchorBucket = 'day' | 'week' | 'month' | 'year'

export function formatCalendarWindowLabel(bucket: OverviewBucket, anchor: string, language: string) {
  if (bucket === 'day') return formatDateOnly(anchor, language)
  if (bucket === 'week') {
    const start = weekStartDate(anchor)
    const end = shiftDate(start, 6)
    return `${formatDateOnly(toIsoDate(start), language)} – ${formatDateOnly(toIsoDate(end), language)}`
  }
  if (bucket === 'month') {
    const [year, month] = anchor.split('-')
    return year && month ? `${year} / ${month}` : anchor.slice(0, 7)
  }
  if (bucket === 'year') return anchor.slice(0, 4)
  return null
}

export function createCalendarAnchors(value: string): Record<CalendarAnchorBucket, string> {
  const today = normalizeAnchorForBucket('day', value)
  return {
    day: today,
    week: today,
    month: normalizeAnchorForBucket('month', today),
    year: normalizeAnchorForBucket('year', today),
  }
}

export function calendarBucketForAnchor(bucket: OverviewBucket): CalendarAnchorBucket | null {
  if (bucket === 'day' || bucket === 'week' || bucket === 'month' || bucket === 'year') return bucket
  return null
}

export function anchorForBucket(bucket: OverviewBucket, anchors: Record<CalendarAnchorBucket, string>) {
  const anchorBucket = calendarBucketForAnchor(bucket)
  return anchorBucket ? anchors[anchorBucket] : todayInputValue()
}

export function normalizeAnchorForBucket(
  bucket: CalendarAnchorBucket,
  value: string,
  fallback = todayInputValue(),
) {
  const fallbackDate = parseIsoDate(fallback) ?? new Date()
  const parsed = parseIsoDate(value)

  if (bucket === 'month') {
    const monthMatch = value.match(/^(\d{4})-(\d{1,2})(?:-\d{1,2})?$/)
    if (monthMatch) {
      const year = Number(monthMatch[1])
      const month = Number(monthMatch[2])
      if (year >= 1 && year <= 9998 && month >= 1 && month <= 12) {
        return `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-01`
      }
    }
    return `${fallbackDate.getFullYear()}-${String(fallbackDate.getMonth() + 1).padStart(2, '0')}-01`
  }

  if (bucket === 'year') {
    const yearMatch = value.match(/^(\d{4})/)
    const candidateYear = yearMatch ? Number(yearMatch[1]) : fallbackDate.getFullYear()
    const year = Math.min(9998, Math.max(1970, candidateYear))
    return `${String(year).padStart(4, '0')}-01-01`
  }

  return parsed ? toIsoDate(parsed) : toIsoDate(fallbackDate)
}

export function shiftAnchorForBucket(bucket: CalendarAnchorBucket, value: string, amount: number) {
  const date = parseIsoDate(normalizeAnchorForBucket(bucket, value)) ?? new Date()
  if (bucket === 'day') return toIsoDate(shiftDate(date, amount))
  if (bucket === 'week') return toIsoDate(shiftDate(date, amount * 7))
  if (bucket === 'month') return toIsoDate(addMonths(date, amount))
  return normalizeAnchorForBucket('year', `${date.getFullYear() + amount}-01-01`, value)
}

export function monthInputValue(value: string) {
  return normalizeAnchorForBucket('month', value).slice(0, 7)
}

export function bucketUsesAnchor(bucket: OverviewBucket) {
  return calendarBucketForAnchor(bucket) !== null
}

export function inclusiveWindowEnd(windowEnd: string | null) {
  if (!windowEnd) return null
  const timestamp = new Date(windowEnd).getTime()
  if (!Number.isFinite(timestamp)) return null
  return new Date(timestamp - 1).toISOString()
}

export function formatCustomRangeLabel(windowStart: string | null, windowEnd: string | null, language: string) {
  const start = formatDateOnly(windowStart ?? '', language)
  const end = formatDateOnly(inclusiveWindowEnd(windowEnd) ?? '', language)
  if (start === end) return start
  return `${start} → ${end}`
}

function formatDateOnly(value: string, language: string) {
  const date = parseIsoDate(value)
  if (!date) return value
  return new Intl.DateTimeFormat(language === 'zh-CN' ? 'zh-CN' : 'en-US', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(date)
}

function weekStartDate(anchor: string) {
  const date = parseIsoDate(anchor) ?? new Date()
  const dayOffset = (date.getDay() + 6) % 7
  return shiftDate(date, -dayOffset)
}

function shiftDate(date: Date, days: number) {
  const next = new Date(date)
  next.setDate(next.getDate() + days)
  return next
}

function addMonths(date: Date, months: number) {
  const next = new Date(date)
  next.setDate(1)
  next.setMonth(next.getMonth() + months)
  return next
}

function parseIsoDate(value: string) {
  const dateOnlyMatch = value.match(/^(\d{4})-(\d{1,2})-(\d{1,2})$/)
  const date = dateOnlyMatch
    ? new Date(Number(dateOnlyMatch[1]), Number(dateOnlyMatch[2]) - 1, Number(dateOnlyMatch[3]))
    : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}

function toIsoDate(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}
