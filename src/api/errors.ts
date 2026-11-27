import { getStrings } from "../i18n/catalog";
import { strings as zhCnStrings } from "../i18n/zh-CN";
import { useAppStore } from "../stores/useAppStore";

function localizeBackendMessage(message: string): string {
  const strings = getStrings(useAppStore.getState().languageSetting);
  if (message === zhCnStrings.errors.cloudflareChallenge) {
    return strings.errors.cloudflareChallenge;
  }
  if (message === zhCnStrings.errors.jobDirectoryNotFound) {
    return strings.errors.jobDirectoryNotFound;
  }
  if (message === zhCnStrings.errors.jobDirectoryMissing) {
    return strings.errors.jobDirectoryMissing;
  }
  return message;
}

export function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return localizeBackendMessage(error);
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string" &&
    error.message.trim()
  ) {
    return localizeBackendMessage(error.message);
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
