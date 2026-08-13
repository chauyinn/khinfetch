export function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string" &&
    error.message.trim()
  ) {
    return error.message;
  }
  return fallback;
}

export function isTransientNetworkError(error: unknown): boolean {
  const message = errorMessage(error, "").toLowerCase();
  return [
    "error sending request",
    "failed to connect",
    "connection refused",
    "connection reset",
    "connection closed",
    "dns error",
    "timed out",
    "timeout",
  ].some((fragment) => message.includes(fragment));
}
