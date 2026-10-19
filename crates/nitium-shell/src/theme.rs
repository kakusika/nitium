//! Glue between `origiri-theme`'s resolved tokens and Slint's `Tokens` global.

use origiri_theme::palette::{self, RgbColor, presets};
use origiri_theme::{CornerRadius, Rgba, ThemeSettings};
use slint::Color;

use nitium_settings::{Settings, ThemeMode};

use crate::Tokens;

fn color(rgba: Rgba) -> Color {
    Color::from_argb_u8(rgba.a, rgba.r, rgba.g, rgba.b)
}

pub fn corner_radius_from_str(s: &str) -> CornerRadius {
    match s {
        "disabled" => CornerRadius::Disabled,
        "small" => CornerRadius::Small,
        "medium" => CornerRadius::Medium,
        "large" => CornerRadius::Large,
        "circle" => CornerRadius::Circle,
        _ => CornerRadius::Small,
    }
}

/// Resolves the variant id from user settings and system dark mode.
pub fn resolve_variant_id(settings: &Settings, system_dark: bool) -> &'static str {
    match settings.theme_preset.as_str() {
        "system" => match settings.theme {
            ThemeMode::System => {
                if system_dark {
                    "dark"
                } else {
                    "light"
                }
            }
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        },
        id => {
            if let Some(v) = presets::find_variant(id) {
                v.id
            } else if system_dark {
                "dark"
            } else {
                "light"
            }
        }
    }
}

/// Resolves the theme from user settings and applies it to the window's `Tokens` global.
/// Returns whether the resolved theme is dark (for synchronizing the page engine).
pub fn apply_theme(tokens: &Tokens, settings: &Settings, system_dark: bool) -> bool {
    let variant_id = resolve_variant_id(settings, system_dark);
    let is_dark = palette::is_dark_variant(variant_id);

    let mut theme_settings = ThemeSettings {
        variant: variant_id.to_string(),
        corner_radius: corner_radius_from_str(&settings.corner_radius),
        ..Default::default()
    };

    if let Some(hex) = &settings.accent_color {
        let hex = hex.trim();
        if !hex.is_empty() && hex != "default" {
            theme_settings.accent = RgbColor::from_hex(hex);
        }
    }

    let resolved = origiri_theme::resolve(&theme_settings);

    origiri::push_color_tokens!(&tokens, resolved.colors, color);

    tokens.set_corner_radius(resolved.shape.corner_radius_px);
    tokens.set_border_width(resolved.shape.border_width_px);

    tokens.set_spacing_small(resolved.spacing.small_px.round());
    tokens.set_spacing_medium(resolved.spacing.medium_px.round());
    tokens.set_spacing_large(resolved.spacing.large_px.round());

    tokens.set_anim_very_short(resolved.animation.very_short_ms);
    tokens.set_anim_short(resolved.animation.short_ms);
    tokens.set_anim_long(resolved.animation.long_ms);
    tokens.set_anim_very_long(resolved.animation.very_long_ms);

    is_dark
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_resolution_respects_settings_and_system() {
        let mut settings = Settings::default();
        assert_eq!(settings.theme_preset, "system");
        assert_eq!(resolve_variant_id(&settings, true), "dark");
        assert_eq!(resolve_variant_id(&settings, false), "light");

        settings.theme = ThemeMode::Dark;
        assert_eq!(resolve_variant_id(&settings, false), "dark");

        settings.theme = ThemeMode::Light;
        assert_eq!(resolve_variant_id(&settings, true), "light");

        settings.theme_preset = "catppuccin-mocha".into();
        assert_eq!(resolve_variant_id(&settings, false), "catppuccin-mocha");

        settings.theme_preset = "tokyonight-day".into();
        assert_eq!(resolve_variant_id(&settings, true), "tokyonight-day");

        settings.theme_preset = "invalid-name".into();
        assert_eq!(resolve_variant_id(&settings, true), "dark");
        assert_eq!(resolve_variant_id(&settings, false), "light");
    }

    #[test]
    fn corner_radius_parsing() {
        assert_eq!(corner_radius_from_str("disabled"), CornerRadius::Disabled);
        assert_eq!(corner_radius_from_str("small"), CornerRadius::Small);
        assert_eq!(corner_radius_from_str("medium"), CornerRadius::Medium);
        assert_eq!(corner_radius_from_str("large"), CornerRadius::Large);
        assert_eq!(corner_radius_from_str("circle"), CornerRadius::Circle);
        assert_eq!(corner_radius_from_str("other"), CornerRadius::Small);
    }
}
