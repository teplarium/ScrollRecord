//! # sr-encoder — frames in, video file out
//!
//! Purpose: implements [`VideoSink`] for the output formats: MP4 (H.264) and
//!          WebM (VP9) via an external `ffmpeg` process, or a folder of PNGs.
//! Input:   finished frames as raw RGBA (the compositor has already drawn
//!          the cursor). Raw pixels need no decoding inside ffmpeg; the size
//!          is taken from the first frame, so ffmpeg starts on that frame.
//! Solves:  ffmpeg is the most reliable encoder on every OS. Running it as a
//!          separate program (frames piped into its stdin, no temp files)
//!          avoids linking codec libraries and their licences (HANDOFF D-09).
//!          PNG output needs no ffmpeg at all, a fallback that always works.
//! Colour:  frames are sRGB; the video is tagged BT.709 so players show the
//!          same colours as the browser (an untagged video often looks
//!          slightly washed out).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use sr_core::flow::OutputFormat;
use sr_core::{AppError, AppResult, ErrorCode};

/// Environment variable with an ffmpeg path that overrides auto-detection.
pub const FFMPEG_ENV_VAR: &str = "SR_FFMPEG";

/// Speed/size trade-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    /// Quick encode, slightly bigger file (default).
    #[default]
    Fast,
    /// Slow encode, smallest file at the same visual quality.
    Best,
}

/// What to produce.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodeSettings {
    /// Final width (even).
    pub width: u32,
    /// Final height (even).
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Container/codec.
    pub format: OutputFormat,
    /// Speed/size trade-off.
    pub quality: Quality,
}

/// Receives finished frames in order.
pub trait VideoSink {
    /// Takes frame `index`: `rgba` holds `width × height` pixels, 4 bytes
    /// each, rows top to bottom. Every frame must have the same size.
    fn push(&mut self, index: u32, width: u32, height: u32, rgba: &[u8]) -> AppResult<()>;

    /// Called once after the last frame (flush, close files, finish encoding).
    fn finish(&mut self) -> AppResult<()>;
}

/// Opens the right sink for `settings.format`. For PNG, `output` is a folder.
/// For video, ffmpeg is located now (so a missing ffmpeg fails before any
/// capture work) and started with the first frame.
pub fn open_sink(
    output: &Path,
    settings: &EncodeSettings,
    ffmpeg: Option<&Path>,
) -> AppResult<Box<dyn VideoSink + Send>> {
    match settings.format {
        OutputFormat::Png => Ok(Box::new(PngSequence::create(output)?)),
        OutputFormat::Mp4 | OutputFormat::Webm => {
            let exe = find_ffmpeg(ffmpeg)?;
            Ok(Box::new(FfmpegEncoder::start(&exe, output, settings)?))
        }
    }
}

// ── Finding ffmpeg ──────────────────────────────────────────────────────────

/// Returns the ffmpeg executable. Search order: explicit path → `SR_FFMPEG`
/// → next to the ScrollRecord executable → `PATH` → known install folders
/// (on Windows: winget and `C:\ffmpeg`).
///
/// Why so many places: on Windows, `winget install Gyan.FFmpeg` only updates
/// `PATH` for *new* terminals, and many users simply unzip ffmpeg somewhere.
/// Dropping `ffmpeg.exe` next to `scrollrecord.exe` always works.
pub fn find_ffmpeg(explicit: Option<&Path>) -> AppResult<PathBuf> {
    let not_found = |what: String| AppError::new(ErrorCode::FfmpegNotFound, what);
    if let Some(p) = explicit {
        return if p.is_file() {
            Ok(p.to_path_buf())
        } else {
            Err(not_found(format!("ffmpeg not found at {}", p.display()))
                .with_param("path", p.display()))
        };
    }
    if let Some(p) = std::env::var_os(FFMPEG_ENV_VAR).filter(|v| !v.is_empty()) {
        let p = PathBuf::from(p);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(not_found(format!(
                "{FFMPEG_ENV_VAR} points to a missing file: {}",
                p.display()
            )))
        };
    }
    ffmpeg_candidates()
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| {
            not_found(format!(
                "ffmpeg is not installed or not on PATH. {INSTALL_HINT}"
            ))
        })
}

/// How to get ffmpeg, shown in the SR-5001 message.
#[cfg(windows)]
const INSTALL_HINT: &str =
    "Install it with `winget install Gyan.FFmpeg`, or put ffmpeg.exe next to scrollrecord.exe";
