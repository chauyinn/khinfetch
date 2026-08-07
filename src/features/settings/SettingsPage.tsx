import React, { useEffect, useState } from "react";
import {
  FileText,
  Folder,
  FolderOpen,
  HardDrive,
  Info,
  Languages,
  Sliders,
  SunMoon,
} from "lucide-react";
import type {
  AppSettings,
  AppSettingsPatch,
  LanguagePreference,
  QualityPreference,
  ThemePreference,
} from "../../api/generated";
import { backendClient } from "../../api/client";
import { errorMessage } from "../../api/errors";
import { useAppStore } from "../../stores/useAppStore";
import { useToastStore } from "../../stores/useToastStore";
import { Switch } from "../../components/ui/Switch";
import { useStrings } from "../../i18n/useStrings";
import logo from "../../assets/logo.png";
import styles from "./SettingsPage.module.css";

export const SettingsPage: React.FC = () => {
  const {
    themeSetting,
    setThemeSetting,
    languageSetting,
    setLanguageSetting,
    settings: appSettings,
    setSettings: setAppSettings,
  } = useAppStore();
  const { showToast } = useToastStore();
  const strings = useStrings();
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const settings = appSettings;

  useEffect(() => {
    let active = true;
    backendClient
      .getAppVersion()
      .then((version) => {
        if (active) setAppVersion(version);
      })
      .catch(() => {
        if (active) setAppVersion(null);
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (appSettings) {
      return;
    }
    let active = true;
    backendClient
      .getSettings()
      .then((res) => {
        if (!active) return;
        setAppSettings(res);
        setThemeSetting(res.theme);
        setLanguageSetting(res.language);
      })
      .catch((err) => {
        if (!active) return;
        const message = errorMessage(err, strings.errors.settingsLoadFallback);
        setLoadError(message);
        showToast(
          strings.toast.settingsLoadError(message),
          "error",
        );
      });
    return () => {
      active = false;
    };
  }, [appSettings, setAppSettings, setLanguageSetting, setThemeSetting, showToast, strings]);

  const updatePatch = async (patch: Partial<AppSettings>) => {
    if (!settings || isSaving) return;
    setIsSaving(true);
    try {
      const fullPatch: AppSettingsPatch = {
        downloadDirectory: patch.downloadDirectory ?? null,
        concurrency: patch.concurrency ?? null,
        preferredQuality: patch.preferredQuality ?? null,
        downloadArtwork: patch.downloadArtwork ?? null,
        skipVerifiedFiles: patch.skipVerifiedFiles ?? null,
        loggingEnabled: patch.loggingEnabled ?? null,
        theme: patch.theme ?? null,
        language: patch.language ?? null,
      };
      const updated = await backendClient.updateSettings(fullPatch);
      setAppSettings(updated);
      if (patch.theme) {
        setThemeSetting(patch.theme);
      }
      if (patch.language) {
        setLanguageSetting(patch.language);
      }
    } catch (error) {
      showToast(strings.toast.settingsUpdateError(errorMessage(error, strings.errors.unknown)), "error");
    } finally {
      setIsSaving(false);
    }
  };

  const handleChooseDirectory = async () => {
    try {
      const dir = await backendClient.chooseDownloadDirectory();
      if (dir) {
        await updatePatch({ downloadDirectory: dir });
      }
    } catch (error) {
      showToast(strings.toast.chooseDirError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const handleOpenDownloadDir = async () => {
    try {
      await backendClient.openDownloadDirectory();
    } catch (error) {
      showToast(strings.toast.openDirError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const handleOpenLogDir = async () => {
    try {
      await backendClient.openLogDirectory();
    } catch (error) {
      showToast(strings.toast.openLogsError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const handleOpenExternalUrl = async (url: string) => {
    try {
      await backendClient.openExternalUrl(url);
    } catch (error) {
      showToast(strings.toast.openExternalError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  if (!settings) {
    return <div className={styles.container}>{loadError ?? strings.settings.loading}</div>;
  }

  return (
    <div className={styles.container}>
      {/* 1. 下载设置 */}
      <div className={styles.section}>
        <div className={styles.sectionTitle}>
          <HardDrive size={18} />
          <span>{strings.settings.downloadSection}</span>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.dirLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.dirDesc}</span>
          </div>
          <div className={styles.inputGroup}>
            <input
              type="text"
              readOnly
              className={styles.textInput}
              value={settings.downloadDirectory || strings.settings.dirUnset}
              aria-label={strings.settings.dirLabel}
            />
            <button
              type="button"
              className={styles.button}
              onClick={handleChooseDirectory}
              disabled={isSaving}
            >
              <Folder size={14} />
              <span>{strings.settings.btnChooseDir}</span>
            </button>
            <button
              type="button"
              className={styles.button}
              onClick={handleOpenDownloadDir}
            >
              <FolderOpen size={14} />
              <span>{strings.settings.btnOpenDir}</span>
            </button>
          </div>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.concurrencyLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.concurrencyDesc}</span>
          </div>
          <div className={styles.numberStepper}>
            <button
              type="button"
              className={styles.stepperBtn}
              disabled={settings.concurrency <= 1 || isSaving}
              onClick={() => updatePatch({ concurrency: settings.concurrency - 1 })}
              aria-label={strings.a11y.decreaseConcurrency}
            >
              -
            </button>
            <span className={styles.stepperValue}>{settings.concurrency}</span>
            <button
              type="button"
              className={styles.stepperBtn}
              disabled={settings.concurrency >= 8 || isSaving}
              onClick={() => updatePatch({ concurrency: settings.concurrency + 1 })}
              aria-label={strings.a11y.increaseConcurrency}
            >
              +
            </button>
          </div>
        </div>
      </div>

      {/* 2. 偏好与规则 */}
      <div className={styles.section}>
        <div className={styles.sectionTitle}>
          <Sliders size={18} />
          <span>{strings.settings.preferenceSection}</span>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.qualityLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.qualityDesc}</span>
          </div>
          <div className={styles.segmentedControl} role="radiogroup" aria-label={strings.settings.qualityLabel}>
            <button
              type="button"
              role="radio"
              aria-checked={settings.preferredQuality === "bestAvailable"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${
                settings.preferredQuality === "bestAvailable" ? styles.segmentBtnActive : ""
              }`}
              onClick={() => updatePatch({ preferredQuality: "bestAvailable" as QualityPreference })}
            >
              {strings.settings.qualityBest}
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={settings.preferredQuality === "mp3"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${
                settings.preferredQuality === "mp3" ? styles.segmentBtnActive : ""
              }`}
              onClick={() => updatePatch({ preferredQuality: "mp3" as QualityPreference })}
            >
              {strings.settings.qualityMp3}
            </button>
          </div>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.artworkLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.artworkDesc}</span>
          </div>
          <Switch
            checked={settings.downloadArtwork}
            onCheckedChange={(checked) => updatePatch({ downloadArtwork: checked })}
            disabled={isSaving}
            ariaLabel={strings.settings.artworkLabel}
          />
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.skipVerifiedLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.skipVerifiedDesc}</span>
          </div>
          <Switch
            checked={settings.skipVerifiedFiles}
            onCheckedChange={(checked) => updatePatch({ skipVerifiedFiles: checked })}
            disabled={isSaving}
            ariaLabel={strings.settings.skipVerifiedLabel}
          />
        </div>
      </div>

      {/* 3. 系统与主题 */}
      <div className={styles.section}>
        <div className={styles.sectionTitle}>
          <SunMoon size={18} />
          <span>{strings.settings.systemSection}</span>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>
              <Languages size={14} aria-hidden="true" />
              {strings.settings.languageLabel}
            </span>
            <span className={styles.settingDesc}>{strings.settings.languageDesc}</span>
          </div>
          <div className={styles.segmentedControl} role="radiogroup" aria-label={strings.settings.languageLabel}>
            <button
              type="button"
              role="radio"
              aria-checked={languageSetting === "zh-cn"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${languageSetting === "zh-cn" ? styles.segmentBtnActive : ""}`}
              onClick={() => updatePatch({ language: "zh-cn" as LanguagePreference })}
            >
              {strings.settings.languageZhCn}
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={languageSetting === "en"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${languageSetting === "en" ? styles.segmentBtnActive : ""}`}
              onClick={() => updatePatch({ language: "en" as LanguagePreference })}
            >
              {strings.settings.languageEn}
            </button>
          </div>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.themeLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.themeDesc}</span>
          </div>
          <div className={styles.segmentedControl} role="radiogroup" aria-label={strings.settings.themeLabel}>
            <button
              type="button"
              role="radio"
              aria-checked={themeSetting === "system"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${themeSetting === "system" ? styles.segmentBtnActive : ""}`}
              onClick={() => updatePatch({ theme: "system" as ThemePreference })}
            >
              {strings.settings.themeSystem}
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={themeSetting === "light"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${themeSetting === "light" ? styles.segmentBtnActive : ""}`}
              onClick={() => updatePatch({ theme: "light" as ThemePreference })}
            >
              {strings.settings.themeLight}
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={themeSetting === "dark"}
              disabled={isSaving}
              className={`${styles.segmentBtn} ${themeSetting === "dark" ? styles.segmentBtnActive : ""}`}
              onClick={() => updatePatch({ theme: "dark" as ThemePreference })}
            >
              {strings.settings.themeDark}
            </button>
          </div>
        </div>

        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>
            <span className={styles.settingName}>{strings.settings.loggingLabel}</span>
            <span className={styles.settingDesc}>{strings.settings.loggingDesc}</span>
          </div>
          <div className={styles.inputGroup}>
            <button type="button" className={styles.button} onClick={handleOpenLogDir}>
              <FileText size={14} />
              <span>{strings.settings.btnOpenLogs}</span>
            </button>
          </div>
        </div>
      </div>

      {/* 4. 关于 */}
      <div className={styles.section}>
        <div className={styles.sectionTitle}>
          <Info size={18} />
          <span>{strings.about.sectionTitle}</span>
        </div>

        <div className={styles.aboutHeader}>
          <div className={styles.aboutIdentity}>
            <img className={styles.aboutLogo} src={logo} alt="" aria-hidden="true" />
            <div className={styles.aboutTitleLine}>
              <span className={styles.aboutName}>{strings.about.title}</span>
              {appVersion && <span className={styles.version}>{`v${appVersion}`}</span>}
            </div>
          </div>
          <p className={styles.aboutDesc}>
            {strings.about.descBeforeOfficial}{" "}
            <button
              type="button"
              className={styles.aboutInlineLink}
              onClick={() => handleOpenExternalUrl("https://downloads.khinsider.com")}
            >
              {strings.about.officialSite}
            </button>
            {strings.about.descBeforeReference}
            <button
              type="button"
              className={styles.aboutInlineLink}
              onClick={() => handleOpenExternalUrl("https://github.com/weespin/KhinsiderDownloader")}
            >
              {strings.about.referenceProject}
            </button>
            {strings.about.descAfterReference}
          </p>
        </div>

      </div>
    </div>
  );
};
