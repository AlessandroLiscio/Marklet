//! Hand-rolled argument parsing and headless dispatch.
//!
//! Marklet ships `#![windows_subsystem = "windows"]` in release, so the
//! binary has **no console** — `println!` before `AttachConsole` prints into
//! the void. Every path through this module must finish and let `main` exit
//! **before** `tauri::Builder::build()` runs: no window, no WebView2 init.
//! That early exit is the entire reason a GUI binary carries a CLI at all.
//! See `docs/architecture.md#the-cli-console-trap` and
//! `.claude/skills/win-integration/SKILL.md`.
//!
//! No `clap`, no `tauri-plugin-cli` — both are denied by name in
//! `src-tauri/deny.toml` (+350-450 KB for argument parsing this file does in
//! well under it). **This module must not import `tauri`.**

use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::{export, platform, render};

// ---------------------------------------------------------------------
// Parsed shape
// ---------------------------------------------------------------------

/// What `main` does with a parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// Runs to completion inside `main` and the process exits. Never touches
    /// a window or WebView2.
    Headless(HeadlessJob),
    /// Falls through to `marklet::run()`.
    Window(WindowJob),
}

/// A CLI path that must exit before `tauri::Builder::build()`.
#[derive(Debug, PartialEq, Eq)]
pub enum HeadlessJob {
    Help,
    Version,
    /// `--install [--silent]`.
    Install {
        silent: bool,
    },
    /// `--uninstall [--silent]`.
    Uninstall {
        silent: bool,
    },
    /// `--unbind [--silent]`.
    Unbind {
        silent: bool,
    },
    /// `MD_HTML=1 <file>`: render to standalone HTML, on stdout unless
    /// `MD_HTML_OUTPUT` names a file.
    RenderHtml {
        file: PathBuf,
        output: Option<PathBuf>,
    },
    /// `--benchmark <file>`: render once, print `boot-ms=<n>` to stderr. The
    /// release workflow's cold-start gate greps exactly that string — the
    /// format is fixed, do not change it without updating the workflow too.
    Benchmark {
        file: PathBuf,
    },
}

/// A CLI path that needs a window — everything below is data for a later
/// phase to consume; this module only parses and validates it.
#[derive(Debug, PartialEq, Eq)]
pub struct WindowJob {
    /// File to open, if any: `marklet file.md`, or a shell double-click.
    pub file: Option<PathBuf>,
    /// `--settings`: open the settings window.
    pub settings: bool,
    /// `PORT` — dev server port for live reload.
    pub port: Option<u16>,
    /// `MD_EDITOR` — external editor path for F4.
    pub editor: Option<String>,
}