#[cfg(target_os = "macos")]
const INSTALL_HINT: &str = "Install it with `brew install ffmpeg`";
#[cfg(all(unix, not(target_os = "macos")))]
const INSTALL_HINT: &str = "Install it with your package manager, e.g. `sudo apt install ffmpeg`";

/// Places where ffmpeg is commonly found, in priority order.
fn ffmpeg_candidates() -> Vec<PathBuf> {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut out = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        out.push(dir.join(name));
    }
    if let Some(path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&path).map(|dir| dir.join(name)));
    }
    out.extend(platform_candidates());
    out
}

#[cfg(windows)]
fn platform_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        out.extend(winget_candidates(&local.join("Microsoft").join("WinGet")));
    }
    out.push(PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"));
    if let Some(pf) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
        out.push(pf.join("ffmpeg").join("bin").join("ffmpeg.exe"));
    }
    out
}

/// ffmpeg locations inside a winget folder (`%LOCALAPPDATA%\Microsoft\WinGet`):
/// the `Links` shim, and portable packages unpacked as
/// `Packages\<id containing FFmpeg>\<version folder>\bin\ffmpeg.exe`.
/// Platform-independent so it can be tested anywhere.
#[cfg_attr(not(windows), allow(dead_code))]
fn winget_candidates(winget: &Path) -> Vec<PathBuf> {
    let mut out = vec![winget.join("Links").join("ffmpeg.exe")];
    let Ok(packages) = std::fs::read_dir(winget.join("Packages")) else {
        return out;
    };
    for pkg in packages.flatten() {
        if !pkg.file_name().to_string_lossy().contains("FFmpeg") {
            continue;
        }
        if let Ok(versions) = std::fs::read_dir(pkg.path()) {
            out.extend(
                versions
                    .flatten()
                    .map(|v| v.path().join("bin").join("ffmpeg.exe")),
            );
        }
    }
    out
}

#[cfg(not(windows))]
fn platform_candidates() -> Vec<PathBuf> {
    [
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
        "/snap/bin/ffmpeg",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

// ── PNG sequence ────────────────────────────────────────────────────────────

/// Writes `frame_000000.png`, `frame_000001.png`, … into a folder.
#[derive(Debug)]
pub struct PngSequence {
    dir: PathBuf,
}

impl PngSequence {
    /// Creates (or reuses) the output folder.
    pub fn create(dir: &Path) -> AppResult<PngSequence> {
        std::fs::create_dir_all(dir).map_err(|err| write_error(dir, &err))?;
        Ok(PngSequence {
            dir: dir.to_path_buf(),
        })
    }
}

impl VideoSink for PngSequence {
    fn push(&mut self, index: u32, width: u32, height: u32, rgba: &[u8]) -> AppResult<()> {
        let path = self.dir.join(format!("frame_{index:06}.png"));
        let mut bytes = Vec::new();
        encode_png(&mut bytes, width, height, rgba).map_err(|err| {
            AppError::new(
                ErrorCode::EncoderFailed,
                format!("{}: {err}", path.display()),
            )
        })?;
        std::fs::write(&path, bytes).map_err(|err| write_error(&path, &err))
    }

    fn finish(&mut self) -> AppResult<()> {
        Ok(())
    }
}

// Fast compression: frames are big and many; disk space matters less than time.
fn encode_png(
    out: &mut Vec<u8>,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), png::EncodingError> {
    let mut enc = png::Encoder::new(out, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()
}

fn write_error(path: &Path, err: &std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::FileWriteFailed,
        format!("{}: {err}", path.display()),
    )
    .with_param("path", path.display())
}

// ── ffmpeg ──────────────────────────────────────────────────────────────────

/// Streams raw RGBA frames into an ffmpeg process.
#[derive(Debug)]
pub struct FfmpegEncoder {
    exe: PathBuf,
    settings: EncodeSettings,
    /// Input frame size, fixed by the first frame.
    input: Option<(u32, u32)>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_reader: Option<JoinHandle<()>>,
    output: PathBuf,
}

impl FfmpegEncoder {
    /// Prepares to write `output` (parent folder is created now); ffmpeg
    /// itself starts with the first frame.
    pub fn start(exe: &Path, output: &Path, s: &EncodeSettings) -> AppResult<FfmpegEncoder> {
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|err| write_error(parent, &err))?;
        }
        Ok(FfmpegEncoder {
            exe: exe.to_path_buf(),
            settings: s.clone(),
            input: None,
            child: None,
            stdin: None,
            stderr: Arc::new(Mutex::new(Vec::new())),
            stderr_reader: None,
            output: output.to_path_buf(),
        })
    }

    fn spawn(&mut self, width: u32, height: u32) -> AppResult<()> {
        let (exe, output) = (self.exe.clone(), self.output.clone());
        let mut cmd = Command::new(&exe);
        cmd.args(ffmpeg_args(&self.settings, width, height))
            .arg(&output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        hide_console_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|err| {
            AppError::new(
                ErrorCode::FfmpegNotFound,
                format!("cannot start {}: {err}", exe.display()),
            )
        })?;
        let stdin = child.stdin.take();

        // ffmpeg's stderr must be drained continuously, otherwise it can block
        // once the pipe buffer is full. Keep the last 8 KB for error messages.
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let stderr_reader = child.stderr.take().map(|mut pipe| {
            let sink = Arc::clone(&stderr);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    if let Ok(mut log) = sink.lock() {
                        log.extend_from_slice(&buf[..n]);
                        let excess = log.len().saturating_sub(8192);
                        log.drain(..excess);
                    }
                }
            })
        });
        tracing::info!(output = %output.display(), width, height, "ffmpeg started");
        self.input = Some((width, height));
        self.child = Some(child);
        self.stdin = stdin;
        self.stderr = stderr;
        self.stderr_reader = stderr_reader;
        Ok(())
    }

    fn stderr_tail(&self) -> String {
        let log = self
            .stderr
            .lock()
            .map(|l| String::from_utf8_lossy(&l).into_owned())
            .unwrap_or_default();
        log.lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn fail(&mut self, what: &str) -> AppError {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
        if let Some(t) = self.stderr_reader.take() {
            let _ = t.join();
        }
        let detail = self.stderr_tail();
        AppError::new(ErrorCode::EncoderFailed, format!("{what}: {detail}"))
            .with_param("detail", detail)
            .with_param("path", self.output.display())
    }
}

