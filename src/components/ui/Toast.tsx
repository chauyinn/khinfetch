import React from "react";
import { CheckCircle2, AlertTriangle, AlertCircle, Info, X } from "lucide-react";
import { useToastStore, type ToastType } from "../../stores/useToastStore";
import { useStrings } from "../../i18n/useStrings";
import styles from "./Toast.module.css";

export const ToastContainer: React.FC = () => {
  const strings = useStrings();
  const { toasts, removeToast } = useToastStore();

  if (toasts.length === 0) return null;

  const renderIcon = (type: ToastType) => {
    switch (type) {
      case "success":
        return <CheckCircle2 size={16} style={{ color: "var(--color-success)" }} />;
      case "warning":
        return <AlertTriangle size={16} style={{ color: "var(--color-warning)" }} />;
      case "error":
        return <AlertCircle size={16} style={{ color: "var(--color-danger)" }} />;
      default:
        return <Info size={16} style={{ color: "var(--color-accent)" }} />;
    }
  };

  return (
    <div className={styles.toastContainer} role="region" aria-live="polite" aria-label={strings.a11y.notifications}>
      {toasts.map((toast) => (
        <div key={toast.id} className={`${styles.toast} ${styles[toast.type]}`}>
          {renderIcon(toast.type)}
          <span>{toast.message}</span>
          <button
            className={styles.closeBtn}
            onClick={() => removeToast(toast.id)}
            aria-label={strings.a11y.closeNotification}
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
};
