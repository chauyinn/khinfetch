import type { BackendNotice } from "./generated";
import { getStrings } from "../i18n/catalog";
import { useAppStore } from "../stores/useAppStore";

function resolveMessageKey(
  messageKey: string,
  params: Record<string, string> | undefined,
): string | null {
  const backend = getStrings(useAppStore.getState().languageSetting).errors.backend;
  switch (messageKey) {
    case "rate_limited_retry_after":
      return backend.rate_limited_retry_after(params?.seconds ?? "");
    case "site_http_status":
      return backend.site_http_status(params?.status ?? "");
    case "download_http_status":
      return backend.download_http_status(params?.status ?? "");
    case "network_request_failed":
      return backend.network_request_failed(params?.detail ?? "");
    case "page_read_failed":
      return backend.page_read_failed(params?.detail ?? "");
    case "parse_structure_missing":
      return backend.parse_structure_missing(params?.detail ?? "");
    case "parse_invalid_link":
      return backend.parse_invalid_link(params?.detail ?? "");
    case "download_request_failed":
      return backend.download_request_failed(params?.detail ?? "");
    case "download_stream_interrupted":
      return backend.download_stream_interrupted(params?.detail ?? "");
    case "open_download_dir_failed":
      return backend.open_download_dir_failed(params?.detail ?? "");
    case "choose_dir_read_failed":
      return backend.choose_dir_read_failed(params?.detail ?? "");
    case "create_log_dir_failed":
      return backend.create_log_dir_failed(params?.detail ?? "");
    case "open_log_dir_failed":
      return backend.open_log_dir_failed(params?.detail ?? "");
    case "open_external_url_failed":
      return backend.open_external_url_failed(params?.detail ?? "");
    case "file_operation_failed":
      return backend.file_operation_failed(params?.detail ?? "");
    default: {
      const entry = backend[messageKey as keyof typeof backend];
      return typeof entry === "string" ? entry : null;
    }
  }
}

export function noticeMessage(notice: BackendNotice): string {
  const strings = getStrings(useAppStore.getState().languageSetting);
  if (notice.code === "artwork_unavailable") {
    return strings.notices.artworkUnavailable;
  }
  return notice.message;
}

export function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (typeof error === "object" && error !== null) {
    if ("messageKey" in error && typeof error.messageKey === "string" && error.messageKey.trim()) {
      const params =
        "messageParams" in error && typeof error.messageParams === "object" && error.messageParams !== null
          ? (error.messageParams as Record<string, string>)
          : undefined;
      const localized = resolveMessageKey(error.messageKey, params);
      if (localized) {
        return localized;
      }
    }
    if ("message" in error && typeof error.message === "string" && error.message.trim()) {
      return error.message;
    }
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
