// Fixture 03 behaviour: GSAP + ScrollTrigger in their most common setups.
gsap.registerPlugin(ScrollTrigger);

// ── hero: intro timeline on load ───────────────────────────────────────────
// Why: "animate in on page load" — GSAP's own ticker must follow page time,
//      so the intro takes exactly 1.2 s of video, never more or less.
gsap
  .timeline({ defaults: { ease: "power2.out" } })
  .from(".hero__title", { y: 60, opacity: 0, duration: 0.6 })
  .from(".hero__lead", { y: 30, opacity: 0, duration: 0.4 }, "-=0.2")
  .from(".hero__dots i", { scale: 0, duration: 0.3, stagger: 0.1 });

// ── scrub: progress follows the scroll position ────────────────────────────
// Why: scrub: true must depend only on where the page is scrolled; scrub: 0.5
//      adds a 0.5 s catch-up that must run on page time.
const scrubbed = { trigger: ".scrub", start: "top bottom", end: "bottom top" };
gsap.to("#scrub-exact", { scaleX: 1, ease: "none", scrollTrigger: { ...scrubbed, scrub: true } });
gsap.to("#scrub-smooth", { scaleX: 1, ease: "none", scrollTrigger: { ...scrubbed, scrub: 0.5 } });

// ── pin: section stays on screen while its row moves sideways ──────────────
// Why: pinning wraps the section in a "pin spacer" that adds height to the
//      page; block positions measured before recording must include it.
gsap.to("#pin-row", {
  x: -1200,
  ease: "none",
  scrollTrigger: { trigger: ".pin", start: "top top", end: "+=1200", pin: true, scrub: true },
});

// ── reveal: a time-based tween started by scrolling ────────────────────────
gsap.utils.toArray(".card").forEach((card, i) => {
  gsap.from(card, {
    y: 80,
    opacity: 0,
    duration: 0.6,
    delay: i * 0.1,
    scrollTrigger: { trigger: card, start: "top 85%", toggleActions: "play none none none" },
  });
});
