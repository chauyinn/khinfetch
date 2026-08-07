import type { AlbumJob } from "./AlbumJob";
import type { EnqueueFailure } from "./EnqueueFailure";

export type BatchEnqueueResult = { accepted: Array<AlbumJob>, rejected: Array<EnqueueFailure>, };
