//! PDF : géométrie, zoom, outline, recherche, modèle des positions de `PdfEditor`
//! (pdfium-render en phase 3).
//!
//! Modèle PUR uniquement (program.md §5, phase 1) : aucun moteur PDF, aucune vue.
//! Le rendu réel (pages, texte, outline) viendra avec `pdfium-render` ; ici, les
//! calculs que Fragment avait isolés du DOM et de pdf.js.

pub mod pdf_geometry;
pub mod pdf_outline;
pub mod pdf_zoom;
