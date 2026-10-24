// ScrollRecord page runtime — injected into every page *before* the page's
// own scripts run (Page.addScriptToEvaluateOnNewDocument).
//
// Purpose: gives ScrollRecord full control over time and scrolling inside the
//          page, plus helpers to prepare the page and measure blocks.
// Solves:  screen recording in real time stutters, because the browser can't
//          always draw 60 frames per second. Here the page's clock is frozen
//          and moved forward by exactly 1/fps per video frame, so every frame
//          is rendered completely, no matter how slow the machine is
//          (HANDOFF §7.3). Same scenario → same video.
// What is virtualised: setTimeout/setInterval, requestAnimationFrame,
//          performance.now, Date, and all CSS/Web animations (paused and
//          seeked via the Web Animations API).
// Rules:   this script must never break a page. Every page callback runs in
//          try/catch; helpers use the *real* timers saved below.
(() => {
  if (window.__sr) return;

  // ── Real browser functions, saved before we replace them ─────────────────
  const realSetTimeout = window.setTimeout.bind(window);
  const realRequestFrame = window.requestAnimationFrame.bind(window);
  const RealDate = window.Date;
  const epoch = RealDate.now();

  // ── Virtual clock ─────────────────────────────────────────────────────────
  // `now` = milliseconds of page time since the page started. It only moves
  // when ScrollRecord calls advance(), so the page is frozen while loading and
  // between frames.
  let now = 0;
  const timers = new Map();
  let nextTimerId = 1;
  const frameCallbacks = new Map();
  let nextFrameId = 1;

  window.setTimeout = function (fn, delay, ...args) {
    const id = nextTimerId++;
    timers.set(id, { fn, args, at: now + Math.max(0, Number(delay) || 0), every: 0 });
    return id;
  };
  window.setInterval = function (fn, delay, ...args) {
    const every = Math.max(1, Number(delay) || 0);
    const id = nextTimerId++;
    timers.set(id, { fn, args, at: now + every, every });
    return id;
  };
  window.clearTimeout = window.clearInterval = function (id) {
    timers.delete(id);
  };
  window.requestAnimationFrame = function (cb) {
    const id = nextFrameId++;
    frameCallbacks.set(id, cb);
    return id;
  };
  window.cancelAnimationFrame = function (id) {
    frameCallbacks.delete(id);
  };
  performance.now = function () {
    return now;
  };

  // `Date` keeps working as before (parsing, UTC, instanceof), only "now"
  // is virtual: `new Date()` and `Date.now()` follow the frozen clock.
  function VirtualDate(...args) {
    if (!new.target) return new RealDate(epoch + now).toString();
    return args.length ? new RealDate(...args) : new RealDate(epoch + now);
  }
  VirtualDate.prototype = RealDate.prototype;
  VirtualDate.now = () => epoch + now;
  VirtualDate.parse = RealDate.parse;
  VirtualDate.UTC = RealDate.UTC;
  window.Date = VirtualDate;

  function safely(fn, ...args) {
    try {
      if (typeof fn === "function") fn(...args);
    } catch (err) {
      console.error("[page error during ScrollRecord frame]", err);
    }
  }

  // Runs every timer due up to `target`, in time order, then rAF callbacks.
  function advance(ms) {
    const target = now + Math.max(0, ms);
    for (let guard = 0; guard < 100000; guard++) {
      let dueId = 0;
      let due = null;
      for (const [id, t] of timers) {
        if (t.at <= target && (!due || t.at < due.at)) {
          dueId = id;
          due = t;
        }
      }
      if (!due) break;
      now = Math.max(now, due.at);
      if (due.every) due.at += due.every;
      else timers.delete(dueId);
      safely(due.fn, ...due.args);
    }
    now = target;
    const callbacks = [...frameCallbacks.values()];
    frameCallbacks.clear();
    for (const cb of callbacks) safely(cb, now);
  }

  // ── Intersection observers (scroll-reveal effects) ────────────────────────
  // The browser reports "element entered the viewport" during its own,
  // real-time rendering. To make that deterministic, frame() waits for the
  // browser to deliver those reports before the frame is captured. Pages
  // without observers skip the wait and render faster.
  let observerCount = 0;
  if (typeof window.IntersectionObserver === "function") {
    const RealObserver = window.IntersectionObserver;
    window.IntersectionObserver = class IntersectionObserver extends RealObserver {
      constructor(callback, options) {
        super(callback, options);
        observerCount++;
      }
    };
  }
  // Two real rendering steps: in the first the browser computes
  // intersections and queues the callbacks; they run before the second.
  const settleObservers = () =>
    new Promise((resolve) => realRequestFrame(() => realRequestFrame(resolve)));

  // ── CSS & Web animations ──────────────────────────────────────────────────
  // Every animation is paused the first time we see it and then positioned
  // explicitly: currentTime = page time since it appeared. Animations the
  // site itself paused (animation-play-state: paused) are left alone.
  const firstSeen = new WeakMap();
  const sitePaused = new WeakSet();
  function syncAnimations() {
    for (const anim of document.getAnimations()) {
      if (sitePaused.has(anim)) continue;
      if (!firstSeen.has(anim)) {
        if (anim.playState === "paused") {
          sitePaused.add(anim);
          continue;
        }
        firstSeen.set(anim, now);
        try {
          anim.pause();
        } catch {
          continue;
        }
      }
      try {
        anim.currentTime = (now - firstSeen.get(anim)) * (anim.playbackRate || 1);
      } catch {
        /* finished or removed meanwhile */
      }
    }
  }

  // ── Locators (see crates/sr-core/src/flow/locator.rs) ─────────────────────
  const norm = (s) => (s || "").replace(/\s+/g, " ").trim().toLowerCase();
  const visible = (el) => el.getClientRects().length > 0;
  const IMPLICIT_ROLES = {
    button: 'button, input[type="button"], input[type="submit"], input[type="reset"]',
    link: "a[href]",
    heading: "h1, h2, h3, h4, h5, h6",
    navigation: "nav",
    main: "main",
    banner: "body > header",
    contentinfo: "body > footer",
    img: "img[alt]",
    list: "ul, ol",
    listitem: "li",
    textbox: 'input:not([type]), input[type="text"], input[type="email"], textarea',
    region: "section[aria-label], section[aria-labelledby]",
  };
  function accessibleName(el) {
    const label = el.getAttribute("aria-label");
    if (label) return label;
    const by = el.getAttribute("aria-labelledby");
    if (by) {
      return by
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.textContent || "")
        .join(" ");
    }
    return el.getAttribute("alt") || el.getAttribute("title") || el.textContent || "";
  }
  function preferVisible(list) {
    const shown = list.filter(visible);
    return shown.length ? shown : list;
  }
  function byText(text, candidates) {
    const want = norm(text);
    const exact = candidates.filter((el) => norm(el.textContent) === want);
    // Deepest elements only: a <section> also "contains" its heading's text.
    const deepest = exact.filter((el) => !exact.some((o) => o !== el && el.contains(o)));
    if (deepest.length) return deepest;
    const partial = candidates.filter((el) => norm(el.textContent).includes(want));
    return partial.filter((el) => !partial.some((o) => o !== el && el.contains(o)));
  }
  function findAll(loc) {
    const attr = (name, value) => [...document.querySelectorAll(`[${name}="${CSS.escape(value)}"]`)];
    if (loc.sr != null) return attr("data-sr", loc.sr);
    if (loc.testid != null) return attr("data-testid", loc.testid);
    if (loc.css != null) return [...document.querySelectorAll(loc.css)];
    if (loc.xpath != null) {
      const snap = document.evaluate(loc.xpath, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
      const out = [];
      for (let i = 0; i < snap.snapshotLength; i++) out.push(snap.snapshotItem(i));
      return out.filter((n) => n instanceof Element);
    }
    if (loc.text != null) return preferVisible(byText(loc.text, [...document.body.querySelectorAll("*")]));
    if (loc.role != null) {
      const sel = [`[role="${CSS.escape(loc.role)}"]`, IMPLICIT_ROLES[loc.role]].filter(Boolean).join(", ");
      let els = [...document.querySelectorAll(sel)];
      if (loc.name != null) {
        const want = norm(loc.name);
        const exact = els.filter((el) => norm(accessibleName(el)) === want);
        els = exact.length ? exact : els.filter((el) => norm(accessibleName(el)).includes(want));
      }
      return preferVisible(els);
    }
    throw new Error("locator has no strategy");
  }
  function describe(loc) {
    for (const k of ["sr", "testid", "css", "role", "text", "xpath"]) {
      if (loc[k] != null) return `${k}=${loc[k]}${loc.name != null ? `[${loc.name}]` : ""}${loc.nth != null ? ` #${loc.nth}` : ""}`;
    }
    return "<empty>";
  }

  // Height of sticky/fixed bars glued to the top (headers). "align: top"
  // lands blocks just below them instead of hidden underneath.
  function topInset() {
    let inset = 0;
    for (const el of document.body.querySelectorAll("*")) {
      const cs = getComputedStyle(el);
      if (cs.position !== "fixed" && cs.position !== "sticky") continue;
      if (cs.display === "none" || cs.visibility === "hidden") continue;
      const top = parseFloat(cs.top);
      if (!(top >= 0 && top <= 2)) continue;
      const r = el.getBoundingClientRect();
      if (r.width < innerWidth * 0.5 || r.height <= 0 || r.height > innerHeight * 0.3) continue;
      inset = Math.max(inset, top + r.height);
    }
    return inset;
  }

  const realWait = (ms) => new Promise((resolve) => realSetTimeout(resolve, ms));
  const withTimeout = (promise, ms) => Promise.race([promise, realWait(ms)]);

  window.__sr = Object.freeze({
    /** Current page time in ms. */
    now: () => now,

    /** Moves page time forward without drawing (used for warm-up). */
    advance(ms) {
      advance(ms);
      syncAnimations();
      return now;
    },

    /**
     * One video frame: scroll, move time forward, position animations.
     * Resolves to the scroll position the browser actually applied.
     */
    async frame(scrollY, dtMs) {
      window.scrollTo(0, scrollY);
      // Deliver the scroll event now, so scroll-linked code reacts in this frame.
      document.dispatchEvent(new Event("scroll", { bubbles: true }));
      advance(dtMs);
      syncAnimations();
      if (observerCount > 0) {
        await settleObservers();
        // Transitions started by observer callbacks begin at this frame's time.
        syncAnimations();
      }
      return (document.scrollingElement || document.documentElement).scrollTop;
    },

    /** Page preparation before recording (HANDOFF FR-VP-04). */
    async prepare(opts) {
      const invalid = [];
      const rules = ["html, body { scroll-behavior: auto !important; }"];
      for (const sel of opts.hide || []) {
        try {
          document.querySelector(sel);
          rules.push(`${sel} { display: none !important; }`);
        } catch {
          invalid.push(sel);
        }
      }
      if (opts.css) rules.push(opts.css);
      const style = document.createElement("style");
      style.id = "__scrollrecord";
      style.textContent = rules.join("\n");
      document.head.appendChild(style);

      if (opts.preloadLazy) {
        for (const el of document.querySelectorAll('img[loading="lazy"], iframe[loading="lazy"]')) {
          el.loading = "eager";
        }
      }
      await withTimeout(document.fonts.ready, 10000);
      const pending = [...document.images]
        .filter((img) => !img.complete)
        .map((img) => new Promise((resolve) => {
          img.addEventListener("load", resolve, { once: true });
          img.addEventListener("error", resolve, { once: true });
        }));
      await withTimeout(Promise.all(pending), 15000);

      if (opts.networkIdle) {
        let last = -1;
        let stableFor = 0;
        for (let waited = 0; waited < 10000 && stableFor < 500; waited += 100) {
          const count = performance.getEntriesByType("resource").length;
          stableFor = count === last ? stableFor + 100 : 0;
          last = count;
          await realWait(100);
        }
      }
      window.scrollTo(0, 0);
      syncAnimations();
      return { invalidSelectors: invalid };
    },

    /** Measures blocks in page coordinates (CSS px). */
    measure(blocks) {
      const result = {
        viewportWidth: innerWidth,
        viewportHeight: innerHeight,
        scrollHeight: (document.scrollingElement || document.documentElement).scrollHeight,
        topInset: topInset(),
        blocks: {},
      };
      for (const block of blocks) {
        const tried = [];
        let found = null;
        let ambiguous = false;
        block.locators.forEach((loc, index) => {
          if (found) return;
          let els;
          try {
            els = findAll(loc);
          } catch (err) {
            tried.push(`${describe(loc)}: invalid (${err.message})`);
            return;
          }
          if (loc.nth != null) els = els[loc.nth] ? [els[loc.nth]] : [];
          tried.push(`${describe(loc)}: ${els.length} match${els.length === 1 ? "" : "es"}`);
          if (els.length === 1) found = { el: els[0], index };
          else if (els.length > 1) ambiguous = true;
        });
        if (!found) {
          result.blocks[block.id] = { status: ambiguous ? "ambiguous" : "not_found", tried };
          continue;
        }
        const r = found.el.getBoundingClientRect();
        result.blocks[block.id] = {
          status: "found",
          top: r.top + scrollY,
          height: r.height,
          matched: found.index,
        };
      }
      return result;
    },
  });
})();
