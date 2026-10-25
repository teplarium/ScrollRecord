// Fixture 02 behaviour. Plain JS, no dependencies.

// ── intro: start the long transition right after the first frame ───────────
// Why: "add a class in requestAnimationFrame" is how sites start CSS
//      transitions on load. The bar must fill exactly 500 ms (delay) to
//      2500 ms of page time after that frame.
requestAnimationFrame(() => document.getElementById("bar").classList.add("go"));

// ── chain: each step starts when the previous one *ends* ───────────────────
// Why: if a captured animation never reports its end (animationend,
//      transitionend, Web Animations `finished`), the chain stops and the
//      boxes stay grey in the video.
const [a, b, c, d] = ["a", "b", "c", "d"].map((id) => document.getElementById(`chain-${id}`));
a.addEventListener("animationend", () => b.classList.add("on"));
b.addEventListener("transitionend", () => c.classList.add("on"));
c.addEventListener("transitionend", () => {
  const spin = d.animate([{ transform: "rotate(0)" }, { transform: "rotate(90deg)" }], { duration: 300 });
  spin.finished.then(() => d.classList.add("on"));
});

// ── play-state: run the paused entrance once the section is in view ────────
const observer = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("in-view");
        observer.unobserve(entry.target);
      }
    }
  },
  { threshold: 0.3 },
);
document.querySelectorAll(".ps").forEach((el) => observer.observe(el));
