//! The user's preferences, kept in a small text file between runs.
//!
//! The file is `key=value` lines in the platform's config directory
//! (`%APPDATA%\lazykss\settings.txt` on Windows). Unknown keys and bad values
//! are ignored, so an older or hand-edited file never stops the editor from
//! starting; a missing file means the defaults.

use std::path::{Path, PathBuf};

/// The colour scheme of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Whatever the toolkit picks for the platform.
    #[default]
    System,
    Light,
    Dark,
    Midnight,
    Daylight,
    Nightfall,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 6] = [
        ThemeChoice::System,
        ThemeChoice::Light,
        ThemeChoice::Dark,
        ThemeChoice::Midnight,
        ThemeChoice::Daylight,
        ThemeChoice::Nightfall,
    ];

    /// The name shown in the settings window.
    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System default",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Midnight => "Midnight",
            ThemeChoice::Daylight => "Daylight",
            ThemeChoice::Nightfall => "Nightfall",
        }
    }

    /// The name stored in the file.
    fn key(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
            ThemeChoice::Midnight => "midnight",
            ThemeChoice::Daylight => "daylight",
            ThemeChoice::Nightfall => "nightfall",
        }
    }

    fn from_key(key: &str) -> Option<ThemeChoice> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }

    /// The position in [`ThemeChoice::ALL`].
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The choice at `index`, or the default for an index past the end.
    pub fn from_index(index: usize) -> ThemeChoice {
        Self::ALL.get(index).copied().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub theme: ThemeChoice,
    /// Whether widgets animate (xui still honours the system's reduced-motion setting).
    pub animations: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeChoice::default(),
            animations: true,
        }
    }
}

impl Settings {
    /// Parses the file's text; anything unreadable keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "theme" => {
                    if let Some(theme) = ThemeChoice::from_key(value) {
                        settings.theme = theme;
                    }
                }
                "animations" => match value {
                    "on" | "true" => settings.animations = true,
                    "off" | "false" => settings.animations = false,
                    _ => {}
                },
                _ => {}
            }
        }
        settings
    }

    pub fn to_text(&self) -> String {
        format!(
            "theme={}\nanimations={}\n",
            self.theme.key(),
            if self.animations { "on" } else { "off" }
        )
    }

    /// Where the file lives: `KSS_SETTINGS` if set, else the config directory.
    pub fn path() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("KSS_SETTINGS") {
            return Some(PathBuf::from(path));
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))?;
        Some(base.join("lazykss").join("settings.txt"))
    }

    /// Reads the saved settings; defaults when there is no readable file.
    pub fn load() -> Settings {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| Settings::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        for theme in ThemeChoice::ALL {
            for animations in [true, false] {
                let s = Settings { theme, animations };
                assert_eq!(Settings::parse(&s.to_text()), s);
            }
        }
    }

    #[test]
    fn junk_keeps_defaults() {
        let s = Settings::parse("theme=plaid\nanimations=maybe\nnonsense\nx=1\n");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn index_round_trips() {
        for theme in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::from_index(theme.index()), theme);
        }
    }

    #[test]
    fn saves_and_reloads_from_a_file() {
        let dir = std::env::temp_dir().join(format!("lazykss-settings-{}", std::process::id()));
        let path = dir.join("nested").join("settings.txt");
        let s = Settings {
            theme: ThemeChoice::Nightfall,
            animations: false,
        };
        s.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(Settings::parse(&text), s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
