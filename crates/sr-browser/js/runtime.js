// ScrollRecord page runtime — injected into every page *before* the page's
// own scripts run (Page.addScriptToEvaluateOnNewDocument).
//
// Purpose: gives ScrollRecord full control over time and scrolling inside the
//          page, plus helpers to prepare the page, measure blocks and report
//          which cursor image (arrow / hand / I-beam) the page shows.
// Solves:  screen recording in real time stutters, because the browser can't
//          always draw 60 frames per second. Here the page's clock is frozen
//          and moved forward by exactly 1/fps per video frame, so every frame
//          is rendered completely, no matter how slow the machine is
//          (HANDOFF §7.3). Same scenario → same video.
// What is virtualised: setTimeout/setInterval, requestAnimationFrame,
//          requestIdleCallback, performance.now, Date, and all CSS/Web
//          animations (moved forward via the Web Animations API). Work that
//          frameworks schedule with MessageChannel (React) is waited for
//          inside the frame.
// Rules:   this script must never break a page. Every page callback runs in
//          try/catch; helpers use the *real* timers saved below.
(() => {
  if (window.__sr) return;

  // ── Real browser functions, saved before we replace them ─────────────────
  const realSetTimeout = window.setTimeout.bind(window);
  const realRequestFrame = window.requestAnimationFrame.bind(window);
  const RealDate = window.Date;
  const realFetch = window.fetch.bind(window); // sites sometimes wrap fetch
  const epoch = RealDate.now();
  const realWait = (ms) => new Promise((resolve) => realSetTimeout(resolve, ms));
  const withTimeout = (promise, ms) => Promise.race([promise, realWait(ms)]);

  // ── Virtual clock ─────────────────────────────────────────────────────────
  // `now` = milliseconds of page time since the page started. It only moves
  // when ScrollRecord calls advance(), so the page is frozen while loading and
  // between frames.
  let now = 0;
  const timers = new Map();
  let nextTimerId = 1;
  const frameCallbacks = new Map();
  let nextFrameId = 1;
  const idleCallbacks = new Map();
  let nextIdleId = 1;

  // Timers scheduled from inside timers more than 5 levels deep wait at
  // least 4 ms, like in every browser (HTML spec). Without this, a
  // `setTimeout(loop, 0)` loop would spin without end inside one frame.
  let nesting = 0;
  const clampDelay = (delay, level) => Math.max(level > 5 ? 4 : 0, Number(delay) || 0);

  window.setTimeout = function (fn, delay, ...args) {
    const id = nextTimerId++;
    const level = nesting + 1;
    timers.set(id, { fn, args, at: now + clampDelay(delay, level), every: 0, level });
    return id;
  };
  window.setInterval = function (fn, delay, ...args) {
    const every = Math.max(4, Number(delay) || 0);
    const id = nextTimerId++;
    timers.set(id, { fn, args, at: now + every, every, level: nesting + 1 });
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
  // Idle callbacks run once per frame, after the animation-frame callbacks.
  // Real browsers run them "whenever the machine is idle", i.e. at random
  // moments; deferred work (lazy widgets, Astro `client:idle`) would then
  // show up in a different frame on every render.
  if (typeof window.requestIdleCallback === "function") {
    window.requestIdleCallback = function (cb) {
      const id = nextIdleId++;
      idleCallbacks.set(id, cb);
      return id;
    };
    window.cancelIdleCallback = function (id) {
      idleCallbacks.delete(id);
    };
  }
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

  // Runs every timer due up to `target` in time order, then animation-frame
  // callbacks, then idle callbacks.
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
      nesting = due.level;
      safely(due.fn, ...due.args);
      nesting = 0;
    }
    now = target;
    const callbacks = [...frameCallbacks.values()];
    frameCallbacks.clear();
    for (const cb of callbacks) safely(cb, now);
    const idle = [...idleCallbacks.values()];
    idleCallbacks.clear();
    for (const cb of idle) {
      // Deterministic "time left": shrinks with every question, so a
      // `while (deadline.timeRemaining() > 0)` loop does a fixed amount of work.
      let asked = 0;
      safely(cb, { didTimeout: false, timeRemaining: () => Math.max(0, 50 - 5 * asked++) });
    }
  }

  // ── Framework tasks (MessageChannel) ──────────────────────────────────────
  // React and other schedulers do their rendering in tasks posted through a
  // MessageChannel, not in timers or animation frames. Those tasks run in
  // real time, so on a busy machine a re-render triggered in this frame
  // could land after the screenshot. ScrollRecord counts such posts and,
  // when there were any, waits inside the frame until the chain of tasks
  // has run (its own message is queued behind the page's ones). Bounded,
  // so a page that posts forever still gets its frame.
  let posted = 0;
  const realPortPost = MessagePort.prototype.postMessage;
  MessagePort.prototype.postMessage = function (...args) {
    posted++;
    return realPortPost.apply(this, args);
  };
  const drainChannel = new MessageChannel();
  let drained = null;
  drainChannel.port1.onmessage = () => drained && drained();
  const messageRoundTrip = () =>
    new Promise((resolve) => {
      drained = resolve;
      realPortPost.call(drainChannel.port2, null);
    });
  async function drainMessages() {
    for (let round = 0; round < 20 && posted > 0; round++) {
      posted = 0;
      await messageRoundTrip();
    }
    posted = 0;
  }

  // ── Scroll events: exactly one per changed position ──────────────────────
  // frame() announces a new scroll position itself, right away, so the
  // page's scroll handlers run *before* this frame's animation callbacks.
  // The browser would announce the same position again a moment later (its
  // own "scroll" event during rendering). Seen twice, a position looks like
  // "no movement": scroll velocity and direction read 0, so effects such as
  // Lenis "skew by speed" vanished from the video (fixture 04). That second,
  // duplicate event is stopped here, before any page listener sees it.
  // Scrolls the site makes itself (a smooth-scroll library, scrollIntoView)
  // land on a new position and pass through untouched.
  const scrollTop = () => (document.scrollingElement || document.documentElement).scrollTop;
  let announcedY = 0;
  function announceScroll() {
    const y = scrollTop();
    if (y === announcedY) return;
    announcedY = y;
    document.dispatchEvent(new Event("scroll", { bubbles: true }));
  }
  window.addEventListener(
    "scroll",
    (event) => {
      if (!event.isTrusted || event.target !== document) return;
      const y = scrollTop();
      if (y === announcedY) event.stopImmediatePropagation();
      else announcedY = y;
    },
    true,
  );

  // ── Intersection observers (scroll-reveal effects) ────────────────────────
  // The browser computes "element entered the viewport" during its own
  // real-time rendering and normally delivers the callbacks in a task whose
  // timing depends on how busy the machine is. That made a reveal start one
  // frame earlier or later between runs. So ScrollRecord delivers them
  // itself at a fixed point of every frame: wait until the browser has
  // computed intersections for the new scroll position, then take the
  // pending records (takeRecords) and call the page's callback directly.
  // Pages without observers skip all of this and render faster.
  const observers = new Set(); // WeakRef<IntersectionObserver>
  const observerCallbacks = new WeakMap();
  if (typeof window.IntersectionObserver === "function") {
    const RealObserver = window.IntersectionObserver;
    window.IntersectionObserver = class IntersectionObserver extends RealObserver {
      constructor(callback, options) {
        super(callback, options);
        observerCallbacks.set(this, callback);
        observers.add(new WeakRef(this));
      }
    };
  }
  function deliverObserverRecords() {
    for (const ref of observers) {
      const observer = ref.deref();
      if (!observer) {
        observers.delete(ref);
        continue;
      }
      const records = observer.takeRecords();
      const callback = observerCallbacks.get(observer);
      if (records.length && typeof callback === "function") {
        try {
          callback.call(observer, records, observer);
        } catch (err) {
          console.error("[page error during ScrollRecord frame]", err);
        }
      }
    }
  }
  // Rendering step 1 runs our callback, then computes intersections for the
  // new scroll position. In rendering step 2 those results are guaranteed to
  // exist: either already delivered by the browser, or waiting to be taken.
  const settleObservers = () =>
    new Promise((resolve) =>
      realRequestFrame(() =>
        realRequestFrame(() => {
          deliverObserverRecords();
          resolve();
        }),
      ),
    );

  // ── CSS & Web animations ──────────────────────────────────────────────────
  // The browser's own animation clock stands still (Animation.setPlaybackRate
  // 0, see page.rs). Every frame, each *running* animation is moved forward
  // by exactly the frame's time: currentTime += dt × playbackRate.
  // Why "move running ones forward" and not "pause everything, then seek":
  //   - the site keeps control: `animation-play-state: paused → running`
  //     (the common "start when in view" reveal), anim.pause()/play() and
  //     :hover pauses keep working, because ScrollRecord never calls
  //     pause()/play() itself (CSS ignores play-state after such calls);
  //   - animations really finish: animationend/transitionend fire and the
  //     Web Animations `finished` promise / `onfinish` resolve, so effects
  //     chained on them continue (fixture 02 "chain").
  // New animations start at this frame's time (currentTime 0), whatever
  // real time passed while the page was loading. Scroll-driven animations
  // (animation-timeline: scroll()/view()) follow the scroll position by
  // themselves and are left alone.
  const seen = new WeakSet();
  const onPageClock = (anim) => anim.timeline === document.timeline;
  function syncAnimations(dtMs) {
    for (const anim of document.getAnimations()) {
      if (!onPageClock(anim)) continue;
      try {
        if (!seen.has(anim)) {
          seen.add(anim);
          if (anim.playState === "running") anim.currentTime = 0;
        } else if (dtMs > 0 && anim.playState === "running") {
          anim.currentTime = (Number(anim.currentTime) || 0) + dtMs * anim.playbackRate;
        }
      } catch {
        /* removed meanwhile */
      }
    }
  }

  // ── Video and audio ───────────────────────────────────────────────────────
  // <video> decodes on its own clock: left alone, it plays in real time, so
  // every render would show different video frames (fixture 06). ScrollRecord
  // keeps one "media time" per element and moves it with page time:
  //   - the real element never plays: it stays paused and is seeked to its
  //     media time before every frame. A video that has never played shows
  //     the right picture after every seek; one that Chrome had played and
  //     that was paused afterwards sometimes kept showing an old picture
  //     (found under load). So the `autoplay` attribute is taken over as
  //     soon as an element appears, before Chrome could act on it (it only
  //     autoplays once data has arrived);
  //   - to the site it still behaves like a playing video: `paused`,
  //     `ended`, `currentTime` and `playbackRate` report the media time and
  //     state, play()/pause() change them, and play / playing / pause / ended
  //     events are sent at the frame where they happen. The browser's own
  //     play/playing/pause events (caused by keeping it paused) are hidden;
  //   - only videos on screen are seeked (decoding is the slow part); audio
  //     is never seeked (the video has no sound), it only reports its time.
  const Media = HTMLMediaElement.prototype;
  const prop = (name) => Object.getOwnPropertyDescriptor(Media, name);
  const realPause = Media.pause;
  const realLoad = Media.load;
  const realPaused = prop("paused");
  const realTime = prop("currentTime");
  const realRate = prop("playbackRate");
  const mediaState = new WeakMap();
  const offDom = new Set(); // played elements outside the document (new Audio())
  const autoplays = new WeakSet(); // elements whose `autoplay` ScrollRecord took over

  function takeOverAutoplay(el) {
    if (el.hasAttribute("autoplay")) {
      autoplays.add(el);
      el.removeAttribute("autoplay");
    }
  }
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === "attributes") takeOverAutoplay(r.target);
      for (const node of r.addedNodes) {
        if (node instanceof HTMLMediaElement) takeOverAutoplay(node);
        else if (node.querySelectorAll) node.querySelectorAll("[autoplay]").forEach(takeOverAutoplay);
      }
    }
  }).observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ["autoplay"] });

  function mediaOf(el) {
    let st = mediaState.get(el);
    if (!st) {
      const playingNow = !realPaused.get.call(el);
      st = {
        playing: autoplays.has(el) || playingNow,
        base: 0, // media time (s) at page time `since`
        since: now,
        rate: realRate.get.call(el) || 1,
        // What the page has been told: "paused", "play" (play sent, waiting
        // for data) or "playing". The browser already told it if it started
        // an autoplay video before ScrollRecord first saw the element.
        told: playingNow ? "playing" : "paused",
        endedNow: false,
        waitedForData: false,
      };
      mediaState.set(el, st);
    }
    return st;
  }
  // Media time is computed from a fixed starting point (not summed frame by
  // frame), so 0.1 s steps never drift into the previous video frame.
  const mediaTime = (st) => (st.playing ? st.base + ((now - st.since) / 1000) * st.rate : st.base);
  function setMedia(st, changes) {
    st.base = mediaTime(st);
    st.since = now;
    Object.assign(st, changes);
  }
  const atEnd = (el, st) => {
    const d = el.duration;
    return !el.loop && Number.isFinite(d) && d > 0 && mediaTime(st) >= d;
  };

  // ── What the site can call and read ──
  Media.play = function () {
    const st = mediaOf(this);
    if (!st.playing) {
      if (atEnd(this, st)) st.base = 0; // play() after the end starts over
      setMedia(st, { playing: true });
    }
    if (!this.isConnected) offDom.add(this);
    // No data yet (preload="none"): load it, like a real play() would.
    if (this.readyState === 0 && this.networkState !== HTMLMediaElement.NETWORK_LOADING) {
      if (this.preload === "none") this.preload = "auto";
      realLoad.call(this);
    }
    return Promise.resolve();
  };
  Media.pause = function () {
    setMedia(mediaOf(this), { playing: false });
    if (!realPaused.get.call(this)) realPause.call(this);
  };
  Media.load = function () {
    const st = mediaOf(this);
    setMedia(st, { base: 0, playing: autoplays.has(this), waitedForData: false });
    return realLoad.call(this);
  };
  const virtual = (name, get, set) =>
    Object.defineProperty(Media, name, { configurable: true, enumerable: true, get, set });
  virtual("paused", function () {
    return !mediaOf(this).playing;
  });
  virtual(
    "autoplay",
    function () {
      return autoplays.has(this);
    },
    function (value) {
      if (value) autoplays.add(this);
      else autoplays.delete(this);
    },
  );
  virtual("ended", function () {
    return atEnd(this, mediaOf(this));
  });
  virtual(
    "playbackRate",
    function () {
      return mediaOf(this).rate;
    },
    function (value) {
      setMedia(mediaOf(this), { rate: Number(value) });
    },
  );
  virtual(
    "currentTime",
    function () {
      return mediaTime(mediaOf(this));
    },
    function (value) {
      setMedia(mediaOf(this), { base: Math.max(0, Number(value) || 0) });
      realTime.set.call(this, mediaOf(this).base);
    },
  );
  // The browser's own play/playing/pause events of managed elements say
  // nothing true about the video the page shows; ScrollRecord sends its own.
  for (const type of ["play", "playing", "pause"]) {
    window.addEventListener(
      type,
      (event) => {
        if (event.isTrusted && event.target instanceof HTMLMediaElement && mediaState.has(event.target)) {
          event.stopImmediatePropagation();
        }
      },
      true,
    );
  }

  function allMedia() {
    const list = [...document.querySelectorAll("video, audio")];
    for (const el of offDom) {
      if (el.isConnected || !mediaOf(el).playing) offDom.delete(el);
      else list.push(el);
    }
    return list;
  }
  const once = (el, events, ms) =>
    withTimeout(
      new Promise((resolve) => {
        for (const e of events) el.addEventListener(e, resolve, { once: true });
      }),
      ms,
    );
  const onScreen = (el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && r.bottom > 0 && r.top < innerHeight;
  };

  // Tells the page about state changes since the last frame, in order.
  function announce(el, st) {
    const send = (type) => el.dispatchEvent(new Event(type));
    if (st.playing) {
      if (st.told === "paused") {
        st.told = "play";
        send("play");
      }
      if (st.told === "play" && el.readyState >= 3) {
        st.told = "playing";
        send("playing");
      }
    } else if (st.told !== "paused") {
      st.told = "paused";
      send("pause");
      if (st.endedNow) send("ended");
    }
    st.endedNow = false;
  }

  // Ends or loops elements that reached their end, keeps every element
  // paused for real, then seeks the visible videos to their media time and
  // waits until the new picture is decoded. A second pass checks that
  // nothing moved meanwhile.
  async function syncMedia() {
    for (let pass = 0; pass < 3; pass++) {
      const waits = [];
      let corrected = false;
      for (const el of allMedia()) {
        const st = mediaOf(el);
        // Only if something played it for real anyway (e.g. its own controls).
        if (!realPaused.get.call(el)) {
          realPause.call(el);
          corrected = true;
        }
        const d = el.duration;
        if (st.playing && Number.isFinite(d) && d > 0 && mediaTime(st) >= d) {
          if (el.loop) setMedia(st, { base: mediaTime(st) % d });
          else setMedia(st, { base: d, playing: false, endedNow: true });
        }
        announce(el, st);
        if (el.tagName !== "VIDEO" || !onScreen(el)) continue;
        // A video that should show a picture but has no data yet: wait for
        // it once (otherwise it would pop in at a random frame).
        if (el.readyState < 2 && !st.waitedForData && (st.playing || el.preload !== "none")) {
          st.waitedForData = true;
          if (el.networkState !== HTMLMediaElement.NETWORK_NO_SOURCE) {
            await once(el, ["loadeddata", "error"], 10000);
          }
        }
        // Compared with where the element really is, not where it was sent.
        const t = mediaTime(st);
        if (el.readyState >= 1 && Math.abs(realTime.get.call(el) - t) > 0.0005) {
          realTime.set.call(el, t);
          corrected = true;
        }
        if (el.seeking) waits.push(once(el, ["seeked", "error"], 5000));
      }
      if (waits.length) await Promise.all(waits);
      if (!corrected) break;
    }
  }

  // ── Animated images (GIF, animated WebP / PNG / AVIF) ─────────────────────
  // The browser animates these on its own real-time clock, and nothing in
  // the page can slow it down. So ScrollRecord decodes each animated image
  // once (ImageDecoder, built into Chrome/Edge), keeps every frame as a
  // still picture, and shows the frame that belongs to the page time by
  // swapping the <img> source. Only images on screen are swapped.
  // Limits: CSS background images and <picture> sources keep real time;
  // images from other sites are left alone when the browser forbids reading
  // them.
  const decoded = new Map(); // original URL → Promise<animation | null>
  const imageState = new WeakMap(); // <img> → { url, start, shown, frames }
  const STILL = /\.(jpe?g|svg|bmp|ico)([?#]|$)/i;

  async function decodeAnimation(url) {
    if (typeof ImageDecoder !== "function" || STILL.test(url)) return null;
    try {
      const res = await realFetch(url);
      const type = (res.headers.get("content-type") || "").split(";")[0].trim();
      if (!res.ok || !(await ImageDecoder.isTypeSupported(type))) return null;
      const decoder = new ImageDecoder({ data: await res.arrayBuffer(), type });
      await decoder.tracks.ready;
      const track = decoder.tracks.selectedTrack;
      if (!track || !track.animated) return null;
      await decoder.completed;
      const frames = [];
      let end = 0;
      for (let i = 0; i < track.frameCount; i++) {
        const { image } = await decoder.decode({ frameIndex: i });
        const canvas = new OffscreenCanvas(image.displayWidth, image.displayHeight);
        canvas.getContext("2d").drawImage(image, 0, 0);
        // Like browsers: frames of 10 ms or less are shown for 100 ms.
        const ms = (image.duration || 0) / 1000;
        end += ms > 10 ? ms : 100;
        image.close();
        const blob = await canvas.convertToBlob({ type: "image/png" });
        frames.push({ src: URL.createObjectURL(blob), end });
      }
      decoder.close();
      const plays = track.repetitionCount === Infinity ? Infinity : track.repetitionCount + 1;
      return { frames, total: end, plays };
    } catch {
      return null;
    }
  }

  async function syncImages() {
    const loads = [];
    for (const img of document.images) {
      if (!img.complete || !onScreen(img)) continue;
      let st = imageState.get(img);
      const current = img.currentSrc || img.src;
      if (!st || (current !== st.url && current !== st.shownSrc)) {
        if (!current || current.startsWith("blob:")) continue;
        st = { url: current, start: now, shown: -1, shownSrc: null, anim: null };
        imageState.set(img, st);
        if (!decoded.has(current)) decoded.set(current, decodeAnimation(current));
        st.anim = await decoded.get(current);
      }
      const anim = st.anim;
      if (!anim) continue;
      const elapsed = now - st.start;
      const finished = anim.plays !== Infinity && elapsed >= anim.total * anim.plays;
      const local = finished ? anim.total - 0.001 : elapsed % anim.total;
      const index = anim.frames.findIndex((f) => local < f.end);
      if (index === st.shown || index < 0) continue;
      st.shown = index;
      st.shownSrc = anim.frames[index].src;
      if (img.hasAttribute("srcset")) img.removeAttribute("srcset");
      img.src = st.shownSrc;
      loads.push(img.decode().catch(() => {}));
    }
    if (loads.length) await Promise.all(loads);
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

  // ── Cursor image under the pointer (HANDOFF FR-CUR-03) ────────────────────
  // Same rules as a desktop browser: CSS `cursor` wins; `auto` shows the
  // I-beam over editable fields and over selectable text, otherwise an arrow.
  const TEXT_INPUTS =
    'textarea, input:not([type]), input[type="text"], input[type="email"], input[type="search"], ' +
    'input[type="password"], input[type="url"], input[type="tel"], input[type="number"]';
  function cursorKind(x, y) {
    const el = document.elementFromPoint(x, y);
    if (!el) return "arrow";
    const cs = getComputedStyle(el);
    if (cs.cursor === "pointer") return "pointer";
    if (cs.cursor === "text" || cs.cursor === "vertical-text") return "text";
    if (cs.cursor !== "auto") return "arrow";
    if (el.isContentEditable || el.matches(TEXT_INPUTS)) return "text";
    if (cs.userSelect === "none") return "arrow";
    const caret = document.caretRangeFromPoint ? document.caretRangeFromPoint(x, y) : null;
    const node = caret && caret.startContainer;
    if (node && node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const r of range.getClientRects()) {
        if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return "text";
      }
    }
    return "arrow";
  }

  // True while a finite animation or transition is still playing (used by
  // the probe to know when a block has finished appearing). Paused and
  // scroll-driven animations don't count: waiting would not finish them.
  function animationsBusy() {
    for (const anim of document.getAnimations()) {
      if (!onPageClock(anim) || anim.playState !== "running" || !anim.effect) continue;
      const end = anim.effect.getComputedTiming().endTime;
      if (Number.isFinite(end)) return true;
    }
    return false;
  }

  window.__sr = Object.freeze({
    /** Current page time in ms. */
    now: () => now,

    /** Moves page time forward without drawing (used for warm-up). */
    advance(ms) {
      advance(ms);
      syncAnimations(ms);
      return now;
    },

    /**
     * One video frame: scroll, move time forward, position animations.
     * Resolves to the scroll position the browser actually applied.
     */
    async frame(scrollY, dtMs) {
      window.scrollTo(0, scrollY);
      // Deliver the scroll event now, so scroll-linked code reacts in this frame.
      announceScroll();
      advance(dtMs);
      syncAnimations(dtMs);
      let waited = false;
      if (posted > 0) {
        await drainMessages();
        waited = true;
      }
      if (observers.size > 0) {
        await settleObservers();
        if (posted > 0) await drainMessages();
        waited = true;
      }
      // Video/audio and animated images show this frame's time; their
      // play/pause events may start more work for the page.
      if (document.querySelector("video, audio") || offDom.size) {
        await syncMedia();
        waited = true;
      }
      if (document.images.length) await syncImages();
      if (posted > 0) await drainMessages();
      // Transitions started by observer callbacks, framework renders or
      // media events begin at this frame's time.
      if (waited) syncAnimations(0);
      return scrollTop();
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
      announceScroll();
      await drainMessages();
      syncAnimations(0);
      await syncMedia();
      await syncImages();
      return { invalidSelectors: invalid };
    },

    /**
     * Pointer bookkeeping after ScrollRecord sent mouse/touch input for this
     * frame: hover transitions that just started begin at this frame's time.
     * Returns the cursor image the page asks for under (x, y), if wanted.
     */
    async pointer(x, y, sync, wantKind) {
      if (posted > 0) await drainMessages();
      if (sync) syncAnimations(0);
      return wantKind ? cursorKind(x, y) : null;
    },

    /**
     * Probe: scrolls to `scrollY` and lets the page settle (reveal effects,
     * transitions) for at most `maxMs` of page time. Used before recording
     * to find where buttons are once a block has appeared; the page is
     * reloaded afterwards, so the probe never shows up in the video.
     */
    async settleAt(scrollY, maxMs) {
      const step = 50;
      for (let waited = 0; waited < maxMs; waited += step) {
        await this.frame(scrollY, step);
        // At least a few steps: reveals start only after observers fire.
        if (waited >= 3 * step && !animationsBusy()) break;
      }
      return now;
    },

    /**
     * Measures elements in page coordinates (CSS px). Items with `within`
     * (targets of actions) are searched inside that block's element first,
     * so `{ "text": "Start" }` means the Start button *of this block*.
     */
    measure(items) {
      const result = {
        viewportWidth: innerWidth,
        viewportHeight: innerHeight,
        scrollHeight: (document.scrollingElement || document.documentElement).scrollHeight,
        topInset: topInset(),
        blocks: {},
      };
      const elements = new Map();
      for (const item of items) {
        const scope = item.within != null ? elements.get(item.within) : null;
        const tried = [];
        let found = null;
        let ambiguous = false;
        item.locators.forEach((loc, index) => {
          if (found) return;
          let els;
          try {
            els = findAll(loc);
          } catch (err) {
            tried.push(`${describe(loc)}: invalid (${err.message})`);
            return;
          }
          if (scope) {
            const inside = els.filter((el) => scope.contains(el) && el !== scope);
            if (inside.length) els = inside;
          }
          if (loc.nth != null) els = els[loc.nth] ? [els[loc.nth]] : [];
          tried.push(`${describe(loc)}: ${els.length} match${els.length === 1 ? "" : "es"}`);
          if (els.length === 1) found = { el: els[0], index };
          else if (els.length > 1) ambiguous = true;
        });
        if (!found) {
          result.blocks[item.id] = { status: ambiguous ? "ambiguous" : "not_found", tried };
          continue;
        }
        elements.set(item.id, found.el);
        const r = found.el.getBoundingClientRect();
        result.blocks[item.id] = {
          status: "found",
          top: r.top + scrollY,
          height: r.height,
          left: r.left + scrollX,
          width: r.width,
          matched: found.index,
        };
      }
      return result;
    },
  });
})();
