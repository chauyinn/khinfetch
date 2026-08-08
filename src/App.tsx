import { AppLayout } from "./components/layout/AppLayout";
import { useAppStore } from "./stores/useAppStore";
import { SearchPage } from "./features/search/SearchPage";
import { DownloadsPage } from "./features/downloads/DownloadsPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import "./styles/global.css";
import styles from "./App.module.css";

export function App() {
  const { activeTab, mountedTabs } = useAppStore();

  return (
    <AppLayout>
      {mountedTabs.search && (
        <div className={styles.page} hidden={activeTab !== "search"}>
          <SearchPage />
        </div>
      )}
      {mountedTabs.downloads && (
        <div className={styles.page} hidden={activeTab !== "downloads"}>
          <DownloadsPage />
        </div>
      )}
      {mountedTabs.settings && (
        <div className={styles.page} hidden={activeTab !== "settings"}>
          <SettingsPage />
        </div>
      )}
    </AppLayout>
  );
}

export default App;
