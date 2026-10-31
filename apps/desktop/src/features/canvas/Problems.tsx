/**
 * Problems — the "N problems" button in the canvas corner and its list
 * (FR-CNV-09).
 *
 * Why a list besides the outlines: some problems belong to no shape ("the
 * scenario needs an End"), and a red outline far off screen is easy to miss.
 * Clicking a row selects its shape and brings it into view. Nothing is shown
 * when the scenario has no problems.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { WarningIcon } from "../../shared/ui/icons";
import type { ShapeProblem } from "./toReactFlow";

type Props = {
  problems: ShapeProblem[];
  /** A row with a shape was clicked. */
  onShow: (nodeId: string) => void;
};

export function Problems({ problems, onShow }: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  if (problems.length === 0) return null;

  const errors = problems.filter((p) => p.severity === "error").length;
  const warnings = problems.length - errors;
  const label = [
    errors > 0 ? t("problems.count", { count: errors }) : null,
    warnings > 0 ? t("problems.warnings", { count: warnings }) : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="canvas-problems">
      <button
        type="button"
        className={`canvas-problems__toggle canvas-problems__toggle--${errors > 0 ? "error" : "warning"}`}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <WarningIcon size={14} />
        {label}
      </button>
      {open && (
        <ul className="canvas-problems__list" aria-label={t("problems.title")}>
          {problems.map((p, i) => {
            const node = p.node;
            return (
              <li key={i}>
                <button
                  type="button"
                  className="canvas-problems__row"
                  disabled={node === null}
                  onClick={() => node !== null && onShow(node)}
                >
                  <span className={`canvas-problems__dot canvas-problems__dot--${p.severity}`} />
                  <span>
                    {node !== null && <strong className="canvas-problems__node">{node}</strong>}
                    {p.text}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
