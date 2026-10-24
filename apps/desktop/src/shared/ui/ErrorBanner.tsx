/**
 * ErrorBanner — shows one `AppError`: translated text + its SR-xxxx code.
 * Why the code is always visible: users can search it in docs/ERRORS.md or
 * paste it into an issue, which makes problems reportable without
 * screenshots of logs (HANDOFF §12).
 */
import { useTranslation } from "react-i18next";

import { formatError } from "../../i18n/errors";
import type { AppError } from "../ipc/generated/AppError";
import "./ErrorBanner.css";

type Props = {
  error: AppError;
  onDismiss: () => void;
};

export function ErrorBanner({ error, onDismiss }: Props) {
  const { t } = useTranslation();

  return (
    <div className="error-banner" role="alert">
      <span className="error-banner__code">{t("common.errorCode", { code: error.code })}</span>
      <span className="error-banner__text">{formatError(t, error)}</span>
      <button type="button" className="error-banner__dismiss" onClick={onDismiss}>
        {t("common.dismiss")}
      </button>
    </div>
  );
}
