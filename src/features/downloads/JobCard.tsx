import React from "react";
import { Disc, Pause, Play, RefreshCw, Trash2, XCircle, AlertCircle, CheckCircle2, Clock, ListMusic } from "lucide-react";
import type { AlbumJob, JobState } from "../../api/generated";
import { useJobsStore } from "../../stores/useJobsStore";
import { Tooltip } from "../../components/ui/Tooltip";
import { useStrings } from "../../i18n/useStrings";
import styles from "./DownloadsPage.module.css";

interface JobCardProps {
  job: AlbumJob;
}

export const JobCard: React.FC<JobCardProps> = ({ job }) => {
  const strings = useStrings();
  const { pauseJob, resumeJob, cancelJob, retryJob, removeJob } = useJobsStore();

  const { album, quality, state, selectedTrackTitles = [], completedTracks, totalTracks, error } = job;
  const progressPercent = totalTracks > 0 ? Math.round((completedTracks / totalTracks) * 100) : 0;
  const previewTitles = selectedTrackTitles.slice(0, 2);
  const remainingTracks = Math.max(0, totalTracks - previewTitles.length);
  const selectionSummary = strings.downloads.selectedTracks(
    totalTracks,
    previewTitles.join(strings.downloads.listSeparator),
    remainingTracks,
  );
  const detailTitles = selectedTrackTitles.slice(0, 10);
  const selectionDetails = detailTitles.length > 0
    ? strings.downloads.selectedTrackDetails(detailTitles, selectedTrackTitles.length - detailTitles.length)
    : selectionSummary;

  const renderStateBadge = (s: JobState) => {
    switch (s) {
      case "downloading":
      case "resolving":
        return <span className={`${styles.stateBadge} ${styles.badgeDownloading}`}><Clock size={12} /> {strings.downloads.stateDownloading}</span>;
      case "completed":
        return <span className={`${styles.stateBadge} ${styles.badgeCompleted}`}><CheckCircle2 size={12} /> {strings.downloads.stateCompleted}</span>;
      case "paused":
        return <span className={`${styles.stateBadge} ${styles.badgePaused}`}><Pause size={12} /> {strings.downloads.statePaused}</span>;
      case "interrupted":
        return <span className={`${styles.stateBadge} ${styles.badgePaused}`}><AlertCircle size={12} /> {strings.downloads.stateInterrupted}</span>;
      case "failed":
        return <span className={`${styles.stateBadge} ${styles.badgeFailed}`}><AlertCircle size={12} /> {strings.downloads.stateFailed}</span>;
      case "canceled":
        return <span className={`${styles.stateBadge} ${styles.badgeQueued}`}><XCircle size={12} /> {strings.downloads.stateCanceled}</span>;
      default:
        return <span className={`${styles.stateBadge} ${styles.badgeQueued}`}>{strings.downloads.stateQueued}</span>;
    }
  };

  return (
    <div className={styles.jobCard}>
      <div className={styles.jobMain}>
        <div className={styles.jobCover}>
          {album.coverUrl ? (
            <img src={album.coverUrl} alt={album.title} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
          ) : (
            <Disc size={24} />
          )}
        </div>

        <div className={styles.jobInfo}>
          <div className={styles.jobTitleRow}>
            <Tooltip content={album.title}>
              <span className={`${styles.jobTitle} truncate`}>{album.title}</span>
            </Tooltip>
            <span className={styles.jobQuality}>
              {quality === "mp3" ? strings.downloads.qualityMp3 : strings.downloads.qualityBest}
            </span>
            {renderStateBadge(state)}
          </div>

          <div className={styles.jobSelection}>
            <ListMusic size={13} />
            <Tooltip content={selectionDetails} side="bottom" align="start">
              <span className={`${styles.jobSelectionText} truncate`}>{selectionSummary}</span>
            </Tooltip>
          </div>

          <div className={styles.progressRow}>
            <div
              className={styles.progressBarTrack}
              role="progressbar"
              aria-label={strings.downloads.progressLabel}
              aria-valuemin={0}
              aria-valuemax={totalTracks}
              aria-valuenow={completedTracks}
            >
              <div className={styles.progressBarFill} style={{ width: `${progressPercent}%` }} />
            </div>
            <span>{progressPercent}%</span>
            <span>{strings.downloads.progressTracks(completedTracks, totalTracks)}</span>
          </div>

          {error && (
            <div style={{ fontSize: "var(--font-size-xs)", color: "var(--color-danger)" }}>
              {strings.downloads.errorMessage(error.message)}
            </div>
          )}
        </div>

        {/* Job Action Controls */}
        <div className={styles.jobControls}>
          {(state === "downloading" || state === "resolving" || state === "queued") && (
            <Tooltip content={strings.downloads.btnPause}>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => pauseJob(job.id)}
                aria-label={strings.downloads.btnPause}
              >
                <Pause size={16} />
              </button>
            </Tooltip>
          )}

          {(state === "paused" || state === "interrupted") && (
            <Tooltip content={strings.downloads.btnResume}>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => resumeJob(job.id)}
                aria-label={strings.downloads.btnResume}
              >
                <Play size={16} />
              </button>
            </Tooltip>
          )}

          {state === "failed" && (
            <Tooltip content={strings.downloads.btnRetry}>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => retryJob(job.id)}
                aria-label={strings.downloads.btnRetry}
              >
                <RefreshCw size={16} />
              </button>
            </Tooltip>
          )}

          {(state === "downloading" || state === "resolving" || state === "queued" || state === "paused") && (
            <Tooltip content={strings.downloads.btnCancel}>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => cancelJob(job.id)}
                aria-label={strings.downloads.btnCancel}
              >
                <XCircle size={16} />
              </button>
            </Tooltip>
          )}

          <Tooltip content={strings.downloads.btnRemove}>
            <button
              type="button"
              className={`${styles.iconBtn} ${styles.iconBtnDanger}`}
              onClick={() => removeJob(job.id)}
              aria-label={strings.downloads.btnRemove}
            >
              <Trash2 size={16} />
            </button>
          </Tooltip>
        </div>
      </div>
    </div>
  );
};
