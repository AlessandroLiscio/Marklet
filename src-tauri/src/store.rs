//! Persistence: two JSON files in the app data directory, nothing heavier.
//!
//! `settings.json` holds the reading preferences every control in
//! `src/lib/settings/**` writes through to. `state.json` holds one reading
//! position per file the user has scrolled in, capped so years of vault use
//! cannot grow it without bound — a store that grows forever is a store that
//! eventually costs a startup.
//!
//! Rejected: `rusqlite` bundled (+1.5 MB) — two JSON files are enough for data
//! this small and this rarely written concurrently. See
//! `.claude/skills/size-budget/SKILL.md`'s "already-rejected alternatives".
//!
//! **Not `#[tauri::command]`.** Every function here takes a plain `&Path` for
//! the app data directory rather than resolving it itself, so this module has
//! no `tauri` type in its signatures and is testable with a temp directory —
//! same shape as `render::` and `protocol::AssetRoot`. The caller (main
//! thread, wired at wave close) resolves the directory once via
//! `app.path().app_data_dir()` and wraps these in commands; see
//! `.claude/skills/tauri-ipc/SKILL.md`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Reading positions are capped at this many entries. Past the cap, the
/// least-recently-*scrolled* entry is evicted — not the oldest-inserted one —
/// so a file reopened every day for a year outlives one opened once and
/// abandoned.
pub const MAX_POSITIONS: usize = 500;

/// `light` / `dark` win in both directions over the OS; `system` follows
/// `prefers-color-scheme`. Mirrors tokens.css's three-state theme rule and
/// `src/lib/ipc.ts`'s `Theme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Light,
    Dark,
    System,
}

/// Reading-font pairing. **Full edition only** in the settings panel —
/// `src/lib/settings/**` gates the control behind
/// `__MARKLET_EDITION__ === 'full'` at compile time, per `docs/editions.md`.
/// The field still exists in lite's `Settings` so a vault opened in both
/// editions round-trips the same `settings.json` without lite clobbering a
/// choice full made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Typeface {
    /// The platform's own UI sans, for reading as well as for chrome. The
    /// default, and the only one lite can offer: it costs no bytes because
    /// the fonts are already on the machine.
    System,
    Editorial,
    Literary,
    Technical,
}

/// Accent-hue preset. **Full edition only** in the UI, same carry-over
/// reasoning as [`Typeface`]. `Default` means "use `accent_hue` as-is";
/// the four named palettes override the hue via `[data-palette]` in
/// `src/styles/full/palettes.css` and take precedence over `accent_hue`
/// while selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Palette {
    Default,
    Teal,
    Amber,
    Forest,
    Violet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    Normal,
    Spacious,
}

/// Every control `src/lib/settings/**` exposes. Adding a control means adding
/// its field here, in the frontend's mirrored `Settings` type in
/// `src/lib/ipc.ts`, and its row in `docs/editions.md` if it differs by
/// edition.
/// `#[serde(default)]` on every field, not just on the struct.
///
/// A settings file is written by one version and read by the next. Without
/// this, adding or removing a single field makes the whole document fail to
/// parse, `read_json_or_default` hands back `Settings::default()`, and every
/// preference the user had set is silently gone — which is exactly what
/// happened to `motion` and `font_size`. Per-field defaults mean an old file
/// keeps the values it does have and picks up defaults only for what it is
/// missing.
///
/// **Field names cross the IPC boundary as they are spelled here.** They were
/// `kebab-case` for one release, which renamed `accent_hue` to `accent-hue` on
/// the wire; `src/lib/ipc.ts` reads `accent_hue`, so the hue arrived at the
/// webview as `undefined`, `--accent-hue` was set to the string "undefined",
/// and every `oklch()` derived from it became invalid — accent backgrounds
/// went transparent, range inputs fell back to the browser's blue, and the
/// panel showed "Accent hue — °". The alias below keeps a settings file
/// written by that release readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Degrees, 0-360. Ignored while `palette` is anything but `Default` —
    /// see that variant's doc comment.
    #[serde(alias = "accent-hue")]
    pub accent_hue: u16,
    pub typeface: Typeface,
    pub palette: Palette,
    pub density: Density,
    /// Column width in `ch`. Clamped to 48-100 on write — see
    /// [`Settings::clamp`] — because a value that reaches this store through
    /// anything other than the slider (a hand-edited `settings.json`, a
    /// future import) must not be able to make the reading column
    /// unreadably narrow or wide.
    pub measure: u8,
    /// Document zoom, as a percentage. Clamped to 50-250 on write for the same
    /// reason as [`Settings::measure`]: a value arriving from anywhere but the
    /// slider must not be able to make the document unreadable.
    ///
    /// Scales the **document column only**, through `#doc { zoom: … }` — not
    /// the root font size, which is what it was for one release and which took
    /// the sidebar and the settings panel with it. Zoom belongs to the thing
    /// being read.
    pub zoom: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            // tokens.css's own default. 70 deg is Material Amber 700
            // (#FFA000) converted to oklch — the hue, not the colour: the
            // lightness still comes from the accent generator, so it clears AA
            // in both themes the same way any other hue does. Amber 500
            // (#FFC107) is hue 86, a yellow that the generator has to darken
            // so far to reach AA that it stops reading as amber at all.
            // Keep in sync with tokens.css by eye; there is no build-time link.
            accent_hue: 70,
            typeface: Typeface::System,
            palette: Palette::Default,
            density: Density::Normal,
            // 100ch, the widest the slider offers. A narrower measure is the
            // classic typographic advice and it is the right default for a
            // book; this is a window someone has already sized, and leaving
            // half of it empty reads as a bug rather than as restraint.
            measure: 100,
            zoom: 100,
        }
    }
}

