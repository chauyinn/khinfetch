import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

const STORAGE_KEY = "khinfetch.searchResultsWidth";
const MIN_WIDTH = 240;
const MAX_WIDTH = 560;
const MIN_DETAIL_WIDTH = 380;
const DIVIDER_WIDTH = 12;

const getDefaultWidth = () =>
  typeof window !== "undefined" && window.matchMedia?.("(max-width: 950px)").matches ? 270 : 320;

const readStoredWidth = () => {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    const width = stored === null ? NaN : Number(stored);
    return Number.isFinite(width) && width >= MIN_WIDTH && width <= MAX_WIDTH ? width : null;
  } catch {
    return null;
  }
};

const rememberWidth = (width: number | null) => {
  try {
    if (width === null) {
      window.localStorage.removeItem(STORAGE_KEY);
    } else {
      window.localStorage.setItem(STORAGE_KEY, String(width));
    }
  } catch {
    // Resizing still works when local storage is unavailable.
  }
};

export const useSearchPaneResize = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startWidth: number;
    width: number;
    moved: boolean;
  } | null>(null);
  const frameRef = useRef<number | null>(null);
  const [preferredWidth, setPreferredWidth] = useState(readStoredWidth);
  const [isPointerFocused, setIsPointerFocused] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [layout, setLayout] = useState(() => ({
    availableWidth: 0,
    defaultWidth: getDefaultWidth(),
  }));

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver(([entry]) => {
      // Tabs stay mounted while hidden; retain the layout until this tab is visible.
      if (entry.contentRect.width <= 0) return;
      setLayout({
        availableWidth: entry.contentRect.width,
        defaultWidth: getDefaultWidth(),
      });
    });
    observer.observe(container);
    return () => {
      observer.disconnect();
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, []);

  const maxWidth = layout.availableWidth > 0
    ? Math.max(MIN_WIDTH, Math.floor(Math.min(
      MAX_WIDTH,
      layout.availableWidth / 2,
      layout.availableWidth - DIVIDER_WIDTH - MIN_DETAIL_WIDTH,
    )))
    : MAX_WIDTH;
  const clampWidth = (width: number) => Math.min(maxWidth, Math.max(MIN_WIDTH, Math.round(width)));
  const width = clampWidth(preferredWidth ?? layout.defaultWidth);

  const resetWidth = () => {
    setPreferredWidth(null);
    rememberWidth(null);
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !event.isPrimary) return;
    // A keyboard focus ring can persist when clicking an already-focused separator.
    setIsPointerFocused(true);
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startWidth: width,
      width,
      moved: false,
    };
    setIsResizing(true);
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (event.clientX !== drag.startX) {
      drag.moved = true;
    }
    if (!drag.moved) return;
    drag.width = clampWidth(drag.startWidth + event.clientX - drag.startX);
    if (frameRef.current !== null) return;
    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      if (dragRef.current) setPreferredWidth(dragRef.current.width);
    });
  };

  const finishResize = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
    if (drag.moved) {
      const finalWidth = clampWidth(drag.width);
      setPreferredWidth(finalWidth);
      rememberWidth(finalWidth);
    }
    dragRef.current = null;
    setIsResizing(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    setIsPointerFocused(false);
    let nextWidth: number;
    const step = event.shiftKey ? 40 : 16;
    switch (event.key) {
      case "ArrowLeft": nextWidth = width - step; break;
      case "ArrowRight": nextWidth = width + step; break;
      case "Home": nextWidth = MIN_WIDTH; break;
      case "End": nextWidth = maxWidth; break;
      case "Enter":
        event.preventDefault();
        resetWidth();
        return;
      default: return;
    }
    event.preventDefault();
    nextWidth = clampWidth(nextWidth);
    setPreferredWidth(nextWidth);
    rememberWidth(nextWidth);
  };

  return {
    containerRef,
    width,
    minWidth: MIN_WIDTH,
    maxWidth,
    isResizing,
    separatorProps: {
      "data-pointer-focus": isPointerFocused || undefined,
      onBlur: () => setIsPointerFocused(false),
      onPointerDown,
      onPointerMove,
      onPointerUp: finishResize,
      onPointerCancel: finishResize,
      onLostPointerCapture: finishResize,
      onKeyDown,
      onDoubleClick: resetWidth,
    },
  };
};
