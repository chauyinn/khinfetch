import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircle,
  CheckSquare,
  Disc,
  Download,
  ExternalLink,
  Loader2,
  Pause,
  Play,
  Square,
} from "lucide-react";
import type { AlbumDetails, QualityPreference, TrackPreview, TrackSummary } from "../../api/generated";
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

const FORMAT_RANK = ["FLAC", "ALAC", "OGG", "AAC", "M4A", "MP3"];

const bestFormatForTrack = (track: TrackSummary) => {
  const formats = new Set(track.availableFormats.map((format) => format.toUpperCase()));
  return FORMAT_RANK.find((format) => formats.has(format)) ?? null;
};

const formatBytes = (bytes: number | null) => {
  if (bytes === null) return "--";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit >= 2 ? 2 : unit === 1 ? 1 : 0;
  return `${value.toFixed(digits)} ${units[unit]}`;
};

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
  const selectionAnchorRef = useRef<{ trackId: string; selected: boolean } | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const previewCacheRef = useRef(new Map<string, TrackPreview>());
  const previewRequestRef = useRef(0);
  const [previewTrackId, setPreviewTrackId] = useState<string | null>(null);
  const [previewTitle, setPreviewTitle] = useState("");
  const [previewSource, setPreviewSource] = useState<TrackPreview | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [isPreviewPlaying, setIsPreviewPlaying] = useState(false);

  useEffect(() => () => {
    previewRequestRef.current += 1;
    audioRef.current?.pause();
  }, []);

  useEffect(() => {
    if (!previewSource || !audioRef.current) return;
    audioRef.current.load();
    void audioRef.current.play().catch(() => {
      setIsPreviewPlaying(false);
    });
  }, [previewSource, previewTrackId]);

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
  const selectedBestFormats = Array.from(new Set(
    tracks
      .filter((track) => selectedTrackIds.has(track.id))
      .map(bestFormatForTrack)
      .filter((format): format is string => format !== null),
  ));
  const bestQualityLabel = selectedBestFormats.length === 1
    ? strings.search.qualityBestFormat(selectedBestFormats[0])
    : selectedBestFormats.length > 1
      ? strings.search.qualityMixed
      : strings.search.qualityBest;

  const handleOpenSource = async () => {
    try {
      await backendClient.openExternalUrl(summary.sourceUrl);
    } catch (error) {
      showToast(strings.toast.openExternalError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const toggleSelectAll = () => {
    selectionAnchorRef.current = null;
    if (isAllSelected) {
      setSelectedTrackIds([]);
    } else {
      setSelectedTrackIds(tracks.map((track) => track.id));
    }
  };

  const toggleTrack = (id: string, shiftKey = false) => {
    const next = new Set(selectedTrackIds);
    const anchor = selectionAnchorRef.current;
    if (shiftKey && anchor) {
      const anchorIndex = tracks.findIndex((track) => track.id === anchor.trackId);
      const targetIndex = tracks.findIndex((track) => track.id === id);
      if (anchorIndex >= 0 && targetIndex >= 0) {
        const start = Math.min(anchorIndex, targetIndex);
        const end = Math.max(anchorIndex, targetIndex);
        for (let index = start; index <= end; index += 1) {
          if (anchor.selected) {
            next.add(tracks[index].id);
          } else {
            next.delete(tracks[index].id);
          }
        }
        setSelectedTrackIds(Array.from(next));
        return;
      }
    }
    const shouldSelect = !next.has(id);
    if (shouldSelect) {
      next.add(id);
    } else {
      next.delete(id);
    }
    selectionAnchorRef.current = { trackId: id, selected: shouldSelect };
    setSelectedTrackIds(Array.from(next));
  };

  const togglePreview = async (track: TrackSummary) => {
    if (previewTrackId === track.id && previewSource && audioRef.current) {
      if (audioRef.current.paused) {
        await audioRef.current.play().catch(() => undefined);
      } else {
        audioRef.current.pause();
      }
      return;
    }

    audioRef.current?.pause();
    const request = previewRequestRef.current + 1;
    previewRequestRef.current = request;
    setPreviewTrackId(track.id);
    setPreviewTitle(track.title);
    setPreviewSource(null);
    setIsPreviewPlaying(false);
    setIsPreviewLoading(true);
    try {
      const cached = previewCacheRef.current.get(track.trackPageUrl);
      const preview = cached ?? await backendClient.getTrackPreview(track.trackPageUrl);
      if (previewRequestRef.current !== request) return;
      previewCacheRef.current.set(track.trackPageUrl, preview);
      setPreviewSource(preview);
    } catch (error) {
      if (previewRequestRef.current !== request) return;
      showToast(strings.toast.previewError(errorMessage(error, strings.errors.unknown)), "error");
      setPreviewTrackId(null);
      setPreviewTitle("");
    } finally {
      if (previewRequestRef.current === request) {
        setIsPreviewLoading(false);
      }
    }
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
              {bestQualityLabel}
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

      {(previewTrackId || isPreviewLoading) && (
        <div className={styles.previewBar} aria-live="polite">
          <div className={styles.previewTrackInfo}>
            {isPreviewLoading ? <Loader2 size={15} className={styles.spin} /> : <Play size={15} />}
            <Tooltip content={previewTitle}>
              <span className={`${styles.previewTitle} truncate`}>
                {isPreviewLoading
                  ? strings.search.previewLoading(previewTitle)
                  : strings.search.previewNowPlaying(previewTitle)}
              </span>
            </Tooltip>
          </div>
          {previewSource && (
            // Music previews contain no spoken content or available caption track.
            // eslint-disable-next-line jsx-a11y/media-has-caption
            <audio
              key={previewTrackId}
              ref={audioRef}
              className={styles.previewAudio}
              src={previewSource.mediaUrl}
              controls
              controlsList="nodownload noplaybackrate"
              preload="metadata"
              onPlay={() => setIsPreviewPlaying(true)}
              onPause={() => setIsPreviewPlaying(false)}
              onEnded={() => setIsPreviewPlaying(false)}
            />
          )}
        </div>
      )}

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
              <th style={{ width: 44, textAlign: "center" }}>{strings.search.tableHeaderPreview}</th>
              <th style={{ width: 228 }}>{strings.search.tableHeaderFormats}</th>
            </tr>
          </thead>
          <tbody>
            {tracks.map((track) => {
              const isChecked = selectedTrackIds.has(track.id);
              const hasMp3 = track.availableFormats.some((format) => format.toUpperCase() === "MP3");
              const isCurrentPreview = previewTrackId === track.id;
              const variants = track.availableVariants.length > 0
                ? track.availableVariants
                : track.availableFormats.map((format) => ({ format, sizeBytes: null }));
              return (
                <tr
                  key={track.id}
                  className={styles.trackRow}
                  onClick={(event) => toggleTrack(track.id, event.shiftKey)}
                  tabIndex={0}
                  role="checkbox"
                  aria-checked={isChecked}
                  aria-label={strings.search.selectTrack(track.title)}
                  onKeyDown={(e) => {
                    if (e.key === " " || e.key === "Enter") {
                      e.preventDefault();
                      toggleTrack(track.id, e.shiftKey);
                    }
                  }}
                  style={{ cursor: "pointer" }}
                >
                  <td style={{ textAlign: "center" }} onClick={(e) => e.stopPropagation()}>
                    <button
                      type="button"
                      onClick={(event) => toggleTrack(track.id, event.shiftKey)}
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
                  <td style={{ textAlign: "center" }} onClick={(event) => event.stopPropagation()}>
                    <Tooltip
                      content={hasMp3
                        ? isCurrentPreview && isPreviewPlaying
                          ? strings.search.previewPause
                          : strings.search.previewPlay
                        : strings.search.previewUnavailable}
                    >
                      <span className={styles.previewButtonTooltip}>
                        <button
                          type="button"
                          className={styles.previewButton}
                          disabled={!hasMp3 || (isCurrentPreview && isPreviewLoading)}
                          aria-label={hasMp3 ? strings.search.previewTrack(track.title) : strings.search.previewUnavailable}
                          onClick={() => void togglePreview(track)}
                        >
                          {isCurrentPreview && isPreviewLoading ? (
                            <Loader2 size={15} className={styles.spin} />
                          ) : isCurrentPreview && isPreviewPlaying ? (
                            <Pause size={15} />
                          ) : (
                            <Play size={15} />
                          )}
                        </button>
                      </span>
                    </Tooltip>
                  </td>
                  <td>
                    <div className={styles.formatList}>
                      {variants.map((variant) => (
                        <span key={variant.format} className={styles.tag}>
                          <strong>{variant.format}</strong>
                          <span>{formatBytes(variant.sizeBytes)}</span>
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
