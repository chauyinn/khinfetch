import type { AppError } from "./AppError";

export type EnqueueFailure = { sourceUrl: string, error: AppError, };
