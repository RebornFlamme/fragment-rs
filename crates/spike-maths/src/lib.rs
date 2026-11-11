//! Spike S5 (crate JETABLE, supprimée ou promue par `fr-t3-widgets-inline`) : rendre `$…$` et
//! `$$…$$` au plus près de MathJax, tel que Fragment les rend
//! (app/src/ui/views/markdown/mathjax.ts et la règle `.cm-math-*` d'editor.css).
//!
//! Trois voies sur les mêmes formules ([`formules::FORMULES`]), toutes rastérisées par resvg
//! puis comparées pixel à pixel aux captures de Fragment ([`reference`]) :
//! - (A) [`voie_a`] : MathJax 3.2.1 lui-même, dans QuickJS ;
//! - (B) [`voie_b`] : un moteur TeX en pur Rust (latex-rust ; `rex` n'existe pas, voir le module) ;
//! - (C) [`voie_c`] : la math de typst.
//!
//! Mesures et verdict : docs/spikes/S5-maths.md ; décision : docs/decisions/ADR-011-maths.md.

pub mod diff;
pub mod formules;
pub mod raster;
pub mod reference;
#[cfg(feature = "voie-a")]
pub mod voie_a;
#[cfg(feature = "voie-b")]
pub mod voie_b;
#[cfg(feature = "voie-c")]
pub mod voie_c;

/// Les erreurs du spike.
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
