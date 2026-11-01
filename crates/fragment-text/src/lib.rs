//! TextSurface/DocumentSurface/SurfaceText, positions et plages, mapRange/mapAnchor,
//! rangeHighlights, viewport, findPassage, revealPassage, textDelta, modèle de texte HTML.
//!
//! Port des modules PURS de `app/src/ui/core/editor/` (et de leurs voisins) de
//! Fragment. Rien ici ne connaît une vue par son nom ni un moteur de rendu :
//! la crate parle en positions opaques, en rectangles document-relatifs et en
//! traits que les surfaces implémentent (`DocumentSurface`, `TextSurface`,
//! `Editor`).
//!
//! ★ Les positions qui SONT des index dans un texte sont des index d'OCTETS
//!   UTF-8 (le TS comptait des unités UTF-16) : voir `editor.rs`.

pub mod editor;
pub mod find_passage;
pub mod html_text_model;
pub mod map_anchor;
pub mod range_highlights;
pub mod text_delta;
pub mod viewport;
pub mod visibility;

pub use editor::{
    Assoc, ChangeOrigin, DocumentSurface, Editor, EditorChange, EditorChangeSpec, EditorPosition,
    EditorRange, EditorSelectionSpec, EditorTransactionSpec, FullText, MapPos, MappedPos,
    MappedRange, Rect, SurfaceText, TextSurface, full_text, has_text, is_editable, map_range,
};
pub use find_passage::{PassageError, TextMatch, find_in_text, find_passage, passage_error, pick};
pub use html_text_model::{PAGE_CORNER, PageText, PageTextDiff, diff_page_text};
pub use map_anchor::{MappedAnchor, TextAnchored, map_anchor};
pub use range_highlights::{HighlightBox, HighlightHandle, HighlightRange, RangeHighlights};
pub use text_delta::{MappedChar, TextDelta, map_through_delta, text_delta};
pub use viewport::{Bande, ViewportBand, Vp, bande, bande_appropriee, biais};
pub use visibility::{PosVisibility, pos_visibility};
