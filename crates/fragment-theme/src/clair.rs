//! Le thème clair : ce que `.theme-light` de `index.css` déclare par-dessus `:root` et `body`.
//!
//! Même principe que `sombre.rs` : une table de déclarations résolue par [`crate::cascade`]. Le
//! clair ne REDÉCLARE que ce qui diffère (accent, fonds, textes, deux gris, ombres, avatars) ;
//! `--text-accent` y devient un alias de l'accent et les autres dérivés de `body` restent ceux du
//! sombre, recalculés sur le texte clair.

use crate::jetons_css::Declaration;

/// Les déclarations propres au thème clair (`.theme-light`), par-dessus `ROOT` et `BODY`.
pub const JETONS: &[Declaration] = &[
    ("--color-accent", "#4b52d6", 301),
    ("--text-accent", "var(--color-accent)", 302),
    ("--tab-bar-bg", "#e8e7e1", 304),
    ("--background-secondary", "#efeee9", 305),
    ("--background-primary", "#f7f6f2", 306),
    ("--background-primary-alt", "#ffffff", 307),
    ("--menu-bg", "#ffffff", 308),
    ("--background-secondary-alt", "#ffffff", 309),
    ("--tab-outline-color", "#d3d2cb", 310),
    ("--text-normal", "#1a1b1e", 312),
    ("--text-muted", "#5c5b57", 313),
    ("--text-faint", "#8a8984", 314),
    ("--text-error", "#c93c34", 315),
    ("--text-warning", "#9a6700", 316),
    ("--text-success", "#2f8a5f", 317),
    (
        "--background-modifier-border",
        "color-mix(in srgb, var(--text-normal) 10%, transparent)",
        319,
    ),
    (
        "--background-modifier-hover",
        "color-mix(in srgb, var(--text-normal) 5%, transparent)",
        320,
    ),
    ("--menu-shadow", "0 6px 20px rgba(17, 18, 20, 0.12)", 322),
    ("--shadow-xs", "0 0.5px 1.5px rgba(17, 18, 20, 0.15)", 323),
    ("--shadow-s", "0 2px 8px rgba(17, 18, 20, 0.08)", 324),
    (
        "--shadow-card",
        "0 1px 2px rgba(17, 18, 20, 0.04), 0 6px 20px rgba(17, 18, 20, 0.06)",
        325,
    ),
    ("--shadow-l", "0 16px 48px rgba(17, 18, 20, 0.16)", 326),
    ("--avatar-bg-l", "0.91", 328),
    ("--avatar-bg-c", "0.045", 329),
    ("--avatar-fg-l", "0.41", 330),
    ("--avatar-fg-c", "0.085", 331),
];
