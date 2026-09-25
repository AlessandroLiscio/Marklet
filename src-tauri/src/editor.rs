//! Launching the user's own editor — the F4 / `Ctrl+E` path.
//!
//! **This lives in Rust rather than in the frontend on purpose.** Spawning a
//! process is the widest privilege this application has, and the rule the whole
//! IPC surface is built on (`.claude/skills/tauri-ipc/SKILL.md`) is that the
//! webview describes *intent* and Rust decides what actually happens. A command
//! shaped `reveal_in_editor(program, args)` would have let anything that got
//! script execution inside the webview run an arbitrary executable with
//! arbitrary arguments; `reveal_in_editor(path, line, column)` cannot. The
//! frontend therefore names no program at all, and this module is the only
//! place a program name is ever chosen.
//!
//! No `tauri` import: this is plain string and process work, unit-testable
//! without a window, and [`crate::ipc`] wraps it like every other subsystem.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// One process to try: a program and its fully-formed arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
}

/// The chain tried when `MD_EDITOR` is unset, best first.
///
/// VS Code first because it is what most people who install a Markdown viewer
/// already have; `notepad` last because on Windows it is the one editor
/// guaranteed to exist. On Linux the last two simply fail to spawn and the
/// walk reports that nothing started, which is the honest answer.
pub const DEFAULT_EDITORS: &[&str] = &["code", "subl", "notepad++", "gedit", "notepad"];

/// How each known editor is told to open at a position, keyed by
/// [`program_key`].
///
/// `notepad` has no line syntax at all, which is exactly why it is last.
fn placement(key: &str, path: &str, line: usize, column: usize) -> Option<Vec<String>> {
    let v = |parts: &[String]| Some(parts.to_vec());
    let at = format!("{path}:{line}:{column}");
    match key {
        "code" | "code-insiders" | "codium" | "cursor" => v(&["-g".into(), at]),
        "subl" | "sublime_text" | "micro" | "helix" | "hx" => v(&[at]),
        "notepad++" => v(&[format!("-n{line}"), format!("-c{column}"), path.into()]),
        "vim" | "nvim" | "gvim" => v(&[format!("+{line}"), path.into()]),
        "nano" => v(&[format!("+{line},{column}"), path.into()]),
        "emacs" => v(&[format!("+{line}:{column}"), path.into()]),
        "gedit" | "kate" => v(&[format!("+{line}:{column}"), path.into()]),
        "idea" => v(&[
            "--line".into(),
            line.to_string(),
            "--column".into(),
            column.to_string(),
            path.into(),
        ]),
        "notepad" => v(&[path.into()]),
        _ => None,
    }
}

