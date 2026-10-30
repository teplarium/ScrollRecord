/**
 * Small icons of the site map rows (SVG, no text to translate).
 * One shape per kind so a long list can be scanned at a glance, like the
 * layer icons in Figma.
 */
import type { ElementKind } from "../../shared/ipc/generated/ElementKind";

const PATHS: Record<ElementKind | "block", string> = {
  block: "M4 5h16v14H4z",
  button: "M5 8h14a3 3 0 0 1 0 8H5a3 3 0 0 1 0-8Z",
  link: "M10 14a4 4 0 0 0 5.6 0l3-3a4 4 0 0 0-5.6-5.6l-1 1M14 10a4 4 0 0 0-5.6 0l-3 3a4 4 0 0 0 5.6 5.6l1-1",
  input: "M4 7h16v10H4zM8 10v4",
  textarea: "M4 5h16v14H4zM8 9h8M8 13h5",
  checkbox: "M5 5h14v14H5zM8.5 12l2.5 2.5 4.5-5",
  radio: "M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16ZM12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z",
  select: "M4 7h16v10H4zM14 11l2 2 2-2",
  tab: "M3 19V9h7V6h6v3h5v10z",
  slider: "M4 12h16M10 9v6",
  draggable: "M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01",
  other: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z",
};

export function KindIcon({ kind }: { kind: ElementKind | "block" }) {
  return (
    <svg
      className="site-map__icon"
      width={14}
      height={14}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path
        d={PATHS[kind]}
        stroke="currentColor"
        strokeWidth={kind === "draggable" ? 3.5 : 1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      className={`site-map__chevron${open ? " site-map__chevron--open" : ""}`}
      width={12}
      height={12}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path d="M9 6l6 6-6 6" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
    </svg>
  );
}

export function RescanIcon() {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
