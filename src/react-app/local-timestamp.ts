const secondsPerMinute = 60
const minutesPerHour = 60
const hoursPerDay = 24
const secondsPerHour = secondsPerMinute * minutesPerHour
const secondsPerDay = secondsPerHour * hoursPerDay

export type LocalTimestamp = {
  readonly year: number
  readonly month: number
  readonly day: number
  readonly hour: number
  readonly minute: number
  readonly second: number
  readonly millisecond: number
  readonly dateKey: string
  readonly timeLabel: string
  readonly secondsOfDay: number
  readonly ordinalSeconds: number
}

type CalendarParts = {
  readonly year: number
  readonly month: number
  readonly day: number
  readonly hour: number
  readonly minute: number
  readonly second: number
  readonly millisecond: number
}

function pad(value: number, length: number): string {
  return String(value).padStart(length, '0')
}

function isLeapYear(year: number): boolean {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
}

function daysInMonth(year: number, month: number): number {
  switch (month) {
    case 1:
    case 3:
    case 5:
    case 7:
    case 8:
    case 10:
    case 12:
      return 31
    case 4:
    case 6:
    case 9:
    case 11:
      return 30
    case 2:
      return isLeapYear(year) ? 29 : 28
    default:
      return 0
  }
}

function daysBeforeYear(year: number): number {
  const completedYears = year - 1
  return completedYears * 365
    + Math.floor(completedYears / 4)
    - Math.floor(completedYears / 100)
    + Math.floor(completedYears / 400)
}

function daysBeforeMonth(year: number, month: number): number {
  let days = 0
  for (let currentMonth = 1; currentMonth < month; currentMonth += 1) {
    days += daysInMonth(year, currentMonth)
  }
  return days
}

function createLocalTimestamp(parts: CalendarParts): LocalTimestamp | null {
  const { year, month, day, hour, minute, second, millisecond } = parts
  if (
    year < 1
    || month < 1
    || month > 12
    || day < 1
    || day > daysInMonth(year, month)
    || hour < 0
    || hour >= hoursPerDay
    || minute < 0
    || minute >= minutesPerHour
    || second < 0
    || second >= secondsPerMinute
    || millisecond < 0
    || millisecond >= 1_000
  ) return null

  const secondsOfDay = hour * secondsPerHour + minute * secondsPerMinute + second
  const ordinalDays = daysBeforeYear(year) + daysBeforeMonth(year, month) + day - 1
  return {
    year,
    month,
    day,
    hour,
    minute,
    second,
    millisecond,
    dateKey: `${pad(year, 4)}-${pad(month, 2)}-${pad(day, 2)}`,
    timeLabel: `${pad(hour, 2)}:${pad(minute, 2)}:${pad(second, 2)}`,
    secondsOfDay,
    ordinalSeconds: ordinalDays * secondsPerDay + secondsOfDay,
  }
}

function parseParts(
  value: string,
  expression: RegExp,
  hasMillisecond: boolean,
): CalendarParts | null {
  const match = expression.exec(value)
  if (match === null) return null
  const year = match[1]
  const month = match[2]
  const day = match[3]
  const hour = match[4]
  const minute = match[5]
  const second = match[6]
  const millisecond = match[7]
  if (
    year === undefined
    || month === undefined
    || day === undefined
    || hour === undefined
    || minute === undefined
    || second === undefined
  ) return null
  return {
    year: Number(year),
    month: Number(month),
    day: Number(day),
    hour: Number(hour),
    minute: Number(minute),
    second: Number(second),
    millisecond: hasMillisecond && millisecond !== undefined ? Number(millisecond.padEnd(3, '0')) : 0,
  }
}

function timestampAtOrdinalSeconds(ordinalSeconds: number): LocalTimestamp | null {
  if (!Number.isSafeInteger(ordinalSeconds) || ordinalSeconds < 0) return null
  const ordinalDays = Math.floor(ordinalSeconds / secondsPerDay)
  const secondsOfDay = ordinalSeconds - ordinalDays * secondsPerDay
  let year = Math.floor(ordinalDays / 365.2425) + 1
  while (daysBeforeYear(year) > ordinalDays) year -= 1
  while (daysBeforeYear(year + 1) <= ordinalDays) year += 1

  let remainingDays = ordinalDays - daysBeforeYear(year)
  let month = 1
  while (remainingDays >= daysInMonth(year, month)) {
    remainingDays -= daysInMonth(year, month)
    month += 1
  }

  return createLocalTimestamp({
    year,
    month,
    day: remainingDays + 1,
    hour: Math.floor(secondsOfDay / secondsPerHour),
    minute: Math.floor((secondsOfDay % secondsPerHour) / secondsPerMinute),
    second: secondsOfDay % secondsPerMinute,
    millisecond: 0,
  })
}

export function parseFilenameLocalTimestamp(value: string): LocalTimestamp | null {
  const parts = parseParts(
    value,
    /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/u,
    false,
  )
  return parts === null ? null : createLocalTimestamp(parts)
}

export function parseMetadataLocalTimestamp(value: string): LocalTimestamp | null {
  const parts = parseParts(
    value,
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?Z?$/u,
    true,
  )
  return parts === null ? null : createLocalTimestamp(parts)
}

export function formatFilenameTimestampAtOffset(
  value: string,
  offsetSeconds: number,
): string | null {
  if (!Number.isInteger(offsetSeconds)) return null
  const timestamp = parseFilenameLocalTimestamp(value)
  if (timestamp === null) return null
  const shifted = timestampAtOrdinalSeconds(timestamp.ordinalSeconds + offsetSeconds)
  return shifted === null ? null : `${shifted.dateKey} ${shifted.timeLabel}`
}
