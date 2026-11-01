//! The colours of this machine's desktop, for the panel's wallpaper theme:
//! DankMaterialShell's Material roles where it runs, else pywal's sixteen.
//! Both are generated from the wallpaper and rewritten when it changes.
//! Pywalfox has nothing of its own to offer: it is a Firefox extension
//! showing this same `wal/colors.json`, and pages cannot read extensions.

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

use serde_json::Value;

use crate::model::{Palette, PaletteColours, PaletteSource};

const DMS_FILE: &str = "DankMaterialShell/dms-colors.json";
const WAL_FILE: &str = "wal/colors.json";
const ANSI: usize = 16;
/// DankMaterialShell writes both files in one run, so wal's being newer by
/// less than this is that run, not someone running `wal`.
const SAME_RUN: Duration = Duration::from_secs(60);

/// When the files changed; `read` is only worth calling when this moves.
pub type Stamp = (Option<SystemTime>, Option<SystemTime>);

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn stamp(cache: &Path) -> Stamp {
    (
        modified(&cache.join(DMS_FILE)),
        modified(&cache.join(WAL_FILE)),
    )
}

/// `Ok(None)` when the file is not there; an error when it is not JSON (yet).
fn json(path: &Path) -> Result<Option<Value>, serde_json::Error> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some),
        Err(_) => Ok(None),
    }
}

/// Reads the newer of the two; DankMaterialShell's on a tie, as it has more,
/// and pywal's when its format turns out to be one this does not know.
/// `None` when neither is there, or one is caught mid-write: falling back
/// then would flash the other file's colours until the next look.
pub fn read(cache: &Path) -> Option<Palette> {
    let wal = json(&cache.join(WAL_FILE)).ok()?;
    let wallpaper = wal.as_ref().and_then(wallpaper_of);
    let (dms_at, wal_at) = stamp(cache);
    let wal_newer = match (dms_at, wal_at) {
        (Some(dms), Some(wal)) => wal.duration_since(dms).is_ok_and(|ahead| ahead > SAME_RUN),
        _ => false,
    };
    let dms = if wal_newer {
        None
    } else {
        json(&cache.join(DMS_FILE)).ok()?
    };
    let palette = dms
        .as_ref()
        .and_then(from_dms)
        .or_else(|| wal.as_ref().and_then(from_wal))?;
    Some(Palette {
        wallpaper,
        ..palette
    })
}

fn wallpaper_of(wal: &Value) -> Option<String> {
    let path = Path::new(wal.get("wallpaper")?.as_str()?);
    Some(path.file_name()?.to_str()?.to_owned())
}

/// `#rrggbb`, lowercased; anything else is not passed on.
fn hex(v: Option<&Value>) -> Option<String> {
    let text = v?.as_str()?;
    let digits = text.strip_prefix('#')?;
    (digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| text.to_ascii_lowercase())
}

fn from_dms(v: &Value) -> Option<Palette> {
    let mode = |name: &str| {
        let roles = v.pointer(&format!("/colors/{name}"))?;
        let role = |key: &str| hex(roles.get(key));
        let ansi: Option<Vec<String>> = (0..ANSI)
            .map(|i| hex(v.pointer(&format!("/dank16/color{i}/{name}"))))
            .collect();
        Some(PaletteColours {
            background: role("background")?,
            foreground: role("on_background")?,
            primary: role("primary"),
            secondary: role("secondary"),
            tertiary: role("tertiary"),
            surface: role("surface_container"),
            muted: role("on_surface_variant"),
            ansi: ansi.unwrap_or_default(),
        })
    };
    let (dark, light) = (mode("dark"), mode("light"));
    (dark.is_some() || light.is_some()).then_some(Palette {
        source: PaletteSource::Dms,
        wallpaper: None,
        dark,
        light,
    })
}

fn from_wal(v: &Value) -> Option<Palette> {
    let colors = v.get("colors")?;
    let ansi: Vec<String> = (0..ANSI)
        .map(|i| hex(colors.get(format!("color{i}"))))
        .collect::<Option<_>>()?;
    let special = |key: &str| hex(v.pointer(&format!("/special/{key}")));
    let background = special("background").unwrap_or_else(|| ansi[0].clone());
    let foreground = special("foreground").unwrap_or_else(|| ansi[15].clone());
    let dark = is_dark(&background);
    let colours = PaletteColours {
        background,
        foreground,
        primary: None,
        secondary: None,
        tertiary: None,
        surface: None,
        muted: None,
        ansi,
    };
    Some(Palette {
        source: PaletteSource::Pywal,
        wallpaper: None,
        dark: dark.then(|| colours.clone()),
        light: (!dark).then_some(colours),
    })
}

