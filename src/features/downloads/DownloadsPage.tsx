import React, { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { Disc, Download, Loader2, Pause, Play, RefreshCw, Trash2 } from "lucide-react";
import { useJobsStore } from "../../stores/useJobsStore";
import { JobCard } from "./JobCard";
import { useStrings } from "../../i18n/useStrings";
import styles from "./DownloadsPage.module.css";

export const DownloadsPage: React.FC = () => {
  const strings = useStrings();
  const { jobs, activeDownloadsCount, pauseAllJobs, resumeAllJobs, removeAllJobs, refreshJobs } = useJobsStore();
  const [clearDialogOpen, setClearDialogOpen] = useState(false);
  const [isClearing, setIsClearing] = useState(false);

  const activeJobs = jobs.filter((j) => j.state === "downloading" || j.state === "resolving" || j.state === "waiting" || j.state === "queued");
  const pausedJobs = jobs.filter((j) => j.state === "paused" || j.state === "interrupted");

  const handleClearQueue = async () => {
    setIsClearing(true);
    try {
      await removeAllJobs();
      setClearDialogOpen(false);
    } finally {
      setIsClearing(false);
    }
  };

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
          <Dialog.Root open={clearDialogOpen} onOpenChange={(open) => !isClearing && setClearDialogOpen(open)}>
            <Dialog.Trigger asChild>
              <button
                type="button"
                className={`${styles.actionBtn} ${styles.dangerActionBtn}`}
                disabled={jobs.length === 0}
                aria-label={strings.downloads.clearQueue}
              >
                <Trash2 size={14} />
                <span>{strings.downloads.clearQueue}</span>
              </button>
            </Dialog.Trigger>

            <Dialog.Portal>
              <Dialog.Overlay className={styles.confirmOverlay} />
              <Dialog.Content
                className={styles.confirmContent}
                onEscapeKeyDown={(event) => isClearing && event.preventDefault()}
                onInteractOutside={(event) => isClearing && event.preventDefault()}
              >
                <Dialog.Title className={styles.confirmTitle}>{strings.downloads.clearQueueTitle}</Dialog.Title>
                <Dialog.Description className={styles.confirmDescription}>
                  {strings.downloads.clearQueueDescription(jobs.length)}
                </Dialog.Description>
                <div className={styles.confirmActions}>
                  <Dialog.Close asChild>
                    <button type="button" className={styles.confirmCancelBtn} disabled={isClearing}>
                      {strings.downloads.clearQueueCancel}
                    </button>
                  </Dialog.Close>
                  <button
                    type="button"
                    className={styles.confirmDangerBtn}
                    disabled={isClearing}
                    aria-busy={isClearing}
                    onClick={() => void handleClearQueue()}
                  >
                    {isClearing ? <Loader2 size={15} className={styles.spinner} /> : <Trash2 size={15} />}
                    <span>{isClearing ? strings.downloads.clearingQueue : strings.downloads.clearQueueConfirm}</span>
                  </button>
                </div>
              </Dialog.Content>
            </Dialog.Portal>
          </Dialog.Root>
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
