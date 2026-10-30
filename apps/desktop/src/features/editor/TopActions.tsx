/**
 * TopActions — the compact row at the top of the right panel (HANDOFF D-38,
 * FR-CNV-10): device switcher, preview, render, render queue.
 *
 * The device switcher already works: it picks which video format the canvas
 * shows, and block cards take that device's shape (D-39). Preview (step 3.8),
 * render and the queue (step 3.7) are visible but disabled until then.
 */
import { useTranslation } from "react-i18next";

import type { Target } from "../../shared/ipc/generated/Target";
import { PlayIcon, QueueIcon } from "../../shared/ui/icons";

type Props = {
  targets: Target[];
  /** Id of the shown target, `null` when the project has none. */
  current: string | null;
  onDevice: (targetId: string) => void;
};

export function TopActions({ targets, current, onDevice }: Props) {
  const { t } = useTranslation();

  return (
    <div className="top-actions">
      <select
        className="input top-actions__device"
        aria-label={t("topActions.device")}
        value={current ?? ""}
        disabled={targets.length === 0}
        onChange={(event) => onDevice(event.target.value)}
      >
        {targets.length === 0 && <option value="">{t("topActions.noTargets")}</option>}
        {targets.map((target) => (
          <option key={target.id} value={target.id}>
            {t("topActions.target", { name: target.id, aspect: target.aspect })}
          </option>
        ))}
      </select>
      {/* Icon buttons keep the row compact so the device name fits. */}
      <button
        type="button"
        className="button top-actions__icon"
        disabled
        title={t("topActions.laterFor", { action: t("topActions.preview") })}
        aria-label={t("topActions.preview")}
      >
        <PlayIcon />
      </button>
      <button
        type="button"
        className="button button--primary"
        disabled
        title={t("topActions.later")}
      >
        {t("topActions.render")}
      </button>
      <button
        type="button"
        className="button top-actions__icon"
        disabled
        title={t("topActions.laterFor", { action: t("topActions.queue") })}
        aria-label={t("topActions.queue")}
      >
        <QueueIcon />
      </button>
    </div>
  );
}
