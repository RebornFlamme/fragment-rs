//! Les captures de Fragment (`reference/*.png`) et leurs mesures (`reference/mesures.json`),
//! produites par `capture/maths.spec.ts` sur le portable (verrou des pixels, Fragment intact).

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::Erreur;

/// Le rectangle rogné de la capture, en px entiers de la fenêtre.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Clip {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Une capture : la formule, le thème et ce que Chromium a mesuré autour du `<svg>` de MathJax.
#[derive(Clone, Debug, Deserialize)]
pub struct Mesure {
    pub nom: String,
    pub formule: String,
    pub theme: String,
    pub bloc: bool,
    pub tex: String,
    pub clip: Clip,
    /// Rectangle exact du `<svg>` (px CSS, fractionnaire : unités de mise en page de 1/64).
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
    /// 1 ex et 1 em du conteneur MathJax (DM Sans au corps de l'éditeur).
    pub ex_px: f64,
    pub em_px: f64,
    pub couleur: String,
    pub fond: String,
    /// Le `<svg>` tel que MathJax l'a mis dans le DOM de Fragment.
    pub svg: String,
}

/// Le dossier `reference/` de la crate.
pub fn dossier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("reference")
}

/// Toutes les mesures, dans l'ordre de la capture (sombre puis clair).
pub fn mesures() -> Result<Vec<Mesure>, Erreur> {
    let texte = std::fs::read_to_string(dossier().join("mesures.json"))?;
    Ok(serde_json::from_str(&texte)?)
}

/// La capture PNG d'une mesure.
pub fn image(m: &Mesure) -> Result<image::RgbImage, Erreur> {
    Ok(image::open(dossier().join(format!("{}.png", m.nom)))?.to_rgb8())
}

/// `rgb(r, g, b)` → `[r, g, b]`.
pub fn couleur(css: &str) -> Result<[u8; 3], Erreur> {
    let dedans = css
        .trim()
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| Erreur::Format(format!("couleur : {css}")))?;
    let mut sortie = [0u8; 3];
    let mut parts = dedans.split(',');
    for c in &mut sortie {
        *c = parts
            .next()
            .and_then(|p| p.trim().parse().ok())
            .ok_or_else(|| Erreur::Format(format!("couleur : {css}")))?;
    }
    Ok(sortie)
}