/// What went wrong parsing the command line.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownFlag(String),
    MissingArgument(&'static str),
    TooManyArguments(String),
    InvalidPort(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            ParseError::MissingArgument(msg) => write!(f, "{msg}"),
            ParseError::TooManyArguments(arg) => write!(f, "unexpected extra argument: {arg}"),
            ParseError::InvalidPort(value) => write!(f, "invalid PORT value: {value}"),
        }
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------
// Environment — a small seam so tests do not have to mutate process env
// ---------------------------------------------------------------------

/// The environment variables the parser reads. A struct (rather than calling
/// `std::env::var` inline) so unit tests can inject values instead of
/// racing real process env vars across parallel test threads.
#[derive(Debug, Default, Clone)]
pub struct Env {
    pub md_html: Option<String>,
    pub md_html_output: Option<String>,
    pub port: Option<String>,
    pub md_editor: Option<String>,
}

impl Env {
    pub fn from_process() -> Self {
        Env {
            md_html: env::var("MD_HTML").ok(),
            md_html_output: env::var("MD_HTML_OUTPUT").ok(),
            port: env::var("PORT").ok(),
            md_editor: env::var("MD_EDITOR").ok(),
        }
    }
}

// ---------------------------------------------------------------------
// The parser — roughly 120 lines, replacing what `clap` would cost 350-450 KB
// to do. Flags: <file.md>, --install, --uninstall, --unbind, --settings,
// --help, --version, --benchmark, --silent. Env: PORT, MD_HTML,
// MD_HTML_OUTPUT, MD_EDITOR.
// ---------------------------------------------------------------------

/// Parses `args` (already stripped of `argv[0]`) against the real process
/// environment.
pub fn parse(args: &[String]) -> Result<Mode, ParseError> {
    parse_with_env(args, &Env::from_process())
}

/// The testable core: `args` and `env` are both plain data.
pub fn parse_with_env(args: &[String], env: &Env) -> Result<Mode, ParseError> {
    let mut file: Option<String> = None;
    let mut silent = false;
    let mut settings = false;
    let mut saw_help = false;
    let mut saw_version = false;
    let mut saw_install = false;
    let mut saw_uninstall = false;
    let mut saw_unbind = false;
    let mut benchmark_file: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" => saw_help = true,
            "--version" => saw_version = true,
            "--install" => saw_install = true,
            "--uninstall" => saw_uninstall = true,
            "--unbind" => saw_unbind = true,
            "--settings" => settings = true,
            "--silent" => silent = true,
            "--benchmark" => {
                i += 1;
                match args.get(i) {
                    Some(next) if !next.starts_with("--") => benchmark_file = Some(next.clone()),
                    _ => {
                        return Err(ParseError::MissingArgument(
                            "--benchmark requires a markdown file argument",
                        ))
                    }
                }
            }
            flag if flag.starts_with("--") => {
                return Err(ParseError::UnknownFlag(flag.to_string()));
            }
            positional => {
                if file.is_some() {
                    return Err(ParseError::TooManyArguments(positional.to_string()));
                }
                file = Some(positional.to_string());
            }
        }
        i += 1;
    }

    if saw_help {
        return Ok(Mode::Headless(HeadlessJob::Help));
    }
    if saw_version {
        return Ok(Mode::Headless(HeadlessJob::Version));
    }
    if saw_install {
        return Ok(Mode::Headless(HeadlessJob::Install { silent }));
    }
    if saw_uninstall {
        return Ok(Mode::Headless(HeadlessJob::Uninstall { silent }));
    }
    if saw_unbind {
        return Ok(Mode::Headless(HeadlessJob::Unbind { silent }));
    }
    if let Some(bench_file) = benchmark_file {
        return Ok(Mode::Headless(HeadlessJob::Benchmark {
            file: PathBuf::from(bench_file),
        }));
    }

    if matches!(env.md_html.as_deref(), Some("1")) {
        let file = file.ok_or(ParseError::MissingArgument(
            "MD_HTML=1 requires a markdown file argument",
        ))?;
        return Ok(Mode::Headless(HeadlessJob::RenderHtml {
            file: PathBuf::from(file),
            output: env.md_html_output.clone().map(PathBuf::from),
        }));
    }

    let port = match env.port.as_deref() {
        Some(value) => Some(
            value
                .parse::<u16>()
                .map_err(|_| ParseError::InvalidPort(value.to_string()))?,
        ),
        None => None,
    };

    Ok(Mode::Window(WindowJob {
        file: file.map(PathBuf::from),
        settings,
        port,
        editor: env.md_editor.clone(),
    }))
}

// ---------------------------------------------------------------------
// Windows console shim
// ---------------------------------------------------------------------

#[cfg(windows)]
mod console {
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_OUTPUT_HANDLE,
    };

    /// Attaches to the console of whatever launched us (a terminal), if any.
    ///
    /// Marklet is built `#![windows_subsystem = "windows"]` in release, so it
    /// starts with no console and no valid stdio handles — `println!` would
    /// write into the void. Per MSDN, once `AttachConsole` succeeds it
    /// updates the handles `GetStdHandle` returns for this process as long
    /// as they were not already redirected — which is exactly "launched
    /// from a terminal" and "output piped to a file", the two cases this
    /// CLI cares about. We re-assert them with `SetStdHandle` explicitly
    /// rather than relying on that alone.
    ///
    /// Returns `false` when there is no parent console to attach to — the
    /// process was launched by double-clicking in Explorer. Callers fall
    /// back to a message box for `--help` and `--version` in that case.
    pub fn attach() -> bool {
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                return false;
            }
            if let Ok(h) = GetStdHandle(STD_OUTPUT_HANDLE) {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, h);
            }
            if let Ok(h) = GetStdHandle(STD_ERROR_HANDLE) {
                let _ = SetStdHandle(STD_ERROR_HANDLE, h);
            }
        }
        true
    }
}

