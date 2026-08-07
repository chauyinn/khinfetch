import type { ErrorCode } from "./ErrorCode";

export type AppError = { code: ErrorCode, message: string, retryable: boolean, };
