// Fixture 06 behaviour.

// ── clip: start the second video once it is in view ────────────────────────
// Why: "play when visible" is the usual way sites start videos further down.
const clip = document.getElementById("clip");
new IntersectionObserver(([entry]) => {
  if (entry.isIntersecting) clip.play();
}).observe(clip);

// ── lottie: the standard player setup (data loaded from a JSON file) ───────
lottie.loadAnimation({
  container: document.getElementById("lottie"),
  renderer: "svg",
  loop: true,
  autoplay: true,
  path: "media/move.json",
});

// ── canvas: redrawn on every animation frame from the frame timestamp ──────
// 2D: a bar that fills over 2 s and starts again.
const c2d = document.getElementById("c2d").getContext("2d");
requestAnimationFrame(function draw(t) {
  c2d.clearRect(0, 0, 400, 40);
  c2d.fillStyle = "rgb(150, 60, 200)";
  c2d.fillRect(0, 0, ((t % 2000) / 2000) * 400, 40);
  requestAnimationFrame(draw);
});
// WebGL: the clear colour's red part follows time (one second per cycle).
const gl = document.getElementById("cgl").getContext("webgl", { preserveDrawingBuffer: false });
if (gl) {
  requestAnimationFrame(function paint(t) {
    gl.clearColor((t % 1000) / 1000, 0.3, 0.6, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    requestAnimationFrame(paint);
  });
}