#[cfg(not(windows))]
mod console {
    /// Every non-Windows target already runs with an inherited console/tty —
    /// there is nothing to attach.
    pub fn attach() -> bool {
        true
    }
}

/// Attaches the console (Windows) or is a no-op (everything else). Call once,
/// as the very first thing `main` does, before any output.
pub fn attach_console() -> bool {
    console::attach()
}

#[cfg(windows)]
fn message_box(text: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let text = HSTRING::from(text);
    let title = HSTRING::from("Marklet");
    unsafe {
        let _ = MessageBoxW(None, &text, &title, MB_OK | MB_ICONINFORMATION);
    }
}

#[cfg(not(windows))]
fn message_box(_text: &str) {
    // Unreachable in practice: `attach_console` never returns `false` off
    // Windows, so nothing calls this fallback there.
}

// ---------------------------------------------------------------------
// Headless execution
// ---------------------------------------------------------------------

/// Runs a [`HeadlessJob`] to completion and returns the process exit code.
/// `console_ok` is `attach_console()`'s result — when it is `false` on
/// Windows, `--help`/`--version` fall back to a message box instead of
/// printing into a console that does not exist.
pub fn run_headless(job: HeadlessJob, console_ok: bool) -> i32 {
    match job {
        HeadlessJob::Help => {
            show_or_box(&help_text(), console_ok);
            0
        }
        HeadlessJob::Version => {
            show_or_box(&version_text(), console_ok);
            0
        }
        HeadlessJob::Install { silent } => run_platform("--install", platform::install(silent)),
        HeadlessJob::Uninstall { silent } => {
            run_platform("--uninstall", platform::uninstall(silent))
        }
        HeadlessJob::Unbind { silent } => run_platform("--unbind", platform::unbind(silent)),
        HeadlessJob::RenderHtml { file, output } => cmd_render_html(&file, output.as_deref()),
        HeadlessJob::Benchmark { file } => cmd_benchmark(&file),
    }
}

/// Prints error + a usage reminder for a [`ParseError`]. Call after
/// [`attach_console`].
pub fn report_parse_error(err: &ParseError, console_ok: bool) {
    let text = format!("marklet: {err}\n\n{}", help_text());
    show_or_box(&text, console_ok);
}

fn show_or_box(text: &str, console_ok: bool) {
    if console_ok {
        print!("{text}");
        let _ = std::io::stdout().flush();
    } else {
        message_box(text);
    }
}

fn run_platform(flag: &str, result: std::io::Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("marklet: {flag} failed: {e}");
            1
        }
    }
}

fn cmd_render_html(file: &Path, output: Option<&Path>) -> i32 {
    match export::html::export_file(file) {
        Ok(html) => match output {
            Some(path) => match std::fs::write(path, &html) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("marklet: could not write {}: {e}", path.display());
                    1
                }
            },
            None => {
                print!("{html}");
                let _ = std::io::stdout().flush();
                0
            }
        },
        Err(e) => {
            eprintln!("marklet: {e}");
            1
        }
    }
}

fn cmd_benchmark(file: &Path) -> i32 {
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("marklet: could not read {}: {e}", file.display());
            return 1;
        }
    };

    let start = Instant::now();
    let _doc = render::render(&bytes, render::RenderOpts::default());
    let elapsed_ms = start.elapsed().as_millis();

    // The release workflow's cold-start gate greps exactly this string.
    eprintln!("boot-ms={elapsed_ms}");
    0
}

