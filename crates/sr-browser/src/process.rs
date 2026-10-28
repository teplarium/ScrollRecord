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
    /// Start the browser below normal priority (default), so a render does
    /// not make the computer sluggish. Frames stay identical, only slower
    /// when other programs want the CPU (HANDOFF §14.1).
    /// Limit on Windows: Chrome sets the priority of its tab processes
    /// (normal) and GPU process (above normal) itself; only the main
    /// process (which also encodes the screenshots) and its helpers stay
    /// below normal. On Unix every child inherits the niceness and cannot
    /// lower it without admin rights.
    pub low_priority: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            extra_args: Vec::new(),
            startup_timeout: Duration::from_secs(30),
            low_priority: true,
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
        let profile_dir = claim_profile_dir().map_err(|err| {
            AppError::new(
                ErrorCode::BrowserLaunchFailed,
                format!("profile dir: {err}"),
            )
        })?;
        let stderr_log = std::fs::File::create(profile_dir.join("browser-stderr.log")).ok();

        let mut cmd = background_command(executable, opts.low_priority);
        cmd.args(flags(&profile_dir, opts))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr_log.map_or_else(Stdio::null, Stdio::from));

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

    /// Process id of the browser (its main process).
    pub fn pid(&self) -> u32 {
        self.child.id()
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

// ── Starting the process: no console window, optionally low priority ────────
// Windows: a child of a GUI app would flash a console window, so it gets
// CREATE_NO_WINDOW; low priority is BELOW_NORMAL_PRIORITY_CLASS. Chrome's
// own child processes start with it, but Chrome raises its tab and GPU
// processes again (measured 2026-10-28: tab normal, GPU above normal; no
// command-line switch prevents that). They did about 17 % of a 4K render's
// CPU work; ffmpeg and Chrome's main process, which stay low, about 65 %.
// Unix: low priority means starting through `nice -n 10`. `nice` replaces
// itself with the browser (exec), so the process id stays the browser's.
// Without `nice` the browser simply runs at normal priority.
// (sr-encoder starts ffmpeg the same way.)
#[cfg(windows)]
fn background_command(program: &Path, low_priority: bool) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    let mut cmd = Command::new(program);
    cmd.creation_flags(if low_priority {
        CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    } else {
        CREATE_NO_WINDOW
    });
    cmd
}

#[cfg(not(windows))]
fn background_command(program: &Path, low_priority: bool) -> Command {
    match low_priority.then(find_nice).flatten() {
        Some(nice) => {
            let mut cmd = Command::new(nice);
            cmd.args(["-n", "10"]).arg(program);
            cmd
        }
        None => Command::new(program),
    }
}

#[cfg(not(windows))]
fn find_nice() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("nice"))
        .find(|p| p.is_file())
}

// ── Profile folder ──────────────────────────────────────────────────────────
// Each browser needs a folder no other browser uses. Chrome allows one running
// browser per profile folder: a second start on the same folder hands its
// command line to the first one and exits (code 21). Process id + clock alone
// is not unique: two browsers started in the same clock tick (parallel tests;
// later a render next to the editor's page analysis) got one folder, then
// drove one shared browser, and whichever finished first closed it under the
// other (phase-3-plan step 1b-fix). `create_dir` fails if the folder exists,
// so exactly one caller gets each name; the others try the next number.

/// Creates a new, empty profile folder in the system temp folder.
fn claim_profile_dir() -> std::io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    claim_profile_dir_in(&std::env::temp_dir(), nanos)
}

/// Creates a folder in `parent` named after this process and `nanos` that no
/// other caller has, adding a number when the name is already taken.
fn claim_profile_dir_in(parent: &Path, nanos: u128) -> std::io::Result<PathBuf> {
    const ATTEMPTS: u32 = 1000;
    std::fs::create_dir_all(parent)?;
    for n in 0..ATTEMPTS {
        let dir = parent.join(format!(
            "scrollrecord-browser-{}-{nanos}-{n}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "{ATTEMPTS} profile folders already exist in {}",
            parent.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Regression (phase-3-plan step 1b-fix): two browsers started in the
    /// same clock tick got the same profile folder, so the second Chrome
    /// handed over to the first (exit code 21) and both callers drove one
    /// browser — whichever finished first closed it under the other
    /// ("Connection reset", "Session with given id not found"). Here several
    /// threads claim folders with one and the same clock reading; every
    /// folder must still be a different one.
    #[test]
    fn launches_in_the_same_clock_tick_never_share_a_profile_folder() {
        let parent =
            std::env::temp_dir().join(format!("scrollrecord-profile-test-{}", std::process::id()));
        std::fs::create_dir_all(&parent).unwrap();
        let dirs: Vec<PathBuf> = std::thread::scope(|s| {
            let claims: Vec<_> = (0..4)
                .map(|_| {
                    s.spawn(|| {
                        (0..25)
                            .map(|_| claim_profile_dir_in(&parent, 42).unwrap())
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            claims.into_iter().flat_map(|c| c.join().unwrap()).collect()
        });
        let distinct: HashSet<&PathBuf> = dirs.iter().collect();
        let _ = std::fs::remove_dir_all(&parent);
        assert_eq!(distinct.len(), dirs.len(), "two launches got one folder");
    }
}
