import React, { useEffect, useState } from "react";
import {
  Download,
  ExternalLink,
  FileText,
  Folder,
  FolderOpen,
  HardDrive,
  Info,
  Languages,
  Loader2,
  RefreshCw,
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
import { rememberLanguagePreference, resolveLanguagePreference, useAppStore } from "../../stores/useAppStore";
import { useToastStore } from "../../stores/useToastStore";
import { useJobsStore } from "../../stores/useJobsStore";
import { useUpdateStore } from "../../stores/useUpdateStore";
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
  const activeDownloadsCount = useJobsStore((state) => state.activeDownloadsCount);
  const {
    status: updateStatus,
    update,
    downloadedBytes,
    totalBytes,
    error: updateError,
    checkForUpdate,
    downloadUpdate,
    installUpdate,
    openReleasePage,
  } = useUpdateStore();
  const strings = useStrings();
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const settings = appSettings;
  const isUpdateRunning =
    updateStatus === "checking" ||
    updateStatus === "downloading" ||
    updateStatus === "installing";
  const isUpdateCheckDisabled = isUpdateRunning || updateStatus === "downloaded";
  const downloadPercent = totalBytes && totalBytes > 0
    ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
    : null;

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
        setLanguageSetting(resolveLanguagePreference());
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
        rememberLanguagePreference(patch.language);
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

  const handleOpenReleasePage = async () => {
    try {
      await openReleasePage();
    } catch (error) {
      showToast(strings.toast.openExternalError(errorMessage(error, strings.errors.unknown)), "error");
    }
  };

  const handleInstallUpdate = async () => {
    const installed = await installUpdate();
    if (!installed && useJobsStore.getState().activeDownloadsCount > 0) {
      showToast(strings.toast.updateBlockedByDownloads, "warning");
    }
  };

  const updateStatusText = (() => {
    if (updateStatus === "checking") return strings.about.updateChecking;
    if (updateStatus === "current") return strings.about.updateCurrent;
    if (updateStatus === "available" && updateError) {
      return strings.about.updateError(errorMessage(updateError, strings.errors.unknown));
    }
    if (updateStatus === "downloading") {
      return downloadPercent === null
        ? strings.about.updateDownloading
        : strings.about.updateDownloadingProgress(downloadPercent);
    }
    if (updateStatus === "downloaded") {
      if (updateError) {
        return strings.about.updateError(errorMessage(updateError, strings.errors.unknown));
      }
      return activeDownloadsCount > 0
        ? strings.about.updateWaitingForDownloads(activeDownloadsCount)
        : strings.about.updateReady;
    }
    if (updateStatus === "installing") return strings.about.updateInstalling;
    if (updateStatus === "error") {
      return strings.about.updateError(errorMessage(updateError, strings.errors.unknown));
    }
    if (update) return strings.about.updateAvailable(update.version);
    return null;
  })();

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
          <div className={styles.aboutTopRow}>
            <div className={styles.aboutIdentity}>
              <img className={styles.aboutLogo} src={logo} alt="" aria-hidden="true" />
              <div className={styles.aboutTitleLine}>
                <span className={styles.aboutName}>{strings.about.title}</span>
                {appVersion && <span className={styles.version}>{`v${appVersion}`}</span>}
              </div>
            </div>

            <div className={styles.updateControl}>
              <div className={styles.updateActions}>
                {update && update.environment.installStrategy === "external" && (
                  <button type="button" className={styles.button} onClick={handleOpenReleasePage}>
                    <ExternalLink size={14} />
                    <span>{strings.about.btnOpenRelease}</span>
                  </button>
                )}
                {update && update.environment.installStrategy === "integrated" && updateStatus === "available" && (
                  <button type="button" className={styles.buttonPrimary} onClick={() => void downloadUpdate()}>
                    <Download size={14} />
                    <span>{strings.about.btnDownloadUpdate}</span>
                  </button>
                )}
                {update && update.environment.installStrategy === "integrated" && updateStatus === "downloaded" && (
                  <button
                    type="button"
                    className={styles.buttonPrimary}
                    disabled={activeDownloadsCount > 0}
                    onClick={() => void handleInstallUpdate()}
                  >
                    <RefreshCw size={14} />
                    <span>{strings.about.btnInstallUpdate}</span>
                  </button>
                )}
                <button
                  type="button"
                  className={styles.button}
                  disabled={isUpdateCheckDisabled}
                  onClick={() => void checkForUpdate()}
                >
                  {isUpdateRunning ? <Loader2 size={14} className={styles.spinner} /> : <RefreshCw size={14} />}
                  <span>{strings.about.btnCheckUpdate}</span>
                </button>
              </div>

              {updateStatusText && (
                <span
                  className={`${styles.updateStatus} ${updateError ? styles.updateError : ""}`}
                  aria-live="polite"
                >
                  {updateStatusText}
                </span>
              )}
              {updateStatus === "downloading" && (
                <div
                  className={styles.progressTrack}
                  role="progressbar"
                  aria-label={strings.about.updateDownloadProgress}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={downloadPercent ?? undefined}
                >
                  <span
                    className={`${styles.progressValue} ${downloadPercent === null ? styles.progressIndeterminate : ""}`}
                    style={downloadPercent === null ? undefined : { width: `${downloadPercent}%` }}
                  />
                </div>
              )}
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

        {update?.body && (
          <details className={styles.updateNotes}>
            <summary>{strings.about.updateNotes}</summary>
            <div className={styles.updateNotesBody}>{update.body}</div>
          </details>
        )}

      </div>
    </div>
  );
};
