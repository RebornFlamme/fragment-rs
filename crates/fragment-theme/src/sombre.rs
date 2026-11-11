//! Le thème sombre : ce que `.theme-dark` de `index.css` déclare par-dessus `:root` et `body`.
//!
//! Ce n'est plus une palette écrite à la main : c'est une table de déclarations que le graphe de
//! [`crate::cascade`] résout (alias, `color-mix`, `calc`). Le résultat reste, au bit près, celui de
//! l'ancienne palette (test `egalite_bit_a_bit_sans_surcharge`, instantané figé dans `tests/figes/`).

use crate::jetons_css::Declaration;

/// Les déclarations propres au thème sombre (`.theme-dark`), par-dessus `ROOT` et `BODY`.
pub const JETONS: &[Declaration] = &[
    ("--color-accent", "#646cff", 265),
    ("--tab-bar-bg", "#0e0f11", 267),
    ("--background-secondary", "#111214", 268),
    ("--background-primary", "#141517", 269),
    ("--background-primary-alt", "#191a1e", 270),
    ("--menu-bg", "#1c1d21", 271),
    ("--background-secondary-alt", "#222328", 272),
    ("--tab-outline-color", "#34363c", 273),
    ("--text-normal", "#dfded9", 276),
    ("--text-muted", "#a3a29d", 277),
    ("--text-faint", "#6f6e6a", 278),
    ("--text-error", "#e5534b", 279),
    ("--text-warning", "#d29922", 280),
    ("--text-success", "#4caf82", 281),
    ("--menu-shadow", "0 8px 24px rgba(0, 0, 0, 0.55)", 283),
    ("--shadow-xs", "0 0.5px 1.5px rgba(0, 0, 0, 0.22)", 286),
    ("--shadow-s", "0 2px 8px rgba(0, 0, 0, 0.4)", 287),
    (
        "--shadow-card",
        "0 1px 2px rgba(0, 0, 0, 0.06), 0 6px 20px rgba(0, 0, 0, 0.08)",
        288,
    ),
    ("--shadow-l", "0 16px 48px rgba(0, 0, 0, 0.6)", 289),
    ("--avatar-bg-l", "0.37", 292),
    ("--avatar-bg-c", "0.07", 293),
    ("--avatar-fg-l", "0.9", 294),
    ("--avatar-fg-c", "0.05", 295),
];
