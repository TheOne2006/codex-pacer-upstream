export function isErrorLikeStatus(message: string) {
  return /error|invalid|unsupported|failed/i.test(message)
}