/// Rough perceived lightness of a checked `#rrggbb`.
fn is_dark(colour: &str) -> bool {
    let channel = |at: usize| u32::from_str_radix(&colour[at..at + 2], 16).unwrap_or(0);
    2126 * channel(1) + 7152 * channel(3) + 722 * channel(5) < 1_280_000
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn dms() -> Value {
        let roles = |bg: &str, fg: &str| {
            json!({
                "background": bg, "on_background": fg, "primary": "#9AA1C9",
                "secondary": "#c3c5dc", "tertiary": "#f8e0ff",
                "surface_container": "#181920", "on_surface_variant": "#a2a2ae",
            })
        };
        let dank16: serde_json::Map<String, Value> = (0..16)
            .map(|i| {
                (
                    format!("color{i}"),
                    json!({"dark": "#101010", "light": "#efefef"}),
                )
            })
            .collect();
        json!({
            "mode": "dark",
            "colors": {"dark": roles("#131318", "#e4e1e8"), "light": roles("#fbf8ff", "#1b1b20")},
            "dank16": dank16,
        })
    }

    fn wal(background: &str) -> Value {
        let mut colors = serde_json::Map::new();
        for i in 0..16 {
            colors.insert(format!("color{i}"), json!(format!("#0000{i:02x}")));
        }
        json!({
            "wallpaper": "/home/u/Wallpapers/forest.jpg",
            "special": {"background": background, "foreground": "#eeeeee"},
            "colors": colors,
        })
    }

    #[test]
    fn dank_material_shell_gives_both_modes_with_roles() {
        let p = from_dms(&dms()).unwrap();
        assert_eq!(p.source, PaletteSource::Dms);
        let dark = p.dark.unwrap();
        assert_eq!(dark.background, "#131318");
        assert_eq!(dark.foreground, "#e4e1e8");
        assert_eq!(dark.primary.as_deref(), Some("#9aa1c9"), "lowercased");
        assert_eq!(dark.ansi.len(), 16);
        assert_eq!(p.light.unwrap().background, "#fbf8ff");
    }

    #[test]
    fn a_dms_file_with_gaps_keeps_what_it_can() {
        let mut v = dms();
        v["colors"]["light"]["background"] = json!("rgb(1,2,3)");
        v["dank16"]["color7"]["dark"] = json!("nope");
        let p = from_dms(&v).unwrap();
        assert!(p.light.is_none(), "no background, no mode");
        assert!(
            p.dark.unwrap().ansi.is_empty(),
            "the ansi set is all or nothing"
        );
        assert!(from_dms(&json!({"colors": {}})).is_none());
    }

    #[test]
    fn pywal_is_the_mode_its_background_is() {
        let dark = from_wal(&wal("#0b0c10")).unwrap();
        assert_eq!(dark.source, PaletteSource::Pywal);
        assert!(dark.light.is_none());
        let colours = dark.dark.unwrap();
        assert_eq!(colours.ansi.len(), 16);
        assert_eq!(colours.primary, None);

        assert!(from_wal(&wal("#f4f1ea")).unwrap().dark.is_none());
    }

    #[test]
    fn pywal_without_special_reads_the_slots_and_rejects_bad_colours() {
        let mut v = wal("#000000");
        v.as_object_mut().unwrap().remove("special");
        let p = from_wal(&v).unwrap().dark.unwrap();
        assert_eq!(
            (p.background.as_str(), p.foreground.as_str()),
            ("#000000", "#00000f")
        );

        v["colors"]["color3"] = json!("url(evil)");
        assert!(from_wal(&v).is_none());
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ccdash-palette-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("wal")).unwrap();
        fs::create_dir_all(dir.join("DankMaterialShell")).unwrap();
        dir
    }

    fn write(path: &Path, v: &Value, at: Option<SystemTime>) {
        fs::write(path, v.to_string()).unwrap();
        if let Some(at) = at {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(at)
                .unwrap();
        }
    }

    #[test]
    fn reads_the_newer_file_and_prefers_dms_within_a_run() {
        let cache = dir("newer");
        assert!(read(&cache).is_none(), "neither exists");
        assert_eq!(stamp(&cache), (None, None));

        let now = SystemTime::now();
        write(&cache.join(WAL_FILE), &wal("#0b0c10"), Some(now));
        let only_wal = read(&cache).unwrap();
        assert_eq!(only_wal.source, PaletteSource::Pywal);
        assert_eq!(only_wal.wallpaper.as_deref(), Some("forest.jpg"));

        write(
            &cache.join(DMS_FILE),
            &dms(),
            Some(now - Duration::from_secs(5)),
        );
        let same_run = read(&cache).unwrap();
        assert_eq!(same_run.source, PaletteSource::Dms);
        assert_eq!(
            same_run.wallpaper.as_deref(),
            Some("forest.jpg"),
            "wal's file names it"
        );

        write(
            &cache.join(DMS_FILE),
            &dms(),
            Some(now - Duration::from_secs(3600)),
        );
        assert_eq!(
            read(&cache).unwrap().source,
            PaletteSource::Pywal,
            "a stale DMS file"
        );
        fs::remove_dir_all(&cache).unwrap();
    }

    #[test]
    fn a_file_caught_mid_write_reads_as_nothing() {
        let cache = dir("partial");
        fs::write(cache.join(WAL_FILE), "{\"colors\": {\"col").unwrap();
        assert!(read(&cache).is_none());
        assert!(stamp(&cache).1.is_some(), "yet it exists");

        write(&cache.join(WAL_FILE), &wal("#0b0c10"), None);
        fs::write(cache.join(DMS_FILE), "{\"colors\": {\"dark").unwrap();
        assert!(read(&cache).is_none(), "not pywal's colours for a moment");
        fs::remove_dir_all(&cache).unwrap();
    }

    #[test]
    fn a_dms_format_this_does_not_know_falls_back_to_pywal() {
        let cache = dir("format");
        write(&cache.join(WAL_FILE), &wal("#0b0c10"), None);
        write(&cache.join(DMS_FILE), &json!({"schema": 2}), None);
        assert_eq!(read(&cache).unwrap().source, PaletteSource::Pywal);
        fs::remove_dir_all(&cache).unwrap();
    }
}
