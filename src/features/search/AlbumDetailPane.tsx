import React, { useMemo, useState } from "react";
import { AlertCircle, Disc, Download, CheckSquare, ExternalLink, Square } from "lucide-react";
import type { AlbumDetails, QualityPreference } from "../../api/generated";
import { useJobsStore } from "../../stores/useJobsStore";
import { useAppStore } from "../../stores/useAppStore";
import { useToastStore } from "../../stores/useToastStore";
import { useSearchStore } from "../../stores/useSearchStore";
import { backendClient } from "../../api/client";
import { errorMessage } from "../../api/errors";
import { Tooltip } from "../../components/ui/Tooltip";
import { useStrings } from "../../i18n/useStrings";
import styles from "./SearchPage.module.css";

interface AlbumDetailPaneProps {
  details: AlbumDetails | null;
  isLoading: boolean;
  error: string | null;
}

export const AlbumDetailPane: React.FC<AlbumDetailPaneProps> = ({ details, isLoading, error }) => {
  const strings = useStrings();
  const { setActiveTab, settings } = useAppStore();
  const { refreshJobs } = useJobsStore();
  const { showToast } = useToastStore();
  const { selectedTrackIds: selectedTrackIdList, setSelectedTrackIds } = useSearchStore();
  const [qualityOverride, setQualityOverride] = useState<QualityPreference | null>(null);
  const quality = qualityOverride ?? settings?.preferredQuality ?? "bestAvailable";
  const selectedTrackIds = useMemo(() => new Set(selectedTrackIdList), [selectedTrackIdList]);
  const [isEnqueuing, setIsEnqueuing] = useState(false);

  if (isLoading) {
    return (
      <div className={styles.detailColumn}>
        <div className={styles.emptyState}>
          <Disc size={36} className={styles.emptyIcon} />
          <p>{strings.search.loadingDetails}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={styles.detailColumn}>
        <div className={styles.emptyState}>
          <AlertCircle size={36} style={{ color: "var(--color-danger)" }} />
          <p style={{ color: "var(--color-danger)" }}>{error}</p>
        </div>
      </div>
    );
  }

  if (!details) {
    return (
      <div className={styles.detailColumn}>
        <div className={styles.emptyState}>
          <Disc size={44} className={styles.emptyIcon} />
          <h3>{strings.search.selectPromptTitle}</h3>
          <p>{strings.search.selectPromptDesc}</p>
        </div>
      </div>
    );
  }

  const { summary, publisher, developer, artworkUrls, tracks } = details;
  const isAllSelected = tracks.length > 0 && selectedTrackIds.size === tracks.length;

  const handleOpenSource = async () => {
    try {
      await backendClient.openExternalUrl(summary.sourceUrl);
    } catch (error) {
      showToast(strings.toast.openExternalError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const toggleSelectAll = () => {
    if (isAllSelected) {
      setSelectedTrackIds([]);
    } else {
      setSelectedTrackIds(tracks.map((track) => track.id));
    }
  };

  const toggleTrack = (id: string) => {
    const next = new Set(selectedTrackIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    setSelectedTrackIds(Array.from(next));
  };

  const handleEnqueue = async () => {
    if (selectedTrackIds.size === 0) return;
    setIsEnqueuing(true);
    try {
      await backendClient.enqueueAlbum({
        sourceUrl: summary.sourceUrl,
        quality,
        downloadArtwork: settings?.downloadArtwork ?? false,
        selectedTrackIds: Array.from(selectedTrackIds),
      });
      await refreshJobs();
      showToast(strings.toast.enqueueSuccess(selectedTrackIds.size), "success");
      setActiveTab("downloads");
    } catch (error) {
      showToast(strings.toast.enqueueError(errorMessage(error, strings.errors.unknown)), "error");
    } finally {
      setIsEnqueuing(false);
    }
  };

  const formatDuration = (secs: number | null) => {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s < 10 ? "0" : ""}${s}`;
  };

  return (
    <div className={styles.detailColumn}>
      {/* Header */}
      <div className={styles.detailHeader}>
        <div className={styles.detailCover}>
          {summary.coverUrl ? (
            <img src={summary.coverUrl} alt={summary.title} className={styles.coverImage} />
          ) : (
            <Disc size={36} />
          )}
        </div>
        <div className={styles.detailMeta}>
          <div className={styles.detailTitleRow}>
            <Tooltip content={summary.title}>
              <h2 className={`${styles.detailTitle} truncate`}>{summary.title}</h2>
            </Tooltip>
            <Tooltip content={strings.search.openOnKhinsider}>
              <button
                type="button"
                className={styles.sourceLinkButton}
                aria-label={strings.search.openOnKhinsider}
                onClick={handleOpenSource}
              >
                <ExternalLink size={15} />
              </button>
            </Tooltip>
          </div>
          <div className={styles.detailSubtitle}>
            <span>{strings.search.metadata(strings.search.yearLabel, summary.year || strings.search.unknownYear)}</span>
            <span>{strings.search.metadata(strings.search.typeLabel, summary.albumType || "Soundtrack")}</span>
            {publisher && <span>{strings.search.metadata(strings.search.publisherLabel, publisher)}</span>}
            {developer && <span>{strings.search.metadata(strings.search.developerLabel, developer)}</span>}
            {artworkUrls.length > 0 && (
              <span>{strings.search.metadata(strings.search.artworkLabel, strings.search.artworkCount(artworkUrls.length))}</span>
            )}
          </div>
        </div>
      </div>

      {/* Control Bar */}
      <div className={styles.detailActions}>
        <div className={styles.formatSelector}>
          <span>{strings.search.qualityLabel}</span>
          <div className={styles.segmentedControl} role="radiogroup" aria-label={strings.search.qualityLabel}>
            <button
              type="button"
              role="radio"
              aria-checked={quality === "bestAvailable"}
              className={`${styles.segmentBtn} ${quality === "bestAvailable" ? styles.segmentBtnActive : ""}`}
              onClick={() => setQualityOverride("bestAvailable")}
            >
              {strings.search.qualityBest}
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={quality === "mp3"}
              className={`${styles.segmentBtn} ${quality === "mp3" ? styles.segmentBtnActive : ""}`}
              onClick={() => setQualityOverride("mp3")}
            >
              {strings.search.qualityMp3}
            </button>
          </div>
        </div>

        <button
          type="button"
          className={styles.enqueueBtn}
          disabled={selectedTrackIds.size === 0 || isEnqueuing}
          aria-busy={isEnqueuing}
          onClick={handleEnqueue}
        >
          <Download size={15} />
          <span>{strings.search.downloadSelected(selectedTrackIds.size)}</span>
        </button>
      </div>

      {/* Track Table */}
      <div className={styles.trackTableContainer}>
        <table className={styles.trackTable}>
          <thead>
            <tr>
              <th style={{ width: 36, textAlign: "center" }}>
                <button
                  type="button"
                  onClick={toggleSelectAll}
                  style={{ color: "inherit" }}
                  aria-label={strings.search.selectAllTracks}
                  title={strings.search.selectAllTracks}
                >
                  {isAllSelected ? <CheckSquare size={16} /> : <Square size={16} />}
                </button>
              </th>
              <th style={{ width: 36, textAlign: "center" }}>{strings.search.tableHeaderDisc}</th>
              <th style={{ width: 36, textAlign: "center" }}>{strings.search.tableHeaderIndex}</th>
              <th>{strings.search.tableHeaderTitle}</th>
              <th style={{ width: 60, textAlign: "right" }}>{strings.search.tableHeaderDuration}</th>
              <th style={{ width: 110 }}>{strings.search.tableHeaderFormats}</th>
            </tr>
          </thead>
          <tbody>
            {tracks.map((track) => {
              const isChecked = selectedTrackIds.has(track.id);
              return (
                <tr
                  key={track.id}
                  className={styles.trackRow}
                  onClick={() => toggleTrack(track.id)}
                  tabIndex={0}
                  role="checkbox"
                  aria-checked={isChecked}
                  aria-label={strings.search.selectTrack(track.title)}
                  onKeyDown={(e) => {
                    if (e.key === " " || e.key === "Enter") {
                      e.preventDefault();
                      toggleTrack(track.id);
                    }
                  }}
                  style={{ cursor: "pointer" }}
                >
                  <td style={{ textAlign: "center" }} onClick={(e) => e.stopPropagation()}>
                    <button
                      type="button"
                      onClick={() => toggleTrack(track.id)}
                      style={{ color: "inherit" }}
                      aria-label={strings.search.selectTrack(track.title)}
                    >
                      {isChecked ? (
                        <CheckSquare size={16} style={{ color: "var(--color-accent)" }} />
                      ) : (
                        <Square size={16} style={{ color: "var(--text-muted)" }} />
                      )}
                    </button>
                  </td>
                  <td
                    style={{
                      color: "var(--text-muted)",
                      fontSize: "var(--font-size-xs)",
                      textAlign: "center",
                    }}
                  >
                    {track.discNumber ?? "--"}
                  </td>
                  <td
                    style={{
                      color: "var(--text-muted)",
                      fontSize: "var(--font-size-xs)",
                      textAlign: "center",
                    }}
                  >
                    {track.index ?? "--"}
                  </td>
                  <td>
                    <Tooltip content={track.title}>
                      <span className="truncate" style={{ display: "block" }}>
                        {track.title}
                      </span>
                    </Tooltip>
                  </td>
                  <td style={{ textAlign: "right", color: "var(--text-secondary)", fontSize: "var(--font-size-xs)" }}>
                    {formatDuration(track.durationSeconds)}
                  </td>
                  <td>
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                      {track.availableFormats.map((fmt) => (
                        <span key={fmt} className={styles.tag}>
                          {fmt}
                        </span>
                      ))}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};
