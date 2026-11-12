//! Le rendu des maths `$…$` et `$$…$$` (ADR-011), promu du spike S5 (`fr-t3-widgets-inline`).
//!
//! - [`mathjax`] : MathJax 3.2.1 (la version de Fragment, mêmes options) dans QuickJS, sans DOM ;
//!   il produit le SVG et sa boîte, au 1/64 px de Chromium ;
//! - [`raster`] : la boîte d'une formule ([`raster::RenduSvg`]) et la rastérisation par resvg ;
//! - [`reference`], [`diff`], [`formules`] : les captures de Fragment et leur comparaison pixel à
//!   pixel, pour les tests.
//!
//! Mesures et verdict : docs/spikes/S5-maths.md ; décision : docs/decisions/ADR-011-maths.md.
//! Ne dépend pas de gpui-kit.

pub mod diff;
pub mod formules;
pub mod mathjax;
pub mod moteur;
pub mod raster;
pub mod reference;

/// Les erreurs du rendu.
#[derive(Debug, thiserror::Error)]
pub enum Erreur {
    #[error("lecture : {0}")]
    Io(#[from] std::io::Error),
    #[error("json : {0}")]
    Json(#[from] serde_json::Error),
    #[error("image : {0}")]
    Image(#[from] image::ImageError),
    #[error("format : {0}")]
    Format(String),
    #[error("moteur : {0}")]
    Moteur(String),
}