impl Settings {
    /// Clamps `measure` into the 48-100ch range the settings slider itself
    /// already limits. Called on every read and write so a value that
    /// reached the file some other way cannot make the reading column
    /// unreadably narrow or absurdly wide.
    pub fn clamp(mut self) -> Self {
        self.measure = self.measure.clamp(48, 100);
        self.zoom = self.zoom.clamp(50, 250);
        self
    }
}

/// One file's reading position: how far the user scrolled, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingPosition {
    pub line: usize,
    /// Milliseconds since the Unix epoch. Doubles as the LRU clock: the
    /// entry with the smallest `scrolled_at` is the one evicted when the
    /// store is over [`MAX_POSITIONS`].
    pub scrolled_at: u64,
}

/// Reading positions, keyed by canonicalized file path, capped at
/// [`MAX_POSITIONS`] and LRU-evicted past it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    positions: HashMap<String, ReadingPosition>,
}

impl State {
    pub fn position(&self, path: &str) -> Option<ReadingPosition> {
        self.positions.get(path).copied()
    }

    /// Records where `path` was last scrolled to, then evicts the
    /// least-recently-scrolled entry if that pushed the store over
    /// [`MAX_POSITIONS`]. A store that grows without bound is a store that
    /// eventually costs a startup — this is what keeps it flat forever.
    pub fn set_position(&mut self, path: impl Into<String>, line: usize, scrolled_at: u64) {
        self.positions
            .insert(path.into(), ReadingPosition { line, scrolled_at });
        self.evict_over_cap();
    }

