// Site map of the open page: blocks and the interactive elements inside them.
//
// Purpose: lets the editor list what can go onto the canvas (HANDOFF D-38,
//          D-39) without the user typing selectors.
// Solves:  a suggested locator must find *exactly* that element when the video
//          is recorded, so every check goes through the recording's own
//          locator engine (`__sr.locate`, runtime.js) and `__sr.measure`.
// Rules:   read-only — never scrolls, clicks or changes the page;
//          blocks = `[data-sr]` elements + the blocks the scenario names
//          (no heuristic auto-detection: Phase 4, D-39);
//          an element belongs to the innermost block that contains it.
// Input:   `__INPUT__` is replaced by Rust with
//          `{ "scenario": [{ "id": "hero", "locators": [...] }] }`.
// Output:  see `RawPage` in crates/sr-analyzer/src/lib.rs.
(() => {
  const input = __INPUT__;
  const { findAll, accessibleName, visible } = window.__sr.locate;

  // ── Helpers ──
  const shown = (el) => {
    if (!visible(el)) return false;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return false;
    const cs = getComputedStyle(el);
    return cs.visibility !== "hidden" && cs.display !== "none";
  };
  const rect = (el) => {
    const r = el.getBoundingClientRect();
    return { x: r.left + scrollX, y: r.top + scrollY, width: r.width, height: r.height };
  };
  const clean = (s) => (s || "").replace(/\s+/g, " ").trim();
  const short = (s) => {
    const text = clean(s);
    return text.length > 80 ? `${text.slice(0, 79)}…` : text;
  };

  // ── Blocks: data-sr first, then the scenario's own blocks ──
  // Several sources may point at one element; they merge into one block.
  const blocks = [];
  const byElement = new Map();
  for (const el of document.querySelectorAll("[data-sr]")) {
    if (!shown(el)) continue;
    const name = el.getAttribute("data-sr");
    const block = { name, marked: true, node: null, locators: [{ sr: name }], status: "found", el, tried: [] };
    blocks.push(block);
    byElement.set(el, block);
  }

  const measured = window.__sr.measure(input.scenario).blocks;
  for (const item of input.scenario) {
    const m = measured[item.id];
    if (!m || m.status !== "found") {
      blocks.push({
        name: item.id,
        marked: false,
        node: item.id,
        locators: item.locators,
        status: m ? m.status : "not_found",
        el: null,
        tried: m ? m.tried || [] : [],
      });
      continue;
    }
    // `measure` found exactly one element with locator #matched: find it again.
    const loc = item.locators[m.matched];
    const el = findAll(loc)[loc.nth ?? 0];
    const same = el && byElement.get(el);
    if (same) {
      same.node = item.id;
      continue;
    }
    const block = { name: item.id, marked: false, node: item.id, locators: item.locators, status: "found", el, tried: [] };
    blocks.push(block);
    if (el) byElement.set(el, block);
  }

  // ── Interactive elements, by kind ──
  // An open list (HANDOFF D-38): plain text or other kinds can be added later.
  const INTERACTIVE = [
    "a[href]", "button", "input:not([type=hidden])", "select", "textarea", "summary",
    "[role=button]", "[role=link]", "[role=checkbox]", "[role=radio]", "[role=switch]",
    "[role=tab]", "[role=menuitem]", "[role=option]", "[role=slider]", "[role=combobox]",
    "[draggable=true]", "[contenteditable=''], [contenteditable=true]",
  ].join(", ");
  function kindOf(el) {
    const role = el.getAttribute("role");
    const tag = el.tagName.toLowerCase();
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (el.getAttribute("draggable") === "true") return "draggable";
    if (role === "checkbox" || role === "switch" || type === "checkbox") return "checkbox";
    if (role === "radio" || type === "radio") return "radio";
    if (role === "slider" || type === "range") return "slider";
    if (role === "tab") return "tab";
    if (tag === "select" || role === "combobox" || role === "option") return "select";
    if (tag === "textarea" || el.isContentEditable) return "textarea";
    if (tag === "button" || role === "button" || role === "menuitem" || tag === "summary") return "button";
    if (["button", "submit", "reset", "image"].includes(type)) return "button";
    if (tag === "input") return "input";
    if (tag === "a" || role === "link") return "link";
    return "other";
  }
  const ROLE_OF_KIND = { button: "button", link: "link", input: "textbox", textarea: "textbox" };

  function labelOf(el) {
    const name = clean(accessibleName(el));
    if (name) return short(name);
    return short(el.getAttribute("placeholder") || el.value || el.getAttribute("name") || el.tagName.toLowerCase());
  }

  // ── Locator candidates (robust first, like locator.rs) ──
  // Kept only if the recording's engine finds exactly this element inside its
  // block (actions are searched inside their block first, `within`).
  function unique(loc, el, scope) {
    let els;
    try {
      els = findAll(loc);
    } catch {
      return false;
    }
    const inside = els.filter((e) => scope.contains(e) && e !== scope);
    if (inside.length) els = inside;
    return els.length === 1 && els[0] === el;
  }
  function cssPath(el, scope) {
    const parts = [];
    for (let node = el; node && node !== scope; node = node.parentElement) {
      const tag = node.tagName.toLowerCase();
      const same = [...(node.parentElement?.children || [])].filter((c) => c.tagName === node.tagName);
      parts.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
    }
    return parts.join(" > ");
  }
  function candidates(el, scope, kind) {
    const out = [];
    const tryAdd = (loc) => {
      if (unique(loc, el, scope)) out.push(loc);
    };
    const sr = el.getAttribute("data-sr");
    if (sr) tryAdd({ sr });
    const testid = el.getAttribute("data-testid");
    if (testid) tryAdd({ testid });
    const role = el.getAttribute("role") || ROLE_OF_KIND[kind];
    const name = clean(accessibleName(el));
    if (role && name && name.length <= 80) tryAdd({ role, name });
    const text = clean(el.textContent);
    if (text && text.length <= 80) tryAdd({ text });
    if (el.id) tryAdd({ css: `#${CSS.escape(el.id)}` });
    const path = cssPath(el, scope);
    if (path) tryAdd({ css: path });
    return out;
  }

  // ── Assign elements to the innermost block ──
  const found = blocks.filter((b) => b.el);
  const innermost = (el) => {
    let best = null;
    for (const b of found) {
      if (b.el !== el && b.el.contains(el) && (!best || best.el.contains(b.el))) best = b;
    }
    return best;
  };
  const elements = new Map(found.map((b) => [b, []]));
  for (const el of document.querySelectorAll(INTERACTIVE)) {
    if (!shown(el)) continue;
    const block = innermost(el);
    if (!block) continue;
    const kind = kindOf(el);
    elements.get(block).push({ kind, label: labelOf(el), rect: rect(el), locators: candidates(el, block.el, kind) });
  }

  // ── Result, blocks in page order (missing ones last) ──
  const out = blocks.map((b) => ({
    name: b.name,
    marked: b.marked,
    node: b.node,
    locators: b.locators,
    status: b.status,
    rect: b.el ? rect(b.el) : null,
    elements: b.el ? elements.get(b) : [],
    tried: b.tried,
  }));
  out.sort((a, b) => (a.rect ? a.rect.y : Infinity) - (b.rect ? b.rect.y : Infinity));
  return {
    scrollHeight: (document.scrollingElement || document.documentElement).scrollHeight,
    blocks: out,
  };
})()
