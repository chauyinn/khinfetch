import React, { useEffect } from "react";
import { Search, Download, Settings } from "lucide-react";
import { resolveLanguagePreference, useAppStore, type NavTab } from "../../stores/useAppStore";
import { useJobsStore } from "../../stores/useJobsStore";
import { TooltipProvider } from "../ui/Tooltip";
import { ToastContainer } from "../ui/Toast";
import { backendClient } from "../../api/client";
import { errorMessage } from "../../api/errors";
import { getStrings } from "../../i18n/catalog";
import { useStrings } from "../../i18n/useStrings";
import { useToastStore } from "../../stores/useToastStore";
import { useUpdateStore } from "../../stores/useUpdateStore";
import logo from "../../assets/logo.png";
import styles from "./AppLayout.module.css";

interface AppLayoutProps {
  children: React.ReactNode;
}

export const AppLayout: React.FC<AppLayoutProps> = ({ children }) => {
  const { activeTab, setActiveTab, setSettings, setThemeSetting, setLanguageSetting } = useAppStore();
  const { activeDownloadsCount, initialize } = useJobsStore();
  const { showToast } = useToastStore();
  const checkForUpdate = useUpdateStore((state) => state.checkForUpdate);
  const strings = useStrings();

  useEffect(() => {
    const suppressContextMenu = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable='true']")) {
        return;
      }
      event.preventDefault();
    };
    document.addEventListener("contextmenu", suppressContextMenu);
    return () => document.removeEventListener("contextmenu", suppressContextMenu);
  }, []);

  useEffect(() => {
    // Initialize jobs store & listeners
    const cleanupPromise = initialize();
    return () => {
      cleanupPromise.then((cleanup) => cleanup && cleanup());
    };
  }, [initialize]);

  useEffect(() => {
    void checkForUpdate(true);
  }, [checkForUpdate]);

  useEffect(() => {
    let active = true;
    backendClient
      .getSettings()
      .then((settings) => {
        if (!active) return;
        setSettings(settings);
        setThemeSetting(settings.theme);
        // First launch follows the system; explicit choices are tracked in local storage.
        setLanguageSetting(resolveLanguagePreference());
      })
      .catch((error) => {
        if (!active) return;
        const currentStrings = getStrings(useAppStore.getState().languageSetting);
        showToast(
          currentStrings.toast.settingsLoadError(
            errorMessage(error, currentStrings.errors.settingsLoadFallback),
          ),
          "error",
        );
      });
    return () => {
      active = false;
    };
  }, [setLanguageSetting, setSettings, setThemeSetting, showToast]);

  const navItems: { id: NavTab; label: string; icon: React.ReactNode; badge?: number }[] = [
    { id: "search", label: strings.nav.search, icon: <Search size={18} /> },
    { id: "downloads", label: strings.nav.downloads, icon: <Download size={18} />, badge: activeDownloadsCount },
    { id: "settings", label: strings.nav.settings, icon: <Settings size={18} /> },
  ];

  return (
    <TooltipProvider>
      <div className={styles.layout}>
        <aside className={styles.sidebar}>
          <div>
            <div className={styles.brand}>
              <img className={styles.brandIcon} src={logo} alt="" aria-hidden="true" />
              <span className={styles.brandTitle}>KhinFetch</span>
            </div>

            <nav className={styles.nav} aria-label={strings.a11y.mainNavigation}>
              {navItems.map((item) => {
                const isActive = activeTab === item.id;
                return (
                  <button
                    key={item.id}
                    type="button"
                    className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                    onClick={() => setActiveTab(item.id)}
                    aria-current={isActive ? "page" : undefined}
                  >
                    {item.icon}
                    <span>{item.label}</span>
                    {item.badge !== undefined && item.badge > 0 && (
                      <span className={styles.navBadge} aria-label={strings.a11y.activeDownloads(item.badge)}>
                        {item.badge}
                      </span>
                    )}
                  </button>
                );
              })}
            </nav>
          </div>

        </aside>

        <main className={styles.mainContent}>
          {children}
          <ToastContainer />
        </main>
      </div>
    </TooltipProvider>
  );
};
