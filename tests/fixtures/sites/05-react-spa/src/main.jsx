/**
 * Fixture 05 — React single-page app (source of app.js).
 *
 * Purpose: a typical framework site (HANDOFF D-15, R-21): the page is empty
 * until JavaScript renders it, content arrives with fetch(), routes change
 * without a page load (history.pushState), and React schedules its work in
 * its own tasks (MessageChannel), outside ScrollRecord's frame callbacks.
 * Covers:
 *   hero      entrance started from a passive effect (useEffect → class → CSS)
 *   stats     count-up re-rendered by React on every animation frame
 *   features  list loaded with fetch() from data/features.json
 *   tabs      click → state → new panel with a CSS transition
 *   /pricing  a second route, opened by link or directly (deep link)
 * Build: see README.md in this folder. app.js is the committed build output.
 */
import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

// ── Tiny router (history API) ──────────────────────────────────────────────
// Why not a router library: keeps the bundle small and the behaviour obvious;
// it does the same thing routers do (pushState + popstate + re-render).
function useRoute() {
  const [path, setPath] = useState(location.pathname);
  useEffect(() => {
    const onPop = () => setPath(location.pathname);
    addEventListener("popstate", onPop);
    return () => removeEventListener("popstate", onPop);
  }, []);
  const go = (to) => {
    history.pushState(null, "", to);
    setPath(to);
    scrollTo(0, 0);
  };
  return [path, go];
}

function Link({ to, go, children }) {
  return (
    <a
      href={to}
      onClick={(e) => {
        e.preventDefault();
        go(to);
      }}
    >
      {children}
    </a>
  );
}

// ── Hero: entrance from a passive effect ───────────────────────────────────
function Hero() {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setShown(true));
    return () => cancelAnimationFrame(id);
  }, []);
  return (
    <section className={`hero${shown ? " is-shown" : ""}`} data-sr="hero">
      <h1>Ship with React</h1>
      <p>Rendered in the browser, like most framework sites.</p>
    </section>
  );
}

// ── Stats: React re-renders on every animation frame ───────────────────────
// Why: each frame's number is a React state update. If React rendered it
//      after the screenshot instead of before, the video would lag or jitter.
function useInView(ref) {
  const [inView, setInView] = useState(false);
  useEffect(() => {
    const io = new IntersectionObserver(([entry]) => entry.isIntersecting && setInView(true), {
      threshold: 0.5,
    });
    io.observe(ref.current);
    return () => io.disconnect();
  }, [ref]);
  return inView;
}

function CountUp({ to, ms }) {
  const ref = useRef(null);
  const inView = useInView(ref);
  const [value, setValue] = useState(0);
  useEffect(() => {
    if (!inView) return undefined;
    const start = performance.now();
    let id = requestAnimationFrame(function tick(now) {
      const k = Math.min(1, (now - start) / ms);
      setValue(Math.round(to * k));
      if (k < 1) id = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(id);
  }, [inView, to, ms]);
  return (
    <span className="stat" ref={ref} data-value={value}>
      {value}
    </span>
  );
}

function Stats() {
  return (
    <section className="stats" data-sr="stats">
      <CountUp to={1000} ms={1000} />
      <p>teams already shipping</p>
    </section>
  );
}

// ── Features: content from fetch() ─────────────────────────────────────────
function Features() {
  const [items, setItems] = useState(null);
  useEffect(() => {
    fetch("/data/features.json")
      .then((r) => r.json())
      .then(setItems)
      .catch(() => setItems([]));
  }, []);
  return (
    <section className="features" data-sr="features">
      <h2>Features</h2>
      {items === null ? (
        <p className="loading">Loading…</p>
      ) : (
        <ul>
          {items.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

// ── Tabs: click → state → transition ───────────────────────────────────────
const TABS = ["Design", "Build", "Launch"];
function Tabs() {
  const [active, setActive] = useState(0);
  return (
    <section className="tabs" data-sr="tabs">
      <div className="tabs__bar" role="tablist">
        {TABS.map((t, i) => (
          <button
            key={t}
            role="tab"
            aria-selected={i === active}
            className={i === active ? "is-active" : ""}
            onClick={() => setActive(i)}
          >
            {t}
          </button>
        ))}
      </div>
      <div className="tabs__panel" key={active} data-tab={active}>
        {TABS[active]} panel
      </div>
    </section>
  );
}

function Home({ go }) {
  return (
    <main>
      <Hero />
      <Stats />
      <Features />
      <Tabs />
      <footer className="footer" data-sr="footer">
        <Link to="/pricing" go={go}>
          See pricing
        </Link>
      </footer>
    </main>
  );
}

function Pricing({ go }) {
  return (
    <main>
      <section className="plans" data-sr="plans">
        <h1>Pricing</h1>
        <div className="plans__row">
          <div className="plan">Free</div>
          <div className="plan plan--pro">Pro</div>
        </div>
        <Link to="/" go={go}>
          Back home
        </Link>
      </section>
    </main>
  );
}

function App() {
  const [path, go] = useRoute();
  return path === "/pricing" ? <Pricing go={go} /> : <Home go={go} />;
}

createRoot(document.getElementById("root")).render(<App />);