/// Splits an `MD_EDITOR` value into program and arguments, honouring double
/// quotes.
///
/// Quotes are not optional politeness on Windows: the default VS Code install
/// path is `C:\Program Files\Microsoft VS Code\bin\code.cmd`, and splitting
/// that on spaces produces three programs that do not exist.
pub fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;

    for ch in command.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ' ' | '\t' if !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                }
                current.clear();
                started = false;
            }
            _ => {
                current.push(ch);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

/// The lookup key for a program path: basename, lowercased, extension dropped.
pub fn program_key(program: &str) -> String {
    let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let lower = base.to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = lower.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    lower
}

/// Whether a configured command spells out its own placement with `%f` (file),
/// `%l` (line) and `%c` (column).
///
/// The escape hatch for an editor this module has never heard of:
/// `MD_EDITOR='myeditor --goto %f@%l'` beats any guess we could make, and it is
/// why an unknown editor is not a dead end.
pub fn has_placeholders(tokens: &[String]) -> bool {
    tokens
        .iter()
        .any(|t| t.contains("%f") || t.contains("%l") || t.contains("%c"))
}

fn substitute(tokens: &[String], path: &str, line: usize, column: usize) -> Vec<String> {
    tokens
        .iter()
        .map(|t| {
            t.replace("%f", path)
                .replace("%l", &line.to_string())
                .replace("%c", &column.to_string())
        })
        .collect()
}

/// Builds the launch for one editor command.
///
/// Whatever the user put after the program name in `MD_EDITOR` is kept and
/// placed *before* the positional arguments, because that is where flags go for
/// every editor in [`placement`].
pub fn launch_for(command: &str, path: &str, line: usize, column: usize) -> Launch {
    let tokens = tokenize(command);
    let program = tokens
        .first()
        .cloned()
        .unwrap_or_else(|| command.to_string());

    if has_placeholders(&tokens) {
        let done = substitute(&tokens, path, line, column);
        return Launch {
            program: done.first().cloned().unwrap_or(program),
            args: done.into_iter().skip(1).collect(),
        };
    }

    let extra: Vec<String> = tokens.into_iter().skip(1).collect();
    let positional =
        placement(&program_key(&program), path, line, column).unwrap_or_else(|| vec![path.into()]);

    Launch {
        args: extra.into_iter().chain(positional).collect(),
        program,
    }
}

/// Every launch worth trying, best first.
///
/// A configured `MD_EDITOR` produces exactly **one** candidate. Falling back to
/// VS Code when somebody's chosen editor fails to start would hide the failure
/// behind a different program opening — the user asked for a specific editor,
/// and "it did not start" is the useful answer.
pub fn candidates(configured: Option<&str>, path: &str, line: usize, column: usize) -> Vec<Launch> {
    match configured.map(str::trim).filter(|c| !c.is_empty()) {
        Some(command) => vec![launch_for(command, path, line, column)],
        None => DEFAULT_EDITORS
            .iter()
            .map(|name| launch_for(name, path, line, column))
            .collect(),
    }
}

/// Spawns the first candidate that starts and returns its program name.
///
/// Detached, with the three standard streams dropped: an editor that inherits
/// this process's stdio keeps a console attached on Windows (see `cli.rs`'s
/// `AttachConsole` note) and can block on a pipe nobody is reading.
///
/// "Started" means the spawn succeeded, not that the editor did anything
/// useful — nothing here waits for it, because a modal editor like `vim` would
/// never return and a GUI editor returns immediately regardless.
pub fn reveal(candidates: &[Launch]) -> io::Result<String> {
    let mut last: Option<io::Error> = None;

    for candidate in candidates {
        let spawned = Command::new(&candidate.program)
            .args(&candidate.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match spawned {
            Ok(_) => return Ok(candidate.program.clone()),
            Err(e) => last = Some(e),
        }
    }

    Err(last.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no editor to try")))
}

/// The candidates for `path`, ready to hand to [`reveal`].
pub fn candidates_for(
    configured: Option<&str>,
    path: &Path,
    line: usize,
    column: usize,
) -> Vec<Launch> {
    candidates(configured, &path.display().to_string(), line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(launch: &Launch) -> Vec<&str> {
        launch.args.iter().map(String::as_str).collect()
    }

    #[test]
    fn quoted_program_paths_survive_tokenizing() {
        assert_eq!(
            tokenize(r#""C:\Program Files\Microsoft VS Code\bin\code.cmd" -n"#),
            vec![r"C:\Program Files\Microsoft VS Code\bin\code.cmd", "-n"]
        );
        assert_eq!(tokenize("  vim   "), vec!["vim"]);
        assert_eq!(tokenize(""), Vec::<String>::new());
    }

    #[test]
    fn the_lookup_key_ignores_directory_case_and_extension() {
        assert_eq!(program_key(r"C:\bin\Code.CMD"), "code");
        assert_eq!(program_key("/usr/bin/CODE"), "code");
        assert_eq!(program_key("notepad++.exe"), "notepad++");
        assert_eq!(program_key("hx"), "hx");
    }

    #[test]
    fn each_editor_gets_its_own_goto_syntax() {
        assert_eq!(
            args(&launch_for("code", "/n.md", 12, 3)),
            ["-g", "/n.md:12:3"]
        );
        assert_eq!(args(&launch_for("subl", "/n.md", 12, 3)), ["/n.md:12:3"]);
        assert_eq!(
            args(&launch_for("notepad++", "/n.md", 12, 3)),
            ["-n12", "-c3", "/n.md"]
        );
        assert_eq!(args(&launch_for("vim", "/n.md", 12, 3)), ["+12", "/n.md"]);
        assert_eq!(
            args(&launch_for("nano", "/n.md", 12, 3)),
            ["+12,3", "/n.md"]
        );
        assert_eq!(args(&launch_for("notepad", "/n.md", 12, 3)), ["/n.md"]);
    }

    #[test]
    fn an_unknown_editor_still_gets_the_file() {
        let launch = launch_for("myeditor", "/n.md", 12, 3);
        assert_eq!(launch.program, "myeditor");
        assert_eq!(args(&launch), ["/n.md"]);
    }

    #[test]
    fn placeholders_beat_the_table() {
        let launch = launch_for("myeditor --goto %f@%l:%c", "/n.md", 12, 3);
        assert_eq!(launch.program, "myeditor");
        assert_eq!(args(&launch), ["--goto", "/n.md@12:3"]);
    }

    #[test]
    fn flags_from_md_editor_come_before_the_positional_arguments() {
        let launch = launch_for("code --wait", "/n.md", 4, 1);
        assert_eq!(args(&launch), ["--wait", "-g", "/n.md:4:1"]);
    }

    #[test]
    fn a_configured_editor_is_the_only_candidate() {
        let only = candidates(Some("vim"), "/n.md", 1, 1);
        assert_eq!(
            only.len(),
            1,
            "a chosen editor must not fall back to another"
        );
        assert_eq!(only[0].program, "vim");
    }

    #[test]
    fn a_blank_setting_is_the_same_as_none() {
        let blank = candidates(Some("   "), "/n.md", 1, 1);
        assert_eq!(blank.len(), DEFAULT_EDITORS.len());
        assert_eq!(blank[0].program, "code");
    }

    #[test]
    fn reveal_reports_failure_rather_than_pretending() {
        let nothing = [Launch {
            program: "marklet-no-such-editor".into(),
            args: vec![],
        }];
        assert!(reveal(&nothing).is_err());
    }
}
