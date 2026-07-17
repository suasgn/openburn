export function isMobileTauri(): boolean {
  return /Android|iPhone|iPad|iPod/i.test(navigator.userAgent)
}
