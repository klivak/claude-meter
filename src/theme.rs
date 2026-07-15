#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeMode {
    Auto,
    Dark,
    Light,
    Midnight,
    Sunset,
}

impl ThemeMode {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Dark,
        Self::Light,
        Self::Midnight,
        Self::Sunset,
    ];

    pub fn from_str(s: &str) -> Self {
        match s {
            "dark" => Self::Dark,
            "light" => Self::Light,
            "midnight" => Self::Midnight,
            "sunset" => Self::Sunset,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Midnight => "midnight",
            Self::Sunset => "sunset",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Midnight => "Midnight",
            Self::Sunset => "Sunset",
        }
    }

    pub fn is_valid(s: &str) -> bool {
        Self::ALL.iter().any(|theme| theme.as_str() == s)
    }
}

/// Resolve the effective theme (Dark or Light) accounting for Auto mode.
pub fn resolve_theme(mode: ThemeMode) -> ResolvedTheme {
    match mode {
        ThemeMode::Dark => ResolvedTheme::Dark,
        ThemeMode::Light => ResolvedTheme::Light,
        ThemeMode::Midnight => ResolvedTheme::Midnight,
        ThemeMode::Sunset => ResolvedTheme::Sunset,
        ThemeMode::Auto => {
            if is_system_light_theme() {
                ResolvedTheme::Light
            } else {
                ResolvedTheme::Dark
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedTheme {
    Dark,
    Light,
    Midnight,
    Sunset,
}

impl ResolvedTheme {
    pub fn uses_dark_chrome(self) -> bool {
        !matches!(self, Self::Light)
    }
}

/// Read Windows registry to determine system theme.
/// Returns true if Windows is in light mode.
#[cfg(windows)]
pub fn is_system_light_theme() -> bool {
    const KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const VALUE: &str = "AppsUseLightTheme";

    let key_wide: Vec<u16> = KEY.encode_utf16().chain(std::iter::once(0)).collect();
    let value_wide: Vec<u16> = VALUE.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false; // Default dark
        }

        let mut data: u32 = 0;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let mut data_type = REG_DWORD;

        let result = RegQueryValueExW(
            hkey,
            PCWSTR(value_wide.as_ptr()),
            None,
            Some(&mut data_type),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey).ok();

        if result.is_ok() {
            data != 0
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
pub fn is_system_light_theme() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectable_themes_round_trip_through_config_values() {
        for theme in ThemeMode::ALL {
            assert_eq!(ThemeMode::from_str(theme.as_str()), theme);
            assert!(ThemeMode::is_valid(theme.as_str()));
        }
    }

    #[test]
    fn stylish_themes_resolve_without_system_lookup() {
        assert_eq!(resolve_theme(ThemeMode::Midnight), ResolvedTheme::Midnight);
        assert_eq!(resolve_theme(ThemeMode::Sunset), ResolvedTheme::Sunset);
        assert!(ResolvedTheme::Midnight.uses_dark_chrome());
        assert!(ResolvedTheme::Sunset.uses_dark_chrome());
    }
}
