//! Starting and stopping the browser process.
//!
//! Why: the browser runs headless with a fresh, throw-away profile so the
//! user's own tabs, extensions and logins never leak into a recording, and
//! two renders never interfere. The DevTools port is chosen by the browser
//! (`--remote-debugging-port=0`) and read back from the `DevToolsActivePort`
//! file in the profile folder, so no port can ever be "already in use".

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sr_core::{AppError, AppResult, ErrorCode};

/// Options for starting the browser.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Run without a window (default). `false` helps debugging.
    pub headless: bool,
    /// Additional command-line flags.
    pub extra_args: Vec<String>,
    /// How long to wait for the browser to open its DevTools port.
    pub startup_timeout: Duration,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            extra_args: Vec::new(),
            startup_timeout: Duration::from_secs(30),
        }
    }
}

/// A running browser process with its temporary profile folder.
#[derive(Debug)]
pub struct BrowserProcess {
    child: Child,
    profile_dir: PathBuf,
}

impl BrowserProcess {
    /// Starts the browser and returns it together with the DevTools
    /// WebSocket URL of the browser endpoint.
    pub fn launch(executable: &Path, opts: &LaunchOptions) -> AppResult<(BrowserProcess, String)> {
        let profile_dir = unique_profile_dir();
        std::fs::create_dir_all(&profile_dir).map_err(|err| {
            AppError::new(
                ErrorCode::BrowserLaunchFailed,
                format!("profile dir: {err}"),
            )
        })?;
        let stderr_log = std::fs::File::create(profile_dir.join("browser-stderr.log")).ok();

        let mut cmd = Command::new(executable);
        cmd.args(flags(&profile_dir, opts))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr_log.map_or_else(Stdio::null, Stdio::from));
        hide_console_window(&mut cmd);

        let child = cmd.spawn().map_err(|err| {
            AppError::new(
                ErrorCode::BrowserLaunchFailed,
                format!("cannot start {}: {err}", executable.display()),
            )
            .with_param("path", executable.display())
        })?;
        let mut process = BrowserProcess { child, profile_dir };
        tracing::info!(browser = %executable.display(), "browser started");

        match process.wait_for_devtools(opts.startup_timeout) {
            Ok(url) => Ok((process, url)),
            Err(err) => {
                process.stop();
                Err(err)
            }
        }
    }

    // ── Waiting for the DevTools endpoint ───────────────────────────────────
    // The browser writes "<port>\n<path>" into DevToolsActivePort once ready.
    fn wait_for_devtools(&mut self, timeout: Duration) -> AppResult<String> {
        let file = self.profile_dir.join("DevToolsActivePort");
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(&file) {
                let mut lines = text.lines();
                if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
                    return Ok(format!("ws://127.0.0.1:{}{}", port.trim(), path.trim()));
                }
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(AppError::new(
                    ErrorCode::BrowserLaunchFailed,
                    format!(
                        "browser exited during startup ({status}): {}",
                        self.stderr_tail()
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(AppError::new(
            ErrorCode::BrowserLaunchFailed,
            format!(
                "browser did not open its DevTools port within {}s: {}",
                timeout.as_secs(),
                self.stderr_tail()
            ),
        ))
    }

    /// Last lines the browser wrote to stderr (for error messages).
    pub fn stderr_tail(&self) -> String {
        let text = std::fs::read_to_string(self.profile_dir.join("browser-stderr.log"))
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().rev().take(5).collect();
        lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
    }

    /// Stops the browser (if still running) and deletes the profile folder.
    pub fn stop(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        // Windows may keep files locked for a moment after the process ends.
        for _ in 0..10 {
            if std::fs::remove_dir_all(&self.profile_dir).is_ok() || !self.profile_dir.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        tracing::warn!(dir = %self.profile_dir.display(), "could not delete temporary browser profile");
    }
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        if self.profile_dir.exists() || matches!(self.child.try_wait(), Ok(None)) {
            self.stop();
        }
    }
}

// ── Command-line flags ──────────────────────────────────────────────────────
// Each flag removes a source of randomness or interference from recordings.
fn flags(profile_dir: &Path, opts: &LaunchOptions) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", profile_dir.display()),
        "--remote-debugging-port=0".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-extensions".into(),
        "--disable-sync".into(),
        "--disable-background-networking".into(),
        // Keep rendering at full speed even though no window is visible.
        "--disable-background-timer-throttling".into(),
        "--disable-backgrounding-occluded-windows".into(),
        "--disable-renderer-backgrounding".into(),
        // Clean frames: no scrollbars, no translate bar, identical colours on every OS.
        "--hide-scrollbars".into(),
        "--disable-features=Translate,MediaRouter,OptimizationHints".into(),
        "--force-color-profile=srgb".into(),
        "--mute-audio".into(),
        // Videos the site starts with play() always start (the recording has
        // no sound anyway), instead of depending on Chrome's autoplay rules.
        "--autoplay-policy=no-user-gesture-required".into(),
        // Same frames on machines without a GPU (CI, VMs, remote desktops).
        // There Chrome draws in software and reuses old tile memory. An
        // element with text on an opaque background that moves by a
        // fraction of a pixel (GSAP: translate3d(0, 8.89px, 0)) gets a new
        // tile whose bottom/right pixel row Chrome does not clear, so what
        // the reused memory held shows through. By default any *larger*
        // leftover tile may be taken, and which one is free depends on
        // raster-thread timing and idle clean-up timers: two renders
        // differed in exactly the frames of such a tween (CI on Windows).
        // Same-size reuse only makes that row repeatable (ADR 0004, with
        // the one case it does not cover).
        "--disallow-non-exact-resource-reuse".into(),
        "--password-store=basic".into(),
        "--use-mock-keychain".into(),
    ];
    if opts.headless {
        args.push("--headless=new".into());
    }
    if needs_no_sandbox() {
        // Chrome refuses to run its sandbox as root (containers, some CI).
        args.push("--no-sandbox".into());
    }
    args.extend(opts.extra_args.iter().cloned());
    args.push("about:blank".into());
    args
}

#[cfg(target_os = "linux")]
fn needs_no_sandbox() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).map(str::to_owned))
        .is_some_and(|line| line.split_whitespace().nth(1) == Some("0"))
}

#[cfg(not(target_os = "linux"))]
fn needs_no_sandbox() -> bool {
    false
}

/// On Windows, child processes of a GUI app would flash a console window.
#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

fn unique_profile_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "scrollrecord-browser-{}-{nanos}",
        std::process::id()
    ))
}
