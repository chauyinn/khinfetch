import { useMemo, useRef, useState, type FormEvent } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { AlertCircle, ListPlus, Loader2, X } from "lucide-react";
import type { BatchEnqueueResult, QualityPreference } from "../../api/generated";
import { backendClient } from "../../api/client";
import { errorMessage } from "../../api/errors";
import { Tooltip } from "../../components/ui/Tooltip";
import { useStrings } from "../../i18n/useStrings";
import { useAppStore } from "../../stores/useAppStore";
import { useJobsStore } from "../../stores/useJobsStore";
import { useToastStore } from "../../stores/useToastStore";
import styles from "./BatchEnqueueDialog.module.css";

export const MAX_BATCH_URLS = 100;

export function parseSourceUrls(value: string): string[] {
  return Array.from(new Set(value.split(/\s+/u).map((item) => item.trim()).filter(Boolean)));
}

export function BatchEnqueueDialog() {
  const strings = useStrings();
  const { settings, setActiveTab } = useAppStore();
  const { refreshJobs } = useJobsStore();
  const { showToast } = useToastStore();
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [quality, setQuality] = useState<QualityPreference>(
    settings?.preferredQuality ?? "bestAvailable",
  );
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [hasSubmitted, setHasSubmitted] = useState(false);
  const [result, setResult] = useState<BatchEnqueueResult | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sourceUrls = useMemo(() => parseSourceUrls(input), [input]);
  const exceedsLimit = sourceUrls.length > MAX_BATCH_URLS;

  const prepareOpen = () => {
    setQuality(settings?.preferredQuality ?? "bestAvailable");
    setHasSubmitted(false);
    setResult(null);
    setSubmitError(null);
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (isSubmitting) return;
    setOpen(nextOpen);
  };

  const handleInputChange = (value: string) => {
    setInput(value);
    setHasSubmitted(false);
    setResult(null);
    setSubmitError(null);
  };

  const showQueue = () => {
    setOpen(false);
    setActiveTab("downloads");
  };

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    setHasSubmitted(true);
    setResult(null);
    setSubmitError(null);
    if (sourceUrls.length === 0 || exceedsLimit) return;

    setIsSubmitting(true);
    try {
      const nextResult = await backendClient.enqueueUrls({
        sourceUrls,
        quality,
        downloadArtwork: settings?.downloadArtwork ?? false,
      });
      setResult(nextResult);
      await refreshJobs();

      if (nextResult.rejected.length === 0) {
        showToast(strings.toast.batchSuccess(nextResult.accepted.length), "success");
        setInput("");
        setOpen(false);
        setActiveTab("downloads");
      } else if (nextResult.accepted.length > 0) {
        showToast(
          strings.toast.batchPartial(nextResult.accepted.length, nextResult.rejected.length),
          "warning",
        );
      }
    } catch (error) {
      const message = errorMessage(error, strings.errors.unknown);
      setSubmitError(message);
      showToast(strings.toast.batchError(message), "error");
    } finally {
      setIsSubmitting(false);
    }
  };

  const validationError = exceedsLimit
    ? strings.batch.tooManyError(MAX_BATCH_URLS)
    : hasSubmitted && sourceUrls.length === 0
      ? strings.batch.emptyError
      : null;

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Trigger asChild>
        <button type="button" className={styles.triggerButton} onClick={prepareOpen}>
          <ListPlus size={16} />
          <span>{strings.search.batchBtn}</span>
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content
          className={styles.content}
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            textareaRef.current?.focus();
          }}
        >
          <div className={styles.header}>
            <div>
              <Dialog.Title className={styles.title}>{strings.batch.title}</Dialog.Title>
              <Dialog.Description className={styles.srOnly}>
                {strings.batch.description}
              </Dialog.Description>
            </div>
            <Tooltip content={strings.batch.close}>
              <Dialog.Close asChild>
                <button type="button" className={styles.iconButton} aria-label={strings.batch.close}>
                  <X size={17} />
                </button>
              </Dialog.Close>
            </Tooltip>
          </div>

          <form className={styles.form} onSubmit={handleSubmit}>
            <div className={styles.field}>
              <div className={styles.labelRow}>
                <label htmlFor="batch-source-urls" className={styles.label}>
                  {strings.batch.urlLabel}
                </label>
                <span className={exceedsLimit ? styles.countError : styles.count}>
                  {strings.batch.urlCount(sourceUrls.length, MAX_BATCH_URLS)}
                </span>
              </div>
              <textarea
                ref={textareaRef}
                id="batch-source-urls"
                className={styles.textarea}
                value={input}
                onChange={(event) => handleInputChange(event.target.value)}
                placeholder={strings.batch.urlPlaceholder}
                aria-invalid={Boolean(validationError)}
                spellCheck={false}
              />
              {validationError && <p className={styles.fieldError}>{validationError}</p>}
            </div>

            <div className={styles.actionRow}>
              <div className={styles.optionGroup}>
                <span className={styles.optionLabel}>{strings.batch.qualityLabel}</span>
                <div className={styles.segmentedControl} role="radiogroup" aria-label={strings.batch.qualityLabel}>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={quality === "bestAvailable"}
                    className={`${styles.segmentButton} ${quality === "bestAvailable" ? styles.segmentButtonActive : ""}`}
                    onClick={() => setQuality("bestAvailable")}
                    disabled={isSubmitting}
                  >
                    {strings.batch.qualityBest}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={quality === "mp3"}
                    className={`${styles.segmentButton} ${quality === "mp3" ? styles.segmentButtonActive : ""}`}
                    onClick={() => setQuality("mp3")}
                    disabled={isSubmitting}
                  >
                    {strings.batch.qualityMp3}
                  </button>
                </div>
              </div>
              <button
                type="submit"
                className={styles.primaryButton}
                disabled={isSubmitting || exceedsLimit}
                aria-busy={isSubmitting}
              >
                {isSubmitting ? <Loader2 size={16} className={styles.spinner} /> : <ListPlus size={16} />}
                <span>{isSubmitting ? strings.batch.submitting : strings.batch.submit}</span>
              </button>
            </div>

            {(submitError || result?.rejected.length) && (
              <div className={styles.resultPanel} role="status">
                <div className={styles.resultSummary}>
                  <AlertCircle size={16} />
                  <span>
                    {submitError
                      ? submitError
                      : result && result.accepted.length > 0
                        ? strings.batch.partial(result.accepted.length, result.rejected.length)
                        : strings.batch.rejected(result?.rejected.length ?? 0)}
                  </span>
                </div>
                {result && result.rejected.length > 0 && (
                  <div className={styles.rejectedBlock}>
                    <span className={styles.rejectedTitle}>{strings.batch.rejectedList}</span>
                    <ul className={styles.rejectedList}>
                      {result.rejected.map((failure) => (
                        <li key={failure.sourceUrl}>
                          <span className={styles.rejectedUrl} title={failure.sourceUrl}>
                            {failure.sourceUrl}
                          </span>
                          <span className={styles.rejectedReason}>{failure.error.message}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            )}

            {result && result.accepted.length > 0 && (
              <div className={styles.footer}>
                <button type="button" className={styles.secondaryButton} onClick={showQueue}>
                  {strings.batch.viewQueue}
                </button>
              </div>
            )}
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
