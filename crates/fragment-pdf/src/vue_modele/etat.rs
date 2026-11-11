// Port de app/src/ui/views/pdf/PdfView.ts (getEphemeralState, setEphemeralState,
// lirePosition, allerAPosition)
//
// La position de lecture et l'état éphémère de la vue : ce qui survit à un
// changement d'onglet sans entrer dans le fichier de mise en page.

use serde_json::{Value, json};

use crate::pdf_zoom::ZoomMode;

/// Où l'on en est dans le document, indépendamment du zoom : la page sous le haut
/// de la zone, la fraction de hauteur dans cette page, la fraction horizontale du
/// défilement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionLecture {
    /// À partir de 1.
    pub page: usize,
    pub fy: f64,
    pub fx: f64,
}

impl PositionLecture {
    pub fn vers_json(&self) -> Value {
        json!({ "page": self.page, "fy": self.fy, "fx": self.fx })
    }

    /// Lit `{page, fy, fx}` ; `fy` et `fx` valent 0 s'ils manquent. `None` sans `page` numérique.
    pub fn depuis_json(v: &Value) -> Option<Self> {
        let page = entier_positif(v.get("page")?)?;
        let lire = |cle: &str| v.get(cle).and_then(Value::as_f64).unwrap_or(0.0);
        Some(Self {
            page,
            fy: lire("fy"),
            fx: lire("fx"),
        })
    }
}

/// Un nombre JSON entier >= 0 (les flottants entiers, comme `3.0`, passent).
pub(crate) fn entier_positif(v: &Value) -> Option<usize> {
    if let Some(n) = v.as_u64() {
        return usize::try_from(n).ok();
    }
    let f = v.as_f64()?;
    (f >= 0.0 && f.fract() == 0.0 && f < 1e15).then_some(f as usize)
}

/// Le nom d'un mode dans l'état sauvegardé (mêmes chaînes que pdf.js).
pub fn mode_vers_texte(mode: ZoomMode) -> &'static str {
    match mode {
        ZoomMode::Auto => "auto",
        ZoomMode::PageActual => "page-actual",
        ZoomMode::PageFit => "page-fit",
        ZoomMode::PageWidth => "page-width",
    }
}

pub fn mode_depuis_texte(texte: &str) -> Option<ZoomMode> {
    match texte {
        "auto" => Some(ZoomMode::Auto),
        "page-actual" => Some(ZoomMode::PageActual),
        "page-fit" => Some(ZoomMode::PageFit),
        "page-width" => Some(ZoomMode::PageWidth),
        _ => None,
    }
}

/// La part de l'état éphémère que porte le modèle (la barre latérale et la
/// recherche ont le leur, assemblé par la vue montée).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EtatEphemere {
    pub page: usize,
    pub scale: f64,
    /// `None` : échelle libre.
    pub zoom_mode: Option<ZoomMode>,
}

impl EtatEphemere {
    /// Clés `page`, `scale`, `zoomMode` (nul si l'échelle est libre).
    pub fn vers_json(&self) -> Value {
        json!({
            "page": self.page,
            "scale": self.scale,
            "zoomMode": self.zoom_mode.map(mode_vers_texte),
        })
    }
}
