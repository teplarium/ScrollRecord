/**
 * PropertiesPanel — the right panel's body (HANDOFF D-38, FR-CNV-10).
 *
 * Step 3.2 only names the selected shape; its settings (locators, hold time,
 * actions, arrows, anchors) become editable in step 3.5.
 */
import { useTranslation } from "react-i18next";

import type { FlowNode } from "../../shared/ipc/generated/FlowNode";

type Props = {
  /** The selected shape, `null` when nothing is selected. */
  node: FlowNode | null;
};

export function PropertiesPanel({ node }: Props) {
  const { t } = useTranslation();

  if (!node) {
    return <p className="properties properties--empty">{t("inspector.nothingSelected")}</p>;
  }

  const kind = {
    start: t("inspector.kindStart"),
    end: t("inspector.kindEnd"),
    block: t("inspector.kindBlock"),
    navigate: t("inspector.kindNavigate"),
  }[node.kind];

  return (
    <div className="properties">
      <span className="properties__kind">{kind}</span>
      <h2 className="properties__name">{node.id}</h2>
      <p className="properties__later">{t("inspector.later")}</p>
    </div>
  );
}
