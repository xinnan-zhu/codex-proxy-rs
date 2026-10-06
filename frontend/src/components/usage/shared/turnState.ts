export function usageIqText(bytes?: number | null) {
  if (bytes === null || bytes === undefined)
    return '—'
  return String(bytes)
}

export function usageIqClass(bytes?: number | null) {
  if (bytes === 292)
    return 'bg-cp-green-container text-cp-green-on-container'
  if (bytes === 312)
    return 'bg-cp-red-container text-cp-red-on-container'
  return 'bg-cp-fill-tertiary text-cp-text-secondary'
}
