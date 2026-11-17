import { useId } from "react";
import type { ReactNode } from "react";
import { GripVertical } from "lucide-react";
import { useStrings } from "../../i18n/useStrings";
import { useSearchPaneResize } from "./useSearchPaneResize";
import styles from "./SearchPage.module.css";

interface SearchSplitPaneProps {
  results: ReactNode;
  children: ReactNode;
}

export const SearchSplitPane = ({ results, children }: SearchSplitPaneProps) => {
  const strings = useStrings();
  const resultsPaneId = useId();
  const { containerRef, width, minWidth, maxWidth, isResizing, separatorProps } = useSearchPaneResize();

  // Keep resize state here so pointer movement does not re-render either pane's content.
  return (
    <div ref={containerRef} className={`${styles.contentSplit} ${isResizing ? styles.resizing : ""}`}>
      <div id={resultsPaneId} className={styles.listColumn} style={{ width }}>
        {results}
      </div>

      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- Resizable ARIA separators support keyboard focus. */}
      <div role="separator" tabIndex={0}
        aria-label={strings.search.resizePanels}
        aria-orientation="vertical"
        aria-controls={resultsPaneId}
        aria-valuemin={minWidth}
        aria-valuemax={maxWidth}
        aria-valuenow={width}
        aria-valuetext={strings.search.resultsWidth(width)}
        title={strings.search.resizePanelsHint}
        className={styles.paneDivider}
        {...separatorProps}
      >
        <span className={styles.paneDividerGrip} aria-hidden="true">
          <GripVertical size={12} />
        </span>
      </div>

      {children}
    </div>
  );
};
