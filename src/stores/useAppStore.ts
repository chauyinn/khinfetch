import { create } from "zustand";
import type { AppSettings, LanguagePreference, ThemePreference } from "../api/generated";

export type NavTab = "search" | "downloads" | "settings";

interface AppState {
  activeTab: NavTab;
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
  const initialLanguage: LanguagePreference = "zh-cn";
  applyLanguageToDOM(initialLanguage);

  return {
    activeTab: "search",
    setActiveTab: (activeTab) => set({ activeTab }),

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
