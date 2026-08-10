import { create } from "zustand";
import type { AlbumSummary, AlbumDetails } from "../api/generated";
import { backendClient } from "../api/client";
import { errorMessage } from "../api/errors";
import { getStrings } from "../i18n/catalog";
import { useAppStore } from "./useAppStore";

export const isDirectSourceInput = (value: string) => /^https?:\/\/\S+$/i.test(value.trim());

interface SearchState {
  query: string;
  setQuery: (query: string) => void;
  
  isSearching: boolean;
  searchResults: AlbumSummary[];
  searchError: string | null;
  
  selectedSourceUrl: string | null;
  selectedAlbumDetails: AlbumDetails | null;
  selectedTrackIds: string[];
  isLoadingDetails: boolean;
  detailsError: string | null;
  
  searchSequence: number;
  detailsSequence: number;

  performSearch: (queryOverride?: string) => Promise<void>;
  loadDetails: (url: string) => Promise<void>;
  setSelectedSourceUrl: (url: string | null) => void;
  setSelectedTrackIds: (ids: string[]) => void;
}

export const useSearchStore = create<SearchState>((set, get) => ({
  query: "",
  setQuery: (query) => set({ query }),

  isSearching: false,
  searchResults: [],
  searchError: null,

  selectedSourceUrl: null,
  selectedAlbumDetails: null,
  selectedTrackIds: [],
  isLoadingDetails: false,
  detailsError: null,

  searchSequence: 0,
  detailsSequence: 0,

  performSearch: async (queryOverride?: string) => {
    const targetQuery = (queryOverride ?? get().query).trim();
    if (!targetQuery) return;

    const seq = get().searchSequence + 1;
    set({
      query: targetQuery,
      isSearching: true,
      searchError: null,
      selectedSourceUrl: null,
      selectedAlbumDetails: null,
      selectedTrackIds: [],
      detailsError: null,
      searchSequence: seq,
      detailsSequence: get().detailsSequence + 1,
    });

    if (isDirectSourceInput(targetQuery)) {
      set({ searchResults: [], selectedSourceUrl: targetQuery });
      await get().loadDetails(targetQuery);
      if (get().searchSequence === seq) {
        set({ isSearching: false });
      }
      return;
    }

    try {
      const results = await backendClient.searchAlbums(targetQuery);
      
      // Prevent race conditions: ignore response if a newer search was initiated
      if (get().searchSequence !== seq) return;

      set({ searchResults: results, isSearching: false });
      if (results.length > 0) {
        set({ selectedSourceUrl: results[0].sourceUrl });
        await get().loadDetails(results[0].sourceUrl);
      } else {
        set({ selectedSourceUrl: null, selectedAlbumDetails: null, selectedTrackIds: [] });
      }
    } catch (error) {
      if (get().searchSequence !== seq) return;
      const strings = getStrings(useAppStore.getState().languageSetting);
      set({
        isSearching: false,
        searchError: errorMessage(error, strings.errors.searchFallback),
      });
    }
  },

  loadDetails: async (url: string) => {
    const seq = get().detailsSequence + 1;
    set({
      isLoadingDetails: true,
      detailsError: null,
      selectedSourceUrl: url,
      detailsSequence: seq,
    });
    try {
      const details = await backendClient.getAlbumDetails(url);
      if (get().detailsSequence !== seq) return;
      set({
        selectedAlbumDetails: details,
        selectedTrackIds: details.tracks.map((track) => track.id),
        isLoadingDetails: false,
      });
    } catch (error) {
      if (get().detailsSequence !== seq) return;
      const strings = getStrings(useAppStore.getState().languageSetting);
      set({
        isLoadingDetails: false,
        detailsError: errorMessage(error, strings.errors.detailsFallback),
      });
    }
  },

  setSelectedSourceUrl: (url) => set({ selectedSourceUrl: url }),
  setSelectedTrackIds: (selectedTrackIds) => set({ selectedTrackIds }),
}));