impl VideoSink for FfmpegEncoder {
    fn push(&mut self, _index: u32, width: u32, height: u32, rgba: &[u8]) -> AppResult<()> {
        match self.input {
            None => self.spawn(width, height)?,
            Some(size) if size != (width, height) => {
                return Err(self.fail(&format!(
                    "frame size changed from {}x{} to {width}x{height}",
                    size.0, size.1
                )));
            }
            Some(_) => {}
        }
        let result = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(rgba),
            None => return Err(self.fail("encoder already closed")),
        };
        result.map_err(|err| self.fail(&format!("ffmpeg stopped accepting frames ({err})")))
    }

    fn finish(&mut self) -> AppResult<()> {
        self.stdin = None; // closing stdin tells ffmpeg the video is complete
        let status = match self.child.take() {
            Some(mut child) => child.wait(),
            None => return Ok(()),
        };
        if let Some(t) = self.stderr_reader.take() {
            let _ = t.join();
        }
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(self.fail(&format!("ffmpeg exited with {s}"))),
            Err(err) => Err(self.fail(&format!("waiting for ffmpeg: {err}"))),
        }
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// ffmpeg arguments (without the output path, which comes last).
/// `width`×`height` is the size of the incoming frames.
fn ffmpeg_args(s: &EncodeSettings, width: u32, height: u32) -> Vec<String> {
    let fps = s.fps.to_string();
    let input_size = format!("{width}x{height}");
    let mut a: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        // Input: raw RGBA frames on stdin at the target frame rate.
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &input_size,
        "-framerate",
        &fps,
        "-i",
        "-",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    // Exact output size + correct RGB → YUV colour conversion (BT.709).
    a.push("-vf".into());
    a.push(format!(
        "scale={}:{}:flags=lanczos:out_color_matrix=bt709:out_range=tv,format=yuv420p",
        s.width, s.height
    ));
    let codec: &[&str] = match (s.format, s.quality) {
        (OutputFormat::Webm, q) => &[
            "-c:v",
            "libvpx-vp9",
            "-crf",
            "24",
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-deadline",
            "good",
            "-cpu-used",
            if q == Quality::Best { "1" } else { "4" },
        ],
        (_, Quality::Fast) => &[
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-movflags",
            "+faststart",
        ],
        (_, Quality::Best) => &[
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "17",
            "-movflags",
            "+faststart",
        ],
    };
    a.extend(codec.iter().map(|s| (*s).to_owned()));
    a.extend(
        [
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-color_range",
            "tv",
            "-r",
            &fps,
        ]
        .iter()
        .map(|s| (*s).to_owned()),
    );
    a
}

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(format: OutputFormat) -> EncodeSettings {
        EncodeSettings {
            width: 1080,
            height: 1920,
            fps: 60,
            format,
            quality: Quality::Fast,
        }
    }

    #[test]
    fn mp4_args_scale_tag_colours_and_use_h264() {
        let args = ffmpeg_args(&settings(OutputFormat::Mp4), 1080, 1920).join(" ");
        assert!(args.contains("-f rawvideo -pix_fmt rgba -s 1080x1920 -framerate 60 -i -"));
        assert!(args.contains("scale=1080:1920"));
        assert!(args.contains("out_color_matrix=bt709"));
        assert!(args.contains("libx264 -preset veryfast"));
        assert!(args.contains("-colorspace bt709"));
    }

    #[test]
    fn webm_uses_vp9() {
        assert!(
            ffmpeg_args(&settings(OutputFormat::Webm), 1080, 1920)
                .join(" ")
                .contains("libvpx-vp9")
        );
    }

    #[test]
    fn png_sequence_writes_numbered_files_without_ffmpeg() {
        let dir = std::env::temp_dir().join(format!("sr-encoder-test-{}", std::process::id()));
        let mut sink = open_sink(&dir, &settings(OutputFormat::Png), None).unwrap();
        let red = [255u8, 0, 0, 255].repeat(6);
        sink.push(0, 3, 2, &red).unwrap();
        sink.push(12, 3, 2, &red).unwrap();
        sink.finish().unwrap();
        let png = std::fs::read(dir.join("frame_000012.png")).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert_eq!(
            u32::from_be_bytes([png[16], png[17], png[18], png[19]]),
            3,
            "width"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_frames_become_a_video_when_ffmpeg_is_installed() {
        let Ok(exe) = find_ffmpeg(None) else {
            eprintln!("skipped: ffmpeg not installed");
            return;
        };
        let out = std::env::temp_dir().join(format!("sr-encoder-video-{}.mp4", std::process::id()));
        let mut s = settings(OutputFormat::Mp4);
        (s.width, s.height, s.fps) = (64, 32, 10);
        let mut enc = FfmpegEncoder::start(&exe, &out, &s).unwrap();
        for i in 0..5u8 {
            let frame = [i * 50, 100, 200, 255].repeat(64 * 32);
            enc.push(u32::from(i), 64, 32, &frame).unwrap();
        }
        let err = enc.push(5, 32, 32, &[0; 32 * 32 * 4]).unwrap_err();
        assert!(
            err.message.contains("frame size changed"),
            "{}",
            err.message
        );

        let mut enc = FfmpegEncoder::start(&exe, &out, &s).unwrap();
        for i in 0..5u32 {
            enc.push(i, 64, 32, &[128; 64 * 32 * 4]).unwrap();
        }
        enc.finish().unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 100);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn finds_ffmpeg_unpacked_by_winget() {
        let winget = std::env::temp_dir().join(format!("sr-winget-test-{}", std::process::id()));
        let bin = winget
            .join("Packages")
            .join("Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe");
        let exe = bin
            .join("ffmpeg-7.1-full_build")
            .join("bin")
            .join("ffmpeg.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        std::fs::create_dir_all(winget.join("Packages").join("Other.Tool")).unwrap();

        let found = winget_candidates(&winget);
        assert_eq!(found[0], winget.join("Links").join("ffmpeg.exe"));
        assert!(found.contains(&exe), "{found:?}");
        assert_eq!(found.len(), 2, "unrelated packages are ignored");
        let _ = std::fs::remove_dir_all(&winget);
    }

    #[test]
    fn missing_explicit_ffmpeg_is_reported() {
        let err = find_ffmpeg(Some(Path::new("/no/such/ffmpeg"))).unwrap_err();
        assert_eq!(err.code, ErrorCode::FfmpegNotFound);
    }
}