    fn evict_over_cap(&mut self) {
        while self.positions.len() > MAX_POSITIONS {
            let oldest = self
                .positions
                .iter()
                .min_by_key(|(_, p)| p.scrolled_at)
                .map(|(path, _)| path.clone());
            match oldest {
                Some(path) => {
                    self.positions.remove(&path);
                }
                None => break, // unreachable: len() > 0 implies a min exists
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.positions.len()
    }
}

pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

pub fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("state.json")
}

/// Reads `settings.json`, or [`Settings::default`] if it is missing,
/// unreadable, or fails to parse — a corrupt or absent settings file is not
/// a reason to refuse to open the app.
pub fn read_settings(app_data_dir: &Path) -> Settings {
    read_json_or_default::<Settings>(&settings_path(app_data_dir)).clamp()
}

pub fn write_settings(app_data_dir: &Path, settings: &Settings) -> io::Result<()> {
    write_json_atomic(&settings_path(app_data_dir), &settings.clone().clamp())
}

/// Reads `state.json`, or an empty [`State`] under the same "never block
/// startup on a bad file" reasoning as [`read_settings`].
pub fn read_state(app_data_dir: &Path) -> State {
    read_json_or_default::<State>(&state_path(app_data_dir))
}

pub fn write_state(app_data_dir: &Path, state: &State) -> io::Result<()> {
    write_json_atomic(&state_path(app_data_dir), state)
}

/// The reading position for one file, or `None` if it has never been
/// recorded. `path` should be the canonicalized path `open_document` already
/// resolves, so the same file is always the same key regardless of how it
/// was opened.
pub fn reading_position(app_data_dir: &Path, path: &str) -> Option<ReadingPosition> {
    read_state(app_data_dir).position(path)
}

/// Records where `path` was last scrolled to and writes `state.json`
/// atomically. `scrolled_at` is the caller's wall-clock reading (Unix millis)
/// rather than something this function reads itself, so a test can supply a
/// controlled clock instead of racing the real one.
pub fn record_reading_position(
    app_data_dir: &Path,
    path: &str,
    line: usize,
    scrolled_at: u64,
) -> io::Result<()> {
    let mut state = read_state(app_data_dir);
    state.set_position(path, line, scrolled_at);
    write_state(app_data_dir, &state)
}

fn read_json_or_default<T>(path: &Path) -> T
where
    T: Default + serde::de::DeserializeOwned,
{
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes `value` to `path` atomically: serialize to a temp file in the same
/// directory, then rename over the target.
///
/// A rename within one filesystem is atomic on both NTFS and ext4/WSL2, so a
/// reader — or a process killed mid-save — never observes a half-written
/// file. Writing the target in place would mean exactly that: a process
/// killed between `File::create` and the last `write` leaves `settings.json`
/// truncated or invalid JSON, and the next launch loses every preference (or
/// worse, every reading position) to a crash that had nothing to do with
/// this file.
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "store path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(dir)?;

    let json = serde_json::to_vec_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let tmp = tmp_path(path);
    std::fs::write(&tmp, &json)?;
    let result = std::fs::rename(&tmp, path);
    if result.is_err() {
        // Best-effort: do not leave the temp file behind after a failed
        // rename. Ignored deliberately — reporting the original rename error
        // matters far more than reporting a cleanup failure on top of it.
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// A sibling of `path`, distinguished by an extension plus this process's id
/// so two Marklet instances (there can be at most one live one, per
/// `tauri-plugin-single-instance`, but a leftover from a crashed prior run is
/// possible) never race on the same temp file.
fn tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("store");
    path.with_file_name(format!("{name}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("marklet-store-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_settings_file_reads_as_default() {
        let dir = sandbox("missing-settings");
        assert_eq!(read_settings(&dir), Settings::default());
    }

    #[test]
    fn settings_round_trip_through_disk() {
        let dir = sandbox("settings-roundtrip");
        let settings = Settings {
            theme: Theme::Dark,
            accent_hue: 142,
            typeface: Typeface::Literary,
            palette: Palette::Forest,
            density: Density::Compact,
            measure: 80,
            zoom: 140,
        };
        write_settings(&dir, &settings).unwrap();
        assert_eq!(read_settings(&dir), settings);
    }

    #[test]
    fn a_corrupt_settings_file_reads_as_default_rather_than_panicking() {
        let dir = sandbox("corrupt-settings");
        std::fs::write(settings_path(&dir), b"{ not json").unwrap();
        assert_eq!(read_settings(&dir), Settings::default());
    }

    #[test]
    fn measure_is_clamped_on_write_and_on_read() {
        let dir = sandbox("measure-clamp");

        // A value that reached the file some other way than the slider —
        // simulated by writing raw JSON directly, which skips `write_settings`
        // and therefore its own clamp — must still come back clamped.
        std::fs::write(
            settings_path(&dir),
            br#"{"theme":"system","accent-hue":221,"typeface":"editorial","palette":"default","density":"normal","measure":255}"#,
        )
        .unwrap();
        assert_eq!(read_settings(&dir).measure, 100);

        let settings = Settings {
            measure: 3,
            ..Settings::default()
        };
        write_settings(&dir, &settings).unwrap();
        assert_eq!(read_settings(&dir).measure, 48);
    }

    #[test]
    fn missing_state_file_reads_as_empty() {
        let dir = sandbox("missing-state");
        assert!(read_state(&dir).position("anything.md").is_none());
    }

    #[test]
    fn records_and_reads_back_a_reading_position() {
        let dir = sandbox("state-roundtrip");
        record_reading_position(&dir, "/vault/note.md", 42, 1_000).unwrap();
        let pos = reading_position(&dir, "/vault/note.md").unwrap();
        assert_eq!(pos.line, 42);
        assert_eq!(pos.scrolled_at, 1_000);
    }

    #[test]
    fn lru_cap_evicts_the_least_recently_scrolled_entry() {
        let mut state = State::default();
        for i in 0..MAX_POSITIONS {
            state.set_position(format!("file-{i}.md"), i, i as u64);
        }
        assert_eq!(state.len(), MAX_POSITIONS);

        // One more, scrolled most recently: pushes the store over the cap.
        state.set_position("new.md", 0, MAX_POSITIONS as u64);

        assert_eq!(state.len(), MAX_POSITIONS, "cap holds after the insert");
        assert!(
            state.position("file-0.md").is_none(),
            "file-0.md had the smallest scrolled_at and must be the one evicted"
        );
        assert!(
            state.position("file-1.md").is_some(),
            "next-oldest survives"
        );
        assert!(state.position("new.md").is_some());
    }

    #[test]
    fn re_scrolling_an_existing_file_updates_it_in_place_without_growing_the_store() {
        let mut state = State::default();
        state.set_position("a.md", 1, 10);
        state.set_position("b.md", 1, 20);
        state.set_position("a.md", 99, 30); // re-scroll, not a new entry

        assert_eq!(state.len(), 2);
        assert_eq!(state.position("a.md").unwrap().line, 99);
        assert_eq!(state.position("a.md").unwrap().scrolled_at, 30);
    }

    #[test]
    fn atomic_write_survives_a_failed_rename_without_touching_the_original() {
        let dir = sandbox("atomic-fail");
        let target = settings_path(&dir);

        // Stand-in for "something already at the destination that must
        // survive a failed save": a directory, not a file. `rename` onto an
        // existing directory fails on both ext4 and NTFS regardless of the
        // directory's contents, which is exactly the failure this write must
        // survive without corrupting what was there.
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.txt"), b"original").unwrap();

        // Not asserting a specific `ErrorKind` here — the exact kind `rename`
        // reports for "target is a directory" is platform-dependent and, on
        // some standard library versions, not even a stable-named variant.
        // What must hold regardless of that is the point of this test: the
        // rename fails, and the original survives the failed write.
        write_json_atomic(&target, &Settings::default()).unwrap_err();

        // The original is untouched: still a directory, still holding its file.
        assert!(
            target.is_dir(),
            "the failed write must not replace the original"
        );
        assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"original");

        // And the temp file used to attempt the write is cleaned up rather
        // than left behind as debris after the failure.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "left temp files behind: {leftovers:?}"
        );
    }

    #[test]
    fn a_successful_write_leaves_no_temp_file_behind() {
        let dir = sandbox("atomic-success");
        write_settings(&dir, &Settings::default()).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "left temp files behind: {leftovers:?}"
        );
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    #[test]
    fn a_zoom_from_outside_the_slider_is_clamped() {
        // `settings.json` is a plain file a person can edit, and a future
        // import could carry anything. 0 would make the document invisible and
        // 400 would put two words on a line; neither should be reachable by
        // any path that does not go through the slider or Ctrl+wheel.
        for (given, expected) in [
            (0, 50),
            (49, 50),
            (50, 50),
            (100, 100),
            (250, 250),
            (255, 250),
        ] {
            let clamped = Settings {
                zoom: given,
                ..Settings::default()
            }
            .clamp();
            assert_eq!(clamped.zoom, expected, "zoom {given}");
        }
    }

    #[test]
    fn the_defaults_are_the_ones_tokens_css_states() {
        // There is no build-time link between this and `src/styles/tokens.css`,
        // so the two are kept in step by eye and this test is the reminder.
        let d = Settings::default();
        assert_eq!(d.accent_hue, 70, "Material Amber 700's hue in oklch");
        assert_eq!(d.measure, 100, "--measure: 100ch");
        assert_eq!(d.zoom, 100, "no scaling until the user asks");
    }
}

#[cfg(test)]
mod migration_tests {
    use super::tests::sandbox;
    use super::*;

    /// A settings file written by an older version must keep the preferences it
    /// does carry.
    ///
    /// This is the regression that motivated `#[serde(default)]` per field:
    /// before it, one unknown or missing key made the whole document fail to
    /// parse and every preference reverted to the default — silently, at the
    /// next launch, with the file rewritten on the first change so the original
    /// values were gone for good.
    #[test]
    fn an_older_settings_file_keeps_what_it_has() {
        let dir = sandbox("settings-migration");

        // Exactly what 0.1.0 wrote: `motion`, which no longer exists, and no
        // `font-size`, which did not exist yet.
        std::fs::write(
            settings_path(&dir),
            br#"{"theme":"dark","accent-hue":142,"typeface":"literary","palette":"forest","density":"compact","motion":"off","measure":72}"#,
        )
        .unwrap();

        let read = read_settings(&dir);
        assert_eq!(read.theme, Theme::Dark, "a kept field must survive");
        assert_eq!(read.accent_hue, 142);
        assert_eq!(read.measure, 72);
        assert_eq!(read.density, Density::Compact);
        assert_eq!(
            read.zoom, 100,
            "a field the old file never had takes the default"
        );
    }

    /// The wire format is what `src/lib/ipc.ts` declares, field for field.
    ///
    /// `read_settings` crosses the IPC boundary as JSON, so a rename here is a
    /// rename in the webview. `kebab-case` spelled this field `accent-hue`,
    /// TypeScript read `accent_hue`, and the whole accent system collapsed into
    /// an invalid `oklch()` — see `Settings`' own doc comment. This test is the
    /// tripwire for that, not a test of serde.
    #[test]
    fn settings_serialize_with_the_names_typescript_reads() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(
            json.contains("\"accent_hue\""),
            "the webview reads `accent_hue`: {json}"
        );
        assert!(
            !json.contains("accent-hue"),
            "no kebab-case on the wire: {json}"
        );
    }

    #[test]
    fn a_file_that_is_not_json_at_all_still_yields_defaults() {
        // The other half of the contract: per-field defaults must not turn a
        // genuinely corrupt file into a partial read.
        let dir = sandbox("settings-corrupt");
        std::fs::write(settings_path(&dir), b"{ this is not json").unwrap();
        assert_eq!(read_settings(&dir), Settings::default());
    }
}
