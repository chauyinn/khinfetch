import { create } from "zustand";
import type { AppSettings, LanguagePreference, ThemePreference } from "../api/generated";

export type NavTab = "search" | "downloads" | "settings";

interface AppState {
  activeTab: NavTab;
  mountedTabs: Record<NavTab, boolean>;
  setActiveTab: (tab: NavTab) => void;
  
  themeSetting: ThemePreference;
  effectiveTheme: "light" | "dark";
  setThemeSetting: (theme: ThemePreference) => void;

  languageSetting: LanguagePreference;
  setLanguageSetting: (language: LanguagePreference) => void;

  settings: AppSettings | null;
  setSettings: (settings: AppSettings) => void;
}

const resolveEffectiveTheme = (setting: ThemePreference): "light" | "dark" => {
  if (setting === "system") {
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return setting === "dark" ? "dark" : "light";
};

const applyThemeToDOM = (effective: "light" | "dark") => {
  document.documentElement.setAttribute("data-theme", effective);
};

const applyLanguageToDOM = (language: LanguagePreference) => {
  document.documentElement.lang = language === "zh-cn" ? "zh-CN" : "en";
};

const LANGUAGE_PREFERENCE_STORAGE_KEY = "khinfetch.languagePreference";

const isLanguagePreference = (value: string | null): value is LanguagePreference =>
  value === "zh-cn" || value === "en";

export const getSystemLanguagePreference = (): LanguagePreference => {
  if (typeof navigator === "undefined") {
    return "en";
  }

  const locale = navigator.languages?.[0] ?? navigator.language;
  return locale?.split("-")[0].toLowerCase() === "zh" ? "zh-cn" : "en";
};

export const getStoredLanguagePreference = (): LanguagePreference | null => {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const stored = window.localStorage.getItem(LANGUAGE_PREFERENCE_STORAGE_KEY);
    return isLanguagePreference(stored) ? stored : null;
  } catch {
    return null;
  }
};

export const rememberLanguagePreference = (language: LanguagePreference) => {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(LANGUAGE_PREFERENCE_STORAGE_KEY, language);
  } catch {
    // The app can still use the selected language for the current session.
  }
};

export const resolveLanguagePreference = (): LanguagePreference =>
  getStoredLanguagePreference() ?? getSystemLanguagePreference();

export const useAppStore = create<AppState>((set, get) => {
  // Listen to system theme changes if matchMedia exists
  if (typeof window !== "undefined" && window.matchMedia) {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (get().themeSetting === "system") {
        const effective = mediaQuery.matches ? "dark" : "light";
        applyThemeToDOM(effective);
        set({ effectiveTheme: effective });
      }
    };
    if (mediaQuery.addEventListener) {
      mediaQuery.addEventListener("change", handleChange);
    } else {
      mediaQuery.addListener(handleChange);
    }
  }

  const initialSetting: ThemePreference = "system";
  const initialEffective = resolveEffectiveTheme(initialSetting);
  const initialLanguage = resolveLanguagePreference();
  applyLanguageToDOM(initialLanguage);

  return {
    activeTab: "search",
    mountedTabs: {
      search: true,
      downloads: false,
      settings: false,
    },
    setActiveTab: (activeTab) =>
      set((state) => ({
        activeTab,
        mountedTabs: { ...state.mountedTabs, [activeTab]: true },
      })),

    themeSetting: initialSetting,
    effectiveTheme: initialEffective,
    setThemeSetting: (themeSetting) => {
      const effective = resolveEffectiveTheme(themeSetting);
      applyThemeToDOM(effective);
      set({ themeSetting, effectiveTheme: effective });
    },

    languageSetting: initialLanguage,
    setLanguageSetting: (languageSetting) => {
      applyLanguageToDOM(languageSetting);
      set({ languageSetting });
    },

    settings: null,
    setSettings: (settings) => set({ settings }),
  };
});
