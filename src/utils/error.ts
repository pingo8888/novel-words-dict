export function resolveErrorMessage(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : fallback;
}
