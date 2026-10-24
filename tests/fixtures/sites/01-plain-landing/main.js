// Fixture 01 behaviour. Plain JS, no dependencies.

// ── Burger menu ─────────────────────────────────────────────────────────────
// Why: on mobile the nav is hidden; scenarios must click the burger first
//      (tests per-viewport `overrides` in the flow format).
const burger = document.querySelector(".burger");
const nav = document.querySelector(".nav");
burger.addEventListener("click", () => {
  const open = nav.classList.toggle("is-open");
  burger.setAttribute("aria-expanded", String(open));
});

// ── Scroll reveal ───────────────────────────────────────────────────────────
// Why: the most common scroll-triggered effect on landing pages. Capture must
//      scroll for real (not screenshot-and-pan) for these cards to appear.
const observer = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      }
    }
  },
  { threshold: 0.2 },
);
document.querySelectorAll(".reveal").forEach((el) => observer.observe(el));
