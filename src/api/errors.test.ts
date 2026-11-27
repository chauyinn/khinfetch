import { describe, expect, it, vi } from "vitest";
import type { AppError, BackendNotice } from "./generated";

const state = vi.hoisted(() => ({ languageSetting: "zh-cn" as "zh-cn" | "en" }));

vi.mock("../stores/useAppStore", () => ({
  useAppStore: { getState: () => state },
}));

import { errorMessage, noticeMessage } from "./errors";

describe("backend error and notice localization", () => {
  it("resolves structured messageKey and messageParams by active language", () => {
    const cloudflareError: AppError = {
      code: "network",
      message: "fallback",
      retryable: false,
      messageKey: "cloudflare_challenge",
    };
    const httpError: AppError = {
      code: "network",
      message: "站点返回 HTTP 502 Bad Gateway",
      retryable: true,
      messageKey: "site_http_status",
      messageParams: { status: "502 Bad Gateway" },
    };

    state.languageSetting = "zh-cn";
    expect(errorMessage(cloudflareError, "fallback")).toContain("Cloudflare 安全验证");
    expect(errorMessage(httpError, "fallback")).toBe("站点返回 HTTP 502 Bad Gateway");

    state.languageSetting = "en";
    expect(errorMessage(cloudflareError, "fallback")).toContain("Cloudflare security challenge");
    expect(errorMessage(httpError, "fallback")).toBe("Site returned HTTP 502 Bad Gateway");
  });

  it("falls back to raw message for unknown keys or legacy errors", () => {
    state.languageSetting = "en";
    expect(
      errorMessage(
        { code: "internal", message: "Custom error", retryable: false, messageKey: "unknown_key" },
        "fallback",
      ),
    ).toBe("Custom error");
    expect(errorMessage({ message: "Legacy error" }, "fallback")).toBe("Legacy error");
  });

  it("localizes backend notices by code", () => {
    const notice: BackendNotice = {
      code: "artwork_unavailable",
      message: "专辑页面没有提供可下载原图",
      jobId: "job-1",
    };

    state.languageSetting = "zh-cn";
    expect(noticeMessage(notice)).toBe("专辑页面没有提供可下载原图");

    state.languageSetting = "en";
    expect(noticeMessage(notice)).toBe("No downloadable original artwork is available on this album page");
  });
});
