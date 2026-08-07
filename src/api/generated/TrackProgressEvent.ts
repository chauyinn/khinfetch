
export type TrackProgressEvent = { jobId: string, trackId: string, bytesReceived: number, bytesTotal: number | null, bytesPerSecond: number, sequence: number, };
