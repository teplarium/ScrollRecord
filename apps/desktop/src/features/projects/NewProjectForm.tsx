/**
 * NewProjectForm — name, location, website, and how to start (HANDOFF D-37,
 * D-39).
 *
 * Rules from the owner's answers:
 * - empty name → Rust picks "Untitled N" (the translated word is sent as
 *   `untitledBase`, Rust finds the free number in the chosen folder);
 * - location is pre-filled: `Documents/ScrollRecord` the first time, then
 *   the last one used (Rust remembers it);
 * - the site is a folder (picked through its `index.html`) or an `http(s)://`
 *   address (typed);
 * - start from an empty canvas or from a scenario file (import).
 * A website is required, except when importing a scenario that names its
 * own site.
 */
import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../../shared/ipc/commands";
import { pickFolder, pickScenarioFile, pickSiteFolder } from "../../shared/ipc/dialogs";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { NewProject } from "../../shared/ipc/generated/NewProject";
import type { OpenedProject } from "../../shared/ipc/generated/OpenedProject";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";

type Props = {
  suggestedLocation: string;
  onCreated: (project: OpenedProject) => void;
  onCancel: () => void;
};

type SiteKind = "folder" | "url";
type StartKind = "empty" | "import";

const URL_PATTERN = /^https?:\/\/\S+$/i;

export function NewProjectForm({ suggestedLocation, onCreated, onCancel }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [location, setLocation] = useState(suggestedLocation);
  const [siteKind, setSiteKind] = useState<SiteKind>("folder");
  const [siteFolder, setSiteFolder] = useState("");
  const [siteUrl, setSiteUrl] = useState("");
  const [startKind, setStartKind] = useState<StartKind>("empty");
  const [importFile, setImportFile] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  // ── What the form would send, and whether it may ──
  const site = siteKind === "folder" ? siteFolder : siteUrl.trim();
  const urlInvalid = siteKind === "url" && site !== "" && !URL_PATTERN.test(site);
  const importing = startKind === "import";
  const ready =
    location.trim() !== "" &&
    !urlInvalid &&
    (site !== "" || importing) &&
    (!importing || importFile !== "");

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!ready || busy) return;
    setBusy(true);
    setError(null);
    // Optional fields are left out rather than sent as `undefined`.
    const request: NewProject = {
      location: location.trim(),
      untitledBase: t("newProject.untitled"),
    };
    if (name.trim() !== "") request.name = name.trim();
    if (site !== "") request.site = site;
    if (importing) request.importFrom = importFile;
    const result = await commands.createProject(request);
    setBusy(false);
    if (result.ok) onCreated(result.value);
    else setError(result.error);
  }

  async function chooseLocation() {
    const dir = await pickFolder(t("newProject.pickLocation"), location || undefined);
    if (dir) setLocation(dir);
  }

  async function chooseSite() {
    const dir = await pickSiteFolder(
      t("newProject.pickSite"),
      { files: t("dialogs.webPages"), all: t("dialogs.allFiles") },
      siteFolder || undefined,
    );
    if (dir) setSiteFolder(dir);
  }

  async function chooseImport() {
    const file = await pickScenarioFile(t("newProject.pickScenario"), {
      files: t("dialogs.scenarios"),
      all: t("dialogs.allFiles"),
    });
    if (file) setImportFile(file);
  }

  return (
    <form className="form" onSubmit={(e) => void submit(e)}>
      <h2 className="form__title">{t("newProject.title")}</h2>

      {/* ── Name ── */}
      <label className="form__row">
        <span className="form__label">{t("newProject.name")}</span>
        <input
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("newProject.namePlaceholder")}
          autoFocus
        />
        <span className="form__hint">{t("newProject.nameHint")}</span>
      </label>

      {/* ── Location ── */}
      <div className="form__row">
        <span className="form__label">{t("newProject.location")}</span>
        <div className="form__pick">
          <input
            className="input"
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            aria-label={t("newProject.location")}
          />
          <button type="button" className="button" onClick={() => void chooseLocation()}>
            {t("newProject.choose")}
          </button>
        </div>
      </div>

      {/* ── Website: folder or address ── */}
      <div className="form__row">
        <span className="form__label">{t("newProject.site")}</span>
        <div className="segmented" role="radiogroup" aria-label={t("newProject.site")}>
          <button
            type="button"
            role="radio"
            aria-checked={siteKind === "folder"}
            className="segmented__item"
            onClick={() => setSiteKind("folder")}
          >
            {t("newProject.siteFolder")}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={siteKind === "url"}
            className="segmented__item"
            onClick={() => setSiteKind("url")}
          >
            {t("newProject.siteUrl")}
          </button>
        </div>
        {siteKind === "folder" ? (
          <div className="form__pick">
            <input
              className="input"
              value={siteFolder}
              readOnly
              placeholder={t("newProject.siteFolderPlaceholder")}
              aria-label={t("newProject.siteFolder")}
            />
            <button type="button" className="button" onClick={() => void chooseSite()}>
              {t("newProject.choose")}
            </button>
          </div>
        ) : (
          <input
            className="input"
            value={siteUrl}
            onChange={(e) => setSiteUrl(e.target.value)}
            placeholder={t("newProject.siteUrlPlaceholder")}
            aria-label={t("newProject.siteUrl")}
            aria-invalid={urlInvalid}
          />
        )}
        <span className={`form__hint${urlInvalid ? " form__hint--error" : ""}`}>
          {siteKind === "folder" ? t("newProject.siteFolderHint") : t("newProject.siteUrlHint")}
        </span>
      </div>

      {/* ── Start with ── */}
      <div className="form__row">
        <span className="form__label">{t("newProject.startWith")}</span>
        <div className="segmented" role="radiogroup" aria-label={t("newProject.startWith")}>
          <button
            type="button"
            role="radio"
            aria-checked={startKind === "empty"}
            className="segmented__item"
            onClick={() => setStartKind("empty")}
          >
            {t("newProject.startEmpty")}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={startKind === "import"}
            className="segmented__item"
            onClick={() => setStartKind("import")}
          >
            {t("newProject.startImport")}
          </button>
        </div>
        {importing && (
          <>
            <div className="form__pick">
              <input
                className="input"
                value={importFile}
                readOnly
                placeholder={t("newProject.importPlaceholder")}
                aria-label={t("newProject.startImport")}
              />
              <button type="button" className="button" onClick={() => void chooseImport()}>
                {t("newProject.choose")}
              </button>
            </div>
            <span className="form__hint">{t("newProject.importHint")}</span>
          </>
        )}
      </div>

      {error && <ErrorBanner error={error} onDismiss={() => setError(null)} />}

      <div className="form__actions">
        <button type="button" className="button" onClick={onCancel} disabled={busy}>
          {t("newProject.cancel")}
        </button>
        <button type="submit" className="button button--primary" disabled={!ready || busy}>
          {busy ? t("newProject.creating") : t("newProject.create")}
        </button>
      </div>
    </form>
  );
}
