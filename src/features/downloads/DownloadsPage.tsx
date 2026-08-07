import React from "react";
import { Download, Pause, Play, RefreshCw, Disc } from "lucide-react";
import { useJobsStore } from "../../stores/useJobsStore";
import { JobCard } from "./JobCard";
import { useStrings } from "../../i18n/useStrings";
import styles from "./DownloadsPage.module.css";

export const DownloadsPage: React.FC = () => {
  const strings = useStrings();
  const { jobs, activeDownloadsCount, pauseAllJobs, resumeAllJobs, refreshJobs } = useJobsStore();

  const activeJobs = jobs.filter((j) => j.state === "downloading" || j.state === "resolving" || j.state === "queued");
  const pausedJobs = jobs.filter((j) => j.state === "paused" || j.state === "interrupted");

  return (
    <div className={styles.container}>
      {/* Header Bar */}
      <div className={styles.header}>
        <div className={styles.stats}>
          <div className={styles.statItem}>
            <Download size={16} />
            <span>{strings.downloads.headerTotal(jobs.length)}</span>
          </div>
          <div className={styles.statItem}>
            <span>{strings.downloads.headerActive(activeDownloadsCount)}</span>
          </div>
        </div>

        <div className={styles.actions}>
          <button
            type="button"
            className={styles.actionBtn}
            onClick={pauseAllJobs}
            disabled={activeJobs.length === 0}
            aria-label={strings.downloads.pauseAll}
          >
            <Pause size={14} />
            <span>{strings.downloads.pauseAll}</span>
          </button>
          <button
            type="button"
            className={styles.actionBtn}
            onClick={resumeAllJobs}
            disabled={pausedJobs.length === 0}
            aria-label={strings.downloads.resumeAll}
          >
            <Play size={14} />
            <span>{strings.downloads.resumeAll}</span>
          </button>
          <button
            type="button"
            className={styles.actionBtn}
            onClick={() => refreshJobs()}
            aria-label={strings.downloads.refresh}
          >
            <RefreshCw size={14} />
            <span>{strings.downloads.refresh}</span>
          </button>
        </div>
      </div>

      {/* Queue Items */}
      <div className={styles.queueList}>
        {jobs.length === 0 ? (
          <div className={styles.emptyState}>
            <Disc size={48} />
            <h3>{strings.downloads.emptyTitle}</h3>
            <p>{strings.downloads.emptyDesc}</p>
          </div>
        ) : (
          jobs.map((job) => <JobCard key={job.id} job={job} />)
        )}
      </div>
    </div>
  );
};
