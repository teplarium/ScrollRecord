// Fixture 04 behaviour: Lenis driven by GSAP's ticker, as in the Lenis README.
gsap.registerPlugin(ScrollTrigger);

// ── Lenis + ScrollTrigger wiring (official recipe) ─────────────────────────
// Why: this is how most Lenis sites are set up. Lenis must report the
//      position ScrollRecord scrolled to, in the same frame.
const lenis = new Lenis();
lenis.on("scroll", ScrollTrigger.update);
gsap.ticker.add((time) => lenis.raf(time * 1000));
gsap.ticker.lagSmoothing(0);

// ── speed: skew by scroll velocity, hide header while going down ───────────
// Why: velocity and direction come from the *difference* between two scroll
//      events. If the page saw the same position twice in a frame, velocity
//      would read 0 and the effect would vanish from the video.
const skew = document.getElementById("skew");
const topbar = document.getElementById("topbar");
lenis.on("scroll", ({ velocity, direction }) => {
  const deg = Math.max(-20, Math.min(20, velocity * 0.5));
  skew.style.transform = `skewY(${deg.toFixed(2)}deg)`;
  if (direction !== 0) topbar.classList.toggle("is-hidden", direction > 0);
});

// ── parallax: a layer moving slower than the page ──────────────────────────
gsap.to("#layer", {
  y: 200,
  ease: "none",
  scrollTrigger: { trigger: ".parallax", start: "top bottom", end: "bottom top", scrub: true },
});
