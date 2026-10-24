//! Catppuccin Mocha palette and semantic UI styles for the RVS++ TUI.
//!
//! Two rules keep the theme legible:
//!
//! 1. **One meaning, one color.** Every role below owns exactly one hue, and
//!    the hues that share a line of text (syntax tokens, status bar modes) are
//!    spaced far apart in CIELAB under normal, deuteranopic and protanopic
//!    vision (see `tests`).
//! 2. **No color is drawn on a background it fails against.** Text colors
//!    used on [`BASE`] / [`MANTLE`] / [`SURFACE0`] clear WCAG AA (4.5:1);
//!    structural lines ([`FAINT`]) clear the 3:1 non-text threshold; badges
//!    pair an accent background with [`CRUST`] foreground (≥ 8:1).
//!
//! Bold/italic are always *additive* cues, never the only cue: `ERR` is bold,
//! `OK` is teal, `WARN` is peach, and glyphs (`●`, `▶`, `!`) carry state too.

use ratatui::style::{Color, Modifier, Style};

// ---------------------------------------------------------------------------
// Catppuccin Mocha palette (https://catppuccin.com/palette)
// ---------------------------------------------------------------------------

pub const ROSEWATER: Color = Color::Rgb(0xf5, 0xe0, 0xdc);
pub const FLAMINGO: Color = Color::Rgb(0xf2, 0xcd, 0xcd);
pub const PINK: Color = Color::Rgb(0xf5, 0xc2, 0xe7);
pub const MAUVE: Color = Color::Rgb(0xcb, 0xa6, 0xf7);
pub const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
pub const MAROON: Color = Color::Rgb(0xeb, 0xa0, 0xac);
pub const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
pub const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
pub const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
pub const SKY: Color = Color::Rgb(0x89, 0xdc, 0xeb);
pub const SAPPHIRE: Color = Color::Rgb(0x74, 0xc7, 0xec);
pub const BLUE: Color = Color::Rgb(0x89, 0xb4, 0xfa);
pub const LAVENDER: Color = Color::Rgb(0xb4, 0xbe, 0xfe);

pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
pub const SUBTEXT1: Color = Color::Rgb(0xba, 0xc2, 0xde);
pub const SUBTEXT0: Color = Color::Rgb(0xa6, 0xad, 0xc8);
pub const OVERLAY2: Color = Color::Rgb(0x93, 0x99, 0xb2);
pub const OVERLAY1: Color = Color::Rgb(0x7f, 0x84, 0x9c);
pub const OVERLAY0: Color = Color::Rgb(0x6c, 0x70, 0x86);

pub const SURFACE2: Color = Color::Rgb(0x58, 0x5b, 0x70);
pub const SURFACE1: Color = Color::Rgb(0x45, 0x47, 0x5a);
pub const SURFACE0: Color = Color::Rgb(0x31, 0x32, 0x44);
pub const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
pub const MANTLE: Color = Color::Rgb(0x18, 0x18, 0x25);
pub const CRUST: Color = Color::Rgb(0x11, 0x11, 0x1b);

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Full-frame / pane / modal fill. Carries the default text color so every
/// unstyled span still meets the contrast contract.
pub const PANE: Style = Style::new().bg(BASE).fg(TEXT);
/// Top bar, bottom bar and pane toolbars.
pub const BAR: Style = Style::new().bg(MANTLE).fg(TEXT);

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Primary content.
pub const TEXT_STYLE: Style = Style::new().fg(TEXT);
/// Secondary content (log lines, inactive titles).
pub const SECONDARY: Style = Style::new().fg(SUBTEXT1);
/// De-emphasized but still AA-compliant text (hints, comments, zero values).
pub const MUTED: Style = Style::new().fg(OVERLAY2);
/// Structural lines only: borders, separators, table rules.
pub const FAINT: Style = Style::new().fg(OVERLAY0);

// ---------------------------------------------------------------------------
// Chrome / status roles
// ---------------------------------------------------------------------------

