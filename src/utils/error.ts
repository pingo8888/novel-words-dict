export function resolveErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (error && typeof error === "object") {
    const maybeMessage = Reflect.get(error, "message");
    if (typeof maybeMessage === "string" && maybeMessage.trim()) {
      return maybeMessage;
    }

    const fromCause = resolveErrorCause(error);
    if (fromCause) {
      return fromCause;
    }

    const maybeReason = Reflect.get(error, "reason");
    if (typeof maybeReason === "string" && maybeReason.trim()) {
      return maybeReason;
    }
    const maybeError = Reflect.get(error, "error");
    if (typeof maybeError === "string" && maybeError.trim()) {
      return maybeError;
    }
  }
  const fromCause = resolveErrorCause(error);
  if (fromCause) {
    return fromCause;
  }
  return fallback;
}

function resolveErrorCause(error: unknown): string | null {
  if (!(error instanceof Error) || !("cause" in error)) {
    return null;
  }

  const cause = (error as Error & { cause?: unknown }).cause;
  if (typeof cause === "string" && cause.trim()) {
    return cause;
  }
  if (cause instanceof Error && cause.message.trim()) {
    return cause.message;
  }
  if (cause && typeof cause === "object") {
    const maybeMessage = Reflect.get(cause, "message");
    if (typeof maybeMessage === "string" && maybeMessage.trim()) {
      return maybeMessage;
    }
  }
  return null;
}
