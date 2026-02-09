export const ACCOUNT_LABEL_DELIMITER = " :: "

export function splitAccountScopedLabel(label: string): {
  accountLabel: string | null
  metricLabel: string
} {
  const index = label.lastIndexOf(ACCOUNT_LABEL_DELIMITER)
  if (index < 0) {
    return { accountLabel: null, metricLabel: label }
  }

  const accountLabel = label.slice(0, index).trim()
  const metricLabel = label.slice(index + ACCOUNT_LABEL_DELIMITER.length).trim()
  if (!accountLabel || !metricLabel) {
    return { accountLabel: null, metricLabel: label }
  }

  return { accountLabel, metricLabel }
}

export function getBaseMetricLabel(label: string): string {
  return splitAccountScopedLabel(label).metricLabel
}
