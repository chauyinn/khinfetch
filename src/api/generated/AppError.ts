import type { ErrorCode } from "./ErrorCode";

export type AppError = { code: ErrorCode, message: string, retryable: boolean, messageKey?: string, messageParams?: Record<string, string>, };
