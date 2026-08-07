import React from "react";
import { Search as SearchIcon, Disc, Loader2 } from "lucide-react";
import type { AlbumSummary } from "../../api/generated";
import { useSearchStore } from "../../stores/useSearchStore";
import { Tooltip } from "../../components/ui/Tooltip";
import { AlbumDetailPane } from "./AlbumDetailPane";
import { BatchEnqueueDialog } from "./BatchEnqueueDialog";
import { useStrings } from "../../i18n/useStrings";
import styles from "./SearchPage.module.css";

export const SearchPage: React.FC = () => {
  const strings = useStrings();
  const {
    query,
    setQuery,
    isSearching,
    searchResults,
    searchError,
    selectedSourceUrl,
    selectedAlbumDetails,
    isLoadingDetails,
    detailsError,
    performSearch,
    loadDetails,
  } = useSearchStore();

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    performSearch();
  };

  const handleSelectAlbum = (album: AlbumSummary) => {
    loadDetails(album.sourceUrl);
  };

  return (
    <div className={styles.container}>
      {/* Search Header */}
      <div className={styles.searchHeader}>
        <form className={styles.searchForm} onSubmit={handleSearchSubmit}>
          <div className={styles.searchInputWrapper}>
            <SearchIcon size={18} className={styles.searchInputIcon} />
            <input
              type="text"
              className={styles.searchInput}
              placeholder={strings.search.placeholder}
              aria-label={strings.search.placeholder}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <button
            type="submit"
            className={styles.searchButton}
            disabled={isSearching || !query.trim()}
            aria-busy={isSearching}
          >
            {isSearching ? <Loader2 size={16} className="animate-spin" /> : <SearchIcon size={16} />}
            <span>{strings.search.submitBtn}</span>
          </button>
        </form>
        <BatchEnqueueDialog />
      </div>

      {/* Content Split: List on Left, Detail on Right */}
      <div className={styles.contentSplit}>
        <div className={styles.listColumn}>
          <div className={styles.listHeader}>
            <span>{strings.search.resultsCount(searchResults.length)}</span>
          </div>

          <div className={styles.listScroll} role="listbox" aria-label={strings.search.resultsCount(searchResults.length)}>
            {isSearching ? (
              <div className={styles.emptyState}>
                <Loader2 size={32} className={styles.emptyIcon} />
                <p>{strings.search.searching}</p>
              </div>
            ) : searchError ? (
              <div className={styles.emptyState}>
                <p style={{ color: "var(--color-danger)" }}>{searchError}</p>
              </div>
            ) : searchResults.length === 0 ? (
              <div className={styles.emptyState}>
                <Disc size={36} className={styles.emptyIcon} />
                <p>{strings.search.emptyPrompt}</p>
              </div>
            ) : (
              searchResults.map((album: AlbumSummary) => {
                const isSelected = selectedSourceUrl === album.sourceUrl;
                const platformText = album.platforms.join(" / ");
                return (
                  <div
                    key={album.sourceUrl}
                    role="option"
                    aria-selected={isSelected}
                    tabIndex={0}
                    className={`${styles.albumCard} ${isSelected ? styles.albumCardSelected : ""}`}
                    onClick={() => handleSelectAlbum(album)}
                    onContextMenu={(e) => e.preventDefault()}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSelectAlbum(album);
                      }
                    }}
                  >
                    <div className={styles.coverPlaceholder}>
                      {album.coverUrl ? (
                        <img src={album.coverUrl} alt={album.title} className={styles.coverImage} />
                      ) : (
                        <Disc size={24} />
                      )}
                    </div>
                    <div className={styles.albumInfo}>
                      <Tooltip content={album.title}>
                        <span className={`${styles.albumTitle} truncate`}>{album.title}</span>
                      </Tooltip>
                      <div className={styles.albumMeta}>
                        <span className={styles.albumYear}>{album.year || strings.search.unknownYear}</span>
                        {album.platforms.length > 0 && (
                          <Tooltip content={platformText}>
                            <span className={styles.platformList}>{platformText}</span>
                          </Tooltip>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Detail Pane */}
        <AlbumDetailPane
          key={selectedAlbumDetails?.summary.sourceUrl ?? "empty"}
          details={selectedAlbumDetails}
          isLoading={isLoadingDetails}
          error={detailsError}
        />
      </div>
    </div>
  );
};