/// Section and table headings. Also the focus ring (`FOCUS`).
pub const HEADING: Style = Style::new().fg(YELLOW).add_modifier(Modifier::BOLD);
pub const FOCUS: Style = Style::new().fg(YELLOW).add_modifier(Modifier::BOLD);
/// Interactive affordances: buttons, dialog borders, addresses.
pub const INFO: Style = Style::new().fg(SAPPHIRE).add_modifier(Modifier::BOLD);
pub const INFO_PLAIN: Style = Style::new().fg(SAPPHIRE);
/// Success / enabled / register-changed.
pub const OK: Style = Style::new().fg(TEAL).add_modifier(Modifier::BOLD);
pub const OK_PLAIN: Style = Style::new().fg(TEAL);
/// Non-fatal warning.
pub const WARN: Style = Style::new().fg(PEACH).add_modifier(Modifier::BOLD);
pub const WARN_PLAIN: Style = Style::new().fg(PEACH);
/// Error / breakpoint / destructive action.
pub const ERR: Style = Style::new().fg(RED).add_modifier(Modifier::BOLD);
pub const ERR_PLAIN: Style = Style::new().fg(RED);
/// Labels and symbols.
pub const LABEL: Style = Style::new().fg(LAVENDER).add_modifier(Modifier::BOLD);

// ---------------------------------------------------------------------------
// Badges: accent background + dark foreground, never light-on-light
// ---------------------------------------------------------------------------

pub const BADGE_INFO: Style = Style::new()
    .fg(CRUST)
    .bg(SAPPHIRE)
    .add_modifier(Modifier::BOLD);
pub const BADGE_ON: Style = Style::new().fg(CRUST).bg(TEAL).add_modifier(Modifier::BOLD);
pub const BADGE_ACCENT: Style = Style::new()
    .fg(CRUST)
    .bg(YELLOW)
    .add_modifier(Modifier::BOLD);
pub const BADGE_ERR: Style = Style::new().fg(CRUST).bg(RED).add_modifier(Modifier::BOLD);
pub const BADGE_NEUTRAL: Style = Style::new()
    .fg(TEXT)
    .bg(SURFACE1)
    .add_modifier(Modifier::BOLD);
/// Selection in lists (explorer, file picker).
pub const SELECTED: Style = Style::new()
    .fg(CRUST)
    .bg(SAPPHIRE)
    .add_modifier(Modifier::BOLD);
/// Program counter in the listing address column.
pub const PC: Style = Style::new()
    .fg(CRUST)
    .bg(YELLOW)
    .add_modifier(Modifier::BOLD);

// ---------------------------------------------------------------------------
// Syntax tokens (rvs-syntax). Semantics are bitfield roles, not text formats.
// ---------------------------------------------------------------------------

