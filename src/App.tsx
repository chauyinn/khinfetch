import { AppLayout } from "./components/layout/AppLayout";
import { useAppStore } from "./stores/useAppStore";
import { SearchPage } from "./features/search/SearchPage";
import { DownloadsPage } from "./features/downloads/DownloadsPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import "./styles/global.css";

export function App() {
  const { activeTab } = useAppStore();

  const renderActiveTab = () => {
    switch (activeTab) {
      case "search":
        return <SearchPage />;
      case "downloads":
        return <DownloadsPage />;
      case "settings":
        return <SettingsPage />;
      default:
        return <SearchPage />;
    }
  };

  return <AppLayout>{renderActiveTab()}</AppLayout>;
}

export default App;