fn version_text() -> String {
    format!("marklet {}\n", env!("CARGO_PKG_VERSION"))
}

fn help_text() -> String {
    format!(
        "marklet {version} — lightweight, open-source Markdown viewer\n\
         \n\
         USAGE:\n\
         \x20   marklet [FILE] [OPTIONS]\n\
         \n\
         ARGS:\n\
         \x20   <FILE>               Markdown file to open\n\
         \n\
         OPTIONS:\n\
         \x20   --install            Register the .md file association (HKCU)\n\
         \x20   --uninstall          Remove the file association and context menu\n\
         \x20   --unbind             Remove the file association only\n\
         \x20   --settings           Open the settings window\n\
         \x20   --benchmark <FILE>   Render FILE once, print boot-ms=<n> to stderr, exit\n\
         \x20   --silent             Suppress status output (used by the installer hooks)\n\
         \x20   --help               Show this help\n\
         \x20   --version            Show the version\n\
         \n\
         ENVIRONMENT:\n\
         \x20   PORT                 Dev server port for live reload\n\
         \x20   MD_HTML=1            Render FILE to standalone HTML on stdout, then exit\n\
         \x20   MD_HTML_OUTPUT       Write MD_HTML output to this path instead of stdout\n\
         \x20   MD_EDITOR            External editor launched by F4\n",
        version = env!("CARGO_PKG_VERSION"),
    )
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn empty_env() -> Env {
        Env::default()
    }

    #[test]
    fn no_arguments_opens_a_window_with_nothing_set() {
        let mode = parse_with_env(&args(&[]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: None,
                settings: false,
                port: None,
                editor: None,
            })
        );
    }

    #[test]
    fn help_flag() {
        let mode = parse_with_env(&args(&["--help"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Help));
    }

    #[test]
    fn version_flag() {
        let mode = parse_with_env(&args(&["--version"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Version));
    }

    #[test]
    fn install_flag() {
        let mode = parse_with_env(&args(&["--install"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Install { silent: false }));
    }

    #[test]
    fn install_with_silent_combo() {
        let mode = parse_with_env(&args(&["--install", "--silent"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Install { silent: true }));

        // Order should not matter.
        let mode = parse_with_env(&args(&["--silent", "--install"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Install { silent: true }));
    }

    #[test]
    fn uninstall_flag() {
        let mode = parse_with_env(&args(&["--uninstall"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Headless(HeadlessJob::Uninstall { silent: false })
        );
    }

    #[test]
    fn unbind_flag() {
        let mode = parse_with_env(&args(&["--unbind"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Unbind { silent: false }));
    }

    #[test]
    fn settings_flag_opens_a_window() {
        let mode = parse_with_env(&args(&["--settings"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: None,
                settings: true,
                port: None,
                editor: None,
            })
        );
    }

    #[test]
    fn silent_alone_is_not_an_error() {
        let mode = parse_with_env(&args(&["--silent"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: None,
                settings: false,
                port: None,
                editor: None,
            })
        );
    }

    #[test]
    fn unknown_flag_is_an_error() {
        let err = parse_with_env(&args(&["--wat"]), &empty_env()).unwrap_err();
        assert_eq!(err, ParseError::UnknownFlag("--wat".to_string()));
    }

    #[test]
    fn plain_file_argument_opens_a_window() {
        let mode = parse_with_env(&args(&["notes.md"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: Some(PathBuf::from("notes.md")),
                settings: false,
                port: None,
                editor: None,
            })
        );
    }

    #[test]
    fn two_positional_arguments_is_an_error() {
        let err = parse_with_env(&args(&["a.md", "b.md"]), &empty_env()).unwrap_err();
        assert_eq!(err, ParseError::TooManyArguments("b.md".to_string()));
    }

    #[test]
    fn benchmark_with_file() {
        let mode = parse_with_env(&args(&["--benchmark", "doc.md"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Headless(HeadlessJob::Benchmark {
                file: PathBuf::from("doc.md"),
            })
        );
    }

    #[test]
    fn benchmark_missing_file_is_an_error() {
        let err = parse_with_env(&args(&["--benchmark"]), &empty_env()).unwrap_err();
        assert_eq!(
            err,
            ParseError::MissingArgument("--benchmark requires a markdown file argument")
        );
    }

    #[test]
    fn benchmark_followed_by_a_flag_is_a_missing_file_error() {
        let err = parse_with_env(&args(&["--benchmark", "--silent"]), &empty_env()).unwrap_err();
        assert_eq!(
            err,
            ParseError::MissingArgument("--benchmark requires a markdown file argument")
        );
    }

    #[test]
    fn md_html_env_requires_a_file_argument() {
        let env = Env {
            md_html: Some("1".to_string()),
            ..Env::default()
        };
        let err = parse_with_env(&args(&[]), &env).unwrap_err();
        assert_eq!(
            err,
            ParseError::MissingArgument("MD_HTML=1 requires a markdown file argument")
        );
    }

    #[test]
    fn md_html_env_with_file_renders_to_stdout() {
        let env = Env {
            md_html: Some("1".to_string()),
            ..Env::default()
        };
        let mode = parse_with_env(&args(&["doc.md"]), &env).unwrap();
        assert_eq!(
            mode,
            Mode::Headless(HeadlessJob::RenderHtml {
                file: PathBuf::from("doc.md"),
                output: None,
            })
        );
    }

    #[test]
    fn md_html_output_env_redirects_to_a_file() {
        let env = Env {
            md_html: Some("1".to_string()),
            md_html_output: Some("out.html".to_string()),
            ..Env::default()
        };
        let mode = parse_with_env(&args(&["doc.md"]), &env).unwrap();
        assert_eq!(
            mode,
            Mode::Headless(HeadlessJob::RenderHtml {
                file: PathBuf::from("doc.md"),
                output: Some(PathBuf::from("out.html")),
            })
        );
    }

    #[test]
    fn md_html_not_equal_to_one_is_ignored() {
        let env = Env {
            md_html: Some("true".to_string()),
            ..Env::default()
        };
        let mode = parse_with_env(&args(&["doc.md"]), &env).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: Some(PathBuf::from("doc.md")),
                settings: false,
                port: None,
                editor: None,
            })
        );
    }

    #[test]
    fn port_env_is_parsed() {
        let env = Env {
            port: Some("4173".to_string()),
            ..Env::default()
        };
        let mode = parse_with_env(&args(&[]), &env).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: None,
                settings: false,
                port: Some(4173),
                editor: None,
            })
        );
    }

    #[test]
    fn invalid_port_env_is_an_error() {
        let env = Env {
            port: Some("not-a-port".to_string()),
            ..Env::default()
        };
        let err = parse_with_env(&args(&[]), &env).unwrap_err();
        assert_eq!(err, ParseError::InvalidPort("not-a-port".to_string()));
    }

    #[test]
    fn editor_env_is_carried_through() {
        let env = Env {
            md_editor: Some("/usr/bin/vim".to_string()),
            ..Env::default()
        };
        let mode = parse_with_env(&args(&[]), &env).unwrap();
        assert_eq!(
            mode,
            Mode::Window(WindowJob {
                file: None,
                settings: false,
                port: None,
                editor: Some("/usr/bin/vim".to_string()),
            })
        );
    }

    #[test]
    fn help_takes_precedence_over_other_headless_flags() {
        let mode = parse_with_env(&args(&["--install", "--help"]), &empty_env()).unwrap();
        assert_eq!(mode, Mode::Headless(HeadlessJob::Help));
    }

    #[test]
    fn benchmark_takes_precedence_over_a_plain_file_and_md_html() {
        // The file after --benchmark is consumed by it; a second positional
        // is a separate, extra argument and is rejected the normal way.
        let mode = parse_with_env(&args(&["--benchmark", "doc.md"]), &empty_env()).unwrap();
        assert_eq!(
            mode,
            Mode::Headless(HeadlessJob::Benchmark {
                file: PathBuf::from("doc.md"),
            })
        );
    }
}
