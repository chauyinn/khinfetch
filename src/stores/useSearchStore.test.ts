import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AlbumDetails } from "../api/generated";
import { deferred } from "../test/deferred";
import type { useSearchStore } from "./useSearchStore";

const backend = vi.hoisted(() => ({ getAlbumDetails: vi.fn(), searchAlbums: vi.fn() }));
vi.mock("../api/client", () => ({ backendClient: backend }));
vi.mock("./useAppStore", () => ({
  useAppStore: { getState: () => ({ languageSetting: "zh-cn" }) },
}));

const details: AlbumDetails = {
  summary: {
    sourceUrl: "https://downloads.khinsider.com/game-soundtracks/album/example",
    title: "Example", coverUrl: null, year: null, platforms: [], albumType: null, sourceKind: "album",
  },
  publisher: null, developer: null, artworkUrls: [], tracks: [], availableFormats: [],
};

let store: typeof useSearchStore;
beforeEach(async () => {
  vi.resetModules();
  vi.resetAllMocks();
  store = (await import("./useSearchStore")).useSearchStore;
});

describe("search request ordering", () => {
  it.each(["empty", "failed"])("clears pending details when the next search is %s", async (outcome) => {
    const oldDetails = deferred<AlbumDetails>();
    backend.getAlbumDetails.mockReturnValueOnce(oldDetails.promise);
    const loading = store.getState().loadDetails(details.summary.sourceUrl);
    expect(store.getState().isLoadingDetails).toBe(true);
    if (outcome === "empty") backend.searchAlbums.mockResolvedValueOnce([]);
    else backend.searchAlbums.mockRejectedValueOnce(new Error("Search unavailable"));
    await store.getState().performSearch("new query");
    expect(store.getState().isLoadingDetails).toBe(false);
    expect(store.getState().isSearching).toBe(false);
    oldDetails.resolve(details);
    await loading;
    expect(store.getState().selectedAlbumDetails).toBeNull();
    expect(store.getState().isLoadingDetails).toBe(false);
  });

  it("keeps new details loading when an obsolete request fails", async () => {
    const oldDetails = deferred<AlbumDetails>();
    const newDetails = deferred<AlbumDetails>();
    backend.getAlbumDetails.mockReturnValueOnce(oldDetails.promise).mockReturnValueOnce(newDetails.promise);
    backend.searchAlbums.mockResolvedValueOnce([details.summary]);
    const oldLoading = store.getState().loadDetails("old album");
    const searching = store.getState().performSearch("new query");
    await vi.waitFor(() => expect(backend.getAlbumDetails).toHaveBeenCalledTimes(2));
    oldDetails.reject(new Error("Obsolete failure"));
    await oldLoading;
    expect(store.getState().isLoadingDetails).toBe(true);
    expect(store.getState().detailsError).toBeNull();
    newDetails.resolve(details);
    await searching;
    expect(store.getState().selectedAlbumDetails).toEqual(details);
    expect(store.getState().isLoadingDetails).toBe(false);
  });

  it("still loads direct source URLs", async () => {
    backend.getAlbumDetails.mockResolvedValueOnce(details);
    await store.getState().performSearch(details.summary.sourceUrl);
    expect(backend.searchAlbums).not.toHaveBeenCalled();
    expect(store.getState().selectedAlbumDetails).toEqual(details);
    expect(store.getState().isSearching).toBe(false);
    expect(store.getState().isLoadingDetails).toBe(false);
  });
});
