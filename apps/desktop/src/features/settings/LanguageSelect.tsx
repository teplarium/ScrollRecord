/**
 * LanguageSelect — UI language picker (HANDOFF FR-I18N-02).
 * Lists every folder in `src/i18n/locales/`, so it grows automatically when a
 * translation is added. With only English bundled it still renders, to show
 * where the setting lives.
 */
import { useTranslation } from "react-i18next";

import { changeLanguage } from "../../i18n";
import { availableLanguages } from "../../i18n/locales";

export function LanguageSelect() {
  const { t, i18n } = useTranslation();
  const languages = availableLanguages();

  return (
    <label className="language-select">
      <span>{t("footer.language")}</span>{" "}
      <select
        value={i18n.resolvedLanguage ?? i18n.language}
        onChange={(event) => changeLanguage(event.target.value)}
      >
        {languages.map((lang) => (
          <option key={lang.code} value={lang.code}>
            {lang.name}
          </option>
        ))}
      </select>
    </label>
  );
}
