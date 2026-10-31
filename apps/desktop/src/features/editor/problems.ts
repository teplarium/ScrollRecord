/**
 * Scenario problems for the canvas (FR-CNV-09), in the user's language.
 *
 * Two sources: Rust's checks of the scenario (`validate_flow` → `Issue`, run
 * ~300 ms after the last edit) and the site map, which knows which of the
 * scenario's blocks the site doesn't have on the shown device ("locators
 * are found" in FR-CNV-09). Errors come first.
 *
 * "The path isn't finished yet" is not shown on the canvas (owner's decision,
 * 2026-10-31): while a scenario is being built it is always unfinished, and a
 * red End or Start that has nothing wrong with it only distracts. Those kinds
 * (`UNTIL_RENDER`) are shown when the user starts a render (step 3.7).
 */
import type { TFunction } from "i18next";
import { useEffect, useState } from "react";

import { commands } from "../../shared/ipc/commands";
import type { Flow } from "../../shared/ipc/generated/Flow";
import type { Issue } from "../../shared/ipc/generated/Issue";
import type { IssueKind } from "../../shared/ipc/generated/IssueKind";
import type { SiteMap } from "../../shared/ipc/generated/SiteMap";
import type { ShapeProblem } from "../canvas/toReactFlow";

/** Wait after the last edit before checking the scenario again. */
export const VALIDATE_DELAY_MS = 300;

/**
 * Kinds about how the shapes are connected into one path: normal while the
 * scenario is being built, so they wait until a render is started. Shapes
 * themselves being broken (unknown page, bad locator, a block the site
 * doesn't have) is still shown at once.
 */
export const UNTIL_RENDER: ReadonlySet<IssueKind> = new Set<IssueKind>([
  "startCount",
  "endCount",
  "anchorMissing",
  "arrowToItself",
  "branch",
  "manyIncoming",
  "startHasIncoming",
  "loop",
  "pathNotToEnd",
  "notOnPath",
]);

/** Rust's issues of `flow`, refreshed shortly after it stops changing. */
export function useIssues(flow: Flow): Issue[] {
  const [issues, setIssues] = useState<Issue[]>([]);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      void commands.validateFlow(flow).then((result) => {
        // Outside the desktop app there is no checker: show nothing.
        if (active) setIssues(result.ok ? result.value : []);
      });
    }, VALIDATE_DELAY_MS);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [flow]);
  return issues;
}

/**
 * Both sources as canvas problems, without the `UNTIL_RENDER` kinds. `map` is
 * the site map of the shown device (`null` or stale = not known yet: no
 * "not found" marks then).
 */
export function toProblems(
  issues: Issue[],
  map: SiteMap | null,
  flow: Flow,
  t: TFunction,
): ShapeProblem[] {
  const shown = issues.filter((issue) => !UNTIL_RENDER.has(issue.kind));
  const fromRust = shown.map((issue): ShapeProblem => ({
    severity: issue.severity,
    // The English text is the fallback if a text is ever missing.
    text: t(`issues.${issue.kind}`, { ...issue.params, defaultValue: issue.message }),
    node: issue.node ?? null,
    edge: issue.edge ?? null,
  }));

  const onCanvas = new Set(flow.nodes.map((n) => n.id));
  const notFound = (map?.pages ?? []).flatMap((page) =>
    page.blocks.flatMap((block): ShapeProblem[] =>
      block.status !== "found" && block.node !== null && onCanvas.has(block.node)
        ? [{ severity: "error", text: t("problems.notFound"), node: block.node, edge: null }]
        : [],
    ),
  );

  const all = [...fromRust, ...notFound];
  return [
    ...all.filter((p) => p.severity === "error"),
    ...all.filter((p) => p.severity !== "error"),
  ];
}
