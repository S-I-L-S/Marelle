// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

//! Where a person's files live, worked out from an environment snapshot.
//!
//! Every function takes a lookup instead of reading the process environment,
//! so a test can describe any system. Each also takes the platform as an
//! argument, so the Windows rules can be tested on Linux and the Unix rules
//! on Windows. Programs pass [`Platform::HOST`].

use std::path::PathBuf;

/// The family of systems whose habits decide where files go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux, macOS, Android (Termux), the BSDs and the rest of the Unix
    /// world. They share the XDG habit of keeping settings in `~/.config`.
    Unix,
    /// Windows.
    Windows,
}

impl Platform {
    /// The platform this program was built for.
    pub const HOST: Platform = if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Unix
    };
}

/// A variable as a path. Unset and empty both mean "not there".
fn var(lookup: &impl Fn(&str) -> Option<String>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The home directory: `HOME`, and on Windows `USERPROFILE` when `HOME` is not
/// set. Git Bash and MSYS set `HOME`; a plain Windows session sets only
/// `USERPROFILE`.
pub fn home_dir(platform: Platform, lookup: &impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let userprofile = match platform {
        Platform::Windows => var(lookup, "USERPROFILE"),
        Platform::Unix => None,
    };
    var(lookup, "HOME").or(userprofile)
}

/// The directory that holds the `marelle` directory with the user-wide
/// settings.
///
/// The first of these that is set wins:
///
/// 1. `XDG_CONFIG_HOME`, on every system, because setting it is a choice.
/// 2. `APPDATA`, on Windows only. It is where Windows programs keep settings.
/// 3. `.config` inside the home directory, see [`home_dir`].
///
/// Without any of them there is no directory, and so no user-wide file.
pub fn config_home(
    platform: Platform,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    let appdata = match platform {
        Platform::Windows => var(lookup, "APPDATA"),
        Platform::Unix => None,
    };
    var(lookup, "XDG_CONFIG_HOME")
        .or(appdata)
        .or_else(|| home_dir(platform, lookup).map(|home| home.join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    const WINDOWS: &[(&str, &str)] = &[
        ("APPDATA", r"C:\Users\someone\AppData\Roaming"),
        ("USERPROFILE", r"C:\Users\someone"),
    ];

    #[test]
    fn xdg_config_home_wins_on_every_platform() {
        for platform in [Platform::Unix, Platform::Windows] {
            let lookup = env(&[
                ("XDG_CONFIG_HOME", "/cfg"),
                ("APPDATA", "/roaming"),
                ("HOME", "/home/someone"),
            ]);
            assert_eq!(
                config_home(platform, &lookup),
                Some(PathBuf::from("/cfg")),
                "{platform:?}"
            );
        }
    }

    #[test]
    fn unix_uses_dot_config_in_the_home_directory() {
        let lookup = env(&[("HOME", "/home/someone")]);
        assert_eq!(
            config_home(Platform::Unix, &lookup),
            Some(PathBuf::from("/home/someone").join(".config"))
        );
    }

    #[test]
    fn unix_does_not_know_appdata_or_userprofile() {
        let lookup = env(WINDOWS);
        assert_eq!(config_home(Platform::Unix, &lookup), None);
        assert_eq!(home_dir(Platform::Unix, &lookup), None);
    }

    #[test]
    fn windows_prefers_appdata_to_home() {
        // Git Bash sets HOME as well, and the native answer should still win.
        let lookup = env(&[
            ("APPDATA", r"C:\Users\someone\AppData\Roaming"),
            ("HOME", r"C:\Users\someone"),
        ]);
        assert_eq!(
            config_home(Platform::Windows, &lookup),
            Some(PathBuf::from(r"C:\Users\someone\AppData\Roaming"))
        );
    }

    #[test]
    fn windows_falls_back_to_home_and_then_to_userprofile() {
        let lookup = env(&[("HOME", r"C:\msys\home\someone")]);
        assert_eq!(
            config_home(Platform::Windows, &lookup),
            Some(PathBuf::from(r"C:\msys\home\someone").join(".config"))
        );

        let lookup = env(&[("USERPROFILE", r"C:\Users\someone")]);
        assert_eq!(
            config_home(Platform::Windows, &lookup),
            Some(PathBuf::from(r"C:\Users\someone").join(".config"))
        );
    }

    #[test]
    fn home_is_home_first_and_userprofile_second_on_windows() {
        let both = env(&[("HOME", "/h"), ("USERPROFILE", r"C:\Users\someone")]);
        assert_eq!(
            home_dir(Platform::Windows, &both),
            Some(PathBuf::from("/h"))
        );
        let only_profile = env(&[("USERPROFILE", r"C:\Users\someone")]);
        assert_eq!(
            home_dir(Platform::Windows, &only_profile),
            Some(PathBuf::from(r"C:\Users\someone"))
        );
    }

    #[test]
    fn empty_values_count_as_unset() {
        let lookup = env(&[
            ("XDG_CONFIG_HOME", ""),
            ("APPDATA", ""),
            ("HOME", ""),
            ("USERPROFILE", ""),
        ]);
        for platform in [Platform::Unix, Platform::Windows] {
            assert_eq!(config_home(platform, &lookup), None, "{platform:?}");
            assert_eq!(home_dir(platform, &lookup), None, "{platform:?}");
        }
    }

    #[test]
    fn no_variables_means_no_directory() {
        let lookup = env(&[]);
        assert_eq!(config_home(Platform::HOST, &lookup), None);
        assert_eq!(home_dir(Platform::HOST, &lookup), None);
    }
}