/// Opcode / funct field.
pub const OP: Style = Style::new().fg(SAPPHIRE).add_modifier(Modifier::BOLD);
/// Destination register (`rd`).
pub const ARG1: Style = Style::new().fg(TEAL);
/// Source register (`rs1` / `rs2`).
pub const ARG2: Style = Style::new().fg(MAUVE);
/// Immediate / offset.
pub const ARG3: Style = Style::new().fg(PEACH);
/// Punctuation inside operands.
pub const PUNCT: Style = Style::new().fg(OVERLAY2);
pub const COMMENT: Style = Style::new().fg(OVERLAY2).add_modifier(Modifier::ITALIC);
/// String and character literals.
pub const STRING: Style = Style::new().fg(PINK);

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color) -> [f64; 3] {
        match c {
            Color::Rgb(r, g, b) => [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0],
            _ => panic!("theme colors must be RGB"),
        }
    }

    fn linearize(c: f64) -> f64 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Color) -> f64 {
        let [r, g, b] = rgb(c);
        0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Brettel/Viénot LMS projection; returns linear-light sRGB.
    fn simulate(c: Color, kind: &str) -> [f64; 3] {
        let [r, g, b] = rgb(c).map(linearize);
        let lms = [
            17.8824 * r + 43.5161 * g + 4.11935 * b,
            3.45565 * r + 27.1554 * g + 3.86714 * b,
            0.0299566 * r + 0.184309 * g + 1.46709 * b,
        ];
        let (l, m, s) = match kind {
            "deutan" => (lms[0], 0.494207 * lms[1] + 1.24827 * lms[2], lms[2]),
            "protan" => (2.02344 * lms[1] - 2.52581 * lms[2], lms[1], lms[2]),
            _ => unreachable!(),
        };
        [
            0.080944 * l - 0.130504 * m + 0.116721 * s,
            -0.0102485 * l + 0.0540194 * m - 0.113615 * s,
            -0.000365294 * l - 0.00412163 * m + 0.693513 * s,
        ]
        .map(|v| v.clamp(0.0, 1.0))
    }

    fn lab(v: [f64; 3]) -> [f64; 3] {
        let x = (0.4124 * v[0] + 0.3576 * v[1] + 0.1805 * v[2]) / 0.95047;
        let y = 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
        let z = (0.0193 * v[0] + 0.1192 * v[1] + 0.9505 * v[2]) / 1.08883;
        let f = |t: f64| {
            if t > 0.008856 {
                t.cbrt()
            } else {
                7.787 * t + 16.0 / 116.0
            }
        };
        let (fx, fy, fz) = (f(x), f(y), f(z));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    fn delta_e(a: Color, b: Color, kind: Option<&str>) -> f64 {
        let (pa, pb) = match kind {
            Some(k) => (simulate(a, k), simulate(b, k)),
            None => (rgb(a).map(linearize), rgb(b).map(linearize)),
        };
        let (la, lb) = (lab(pa), lab(pb));
        ((la[0] - lb[0]).powi(2) + (la[1] - lb[1]).powi(2) + (la[2] - lb[2]).powi(2)).sqrt()
    }

    fn min_distance(colors: &[Color], kind: Option<&str>) -> f64 {
        let mut min = f64::INFINITY;
        for (i, &a) in colors.iter().enumerate() {
            for &b in &colors[i + 1..] {
                min = min.min(delta_e(a, b, kind));
            }
        }
        min
    }

    #[test]
    fn text_meets_wcag_aa_on_every_surface_text_is_drawn_on() {
        for text in [TEXT, SUBTEXT1] {
            for bg in [BASE, MANTLE, SURFACE0] {
                let ratio = contrast(text, bg);
                assert!(ratio >= 4.5, "text contrast {ratio:.2} below AA");
            }
        }
        // Muted text is only used on the two darkest fills.
        for bg in [BASE, MANTLE] {
            let ratio = contrast(OVERLAY2, bg);
            assert!(ratio >= 4.5, "muted contrast {ratio:.2} below AA");
        }
        // Structural lines are non-text: WCAG 1.4.11 asks for 3:1.
        assert!(contrast(OVERLAY0, BASE) >= 3.0);
        // Neutral badge keeps body text legible.
        assert!(contrast(TEXT, SURFACE1) >= 4.5);
    }

    #[test]
    fn every_accent_is_legible_on_content_surfaces() {
        let accents = [YELLOW, SAPPHIRE, TEAL, PEACH, RED, MAUVE, LAVENDER, PINK];
        for accent in accents {
            for bg in [BASE, MANTLE, SURFACE0] {
                let ratio = contrast(accent, bg);
                assert!(ratio >= 4.5, "accent contrast {ratio:.2} below AA");
            }
        }
    }

    #[test]
    fn badges_use_dark_foreground_on_accent_backgrounds() {
        let badges = [
            (BADGE_INFO, SAPPHIRE),
            (BADGE_ON, TEAL),
            (BADGE_ACCENT, YELLOW),
            (BADGE_ERR, RED),
            (SELECTED, SAPPHIRE),
            (PC, YELLOW),
        ];
        for (style, bg) in badges {
            assert_eq!(style.fg, Some(CRUST));
            let ratio = contrast(CRUST, bg);
            assert!(ratio >= 4.5, "badge contrast {ratio:.2} below AA");
        }
    }

    #[test]
    fn semantic_accents_stay_distinguishable_with_cvd() {
        // Colors that can share one line of text or one status bar.
        let chrome = [YELLOW, SAPPHIRE, TEAL, PEACH, RED, MAUVE];
        let syntax = [SAPPHIRE, TEAL, MAUVE, PEACH, YELLOW, LAVENDER, PINK];
        for kind in [None, Some("deutan"), Some("protan")] {
            let chrome_min = min_distance(&chrome, kind);
            let syntax_min = min_distance(&syntax, kind);
            assert!(
                chrome_min >= 10.0,
                "chrome colors collide under {kind:?}: ΔE {chrome_min:.1}"
            );
            assert!(
                syntax_min >= 7.0,
                "syntax colors collide under {kind:?}: ΔE {syntax_min:.1}"
            );
        }
    }
}
