/**
 * AppFooter — versions, platform and language picker.
 * Why: versions on screen make every screenshot of a problem a useful bug
 * report (HANDOFF §12). Also the first real IPC call: if the Rust core is not
 * reachable, the footer says so with a translated SR-xxxx message.
 */
import { useTranslation } from "react-i18next";

import { LanguageSelect } from "../features/settings/LanguageSelect";
import { formatError } from "../i18n/errors";
import { useAppInfo } from "../shared/ipc/useAppInfo";

export function AppFooter() {
  const { t } = useTranslation();
  const info = useAppInfo();

  return (
    <footer className="app__footer">
      <span className="app__meta">
        {info.status === "loading" && t("footer.loading")}
        {info.status === "ready" &&
          [
            t("footer.version", { version: info.value.appVersion }),
            t("footer.core", { version: info.value.coreVersion }),
            t("footer.platform", { os: info.value.os, arch: info.value.arch }),
          ].join("  ·  ")}
        {info.status === "error" && (
          <span className="app__meta-error" title={info.error.message}>
            {info.error.code}
            {" — "}
            {formatError(t, info.error)}
          </span>
        )}
      </span>
      <LanguageSelect />
    </footer>
  );
}
