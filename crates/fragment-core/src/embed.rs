// Port de app/src/ui/core/embed/embedDocument.ts (la RÉSOLUTION : `embedKind`) et de
// `wheelPixels` (embed/embedFrame.ts, testée dans le même fichier de test TS).
//
// EMBARQUER UN DOCUMENT PAR SA CLÉ — un fichier du coffre (`notes/a.md`, `doc.pdf`) ou le
// document d'un plugin (`chat:agent/…`) :
//   1. une fabrique d'embed pour son extension (`app.embedRegistry`) → elle ;
//   2. sinon → LA VUE ELLE-MÊME, hébergée : celle que son fournisseur ouvre
//      (`documents.view_state_for`). ★ Un plugin qui enregistre une vue s'embarque donc
//      SEUL, sans une ligne de code de plus.
//
// Le montage (cadre, vue hébergée, DOM) n'a pas de pendant ici : il est dans fragment-ui
// (`embed_document` de docs/contrats.md). Ce module garde ce qui est pur.

use crate::document_key::is_vault_key;
use crate::workspace::ViewState;

/// Profondeur max d'embeds imbriqués (note dans note dans note…) avant de couper.
pub const MAX_EMBED_DEPTH: usize = 4;

/// Le type de la vue vide (`EMPTY_VIEW_TYPE` de EmptyView.ts) : « rien ne sait montrer ça ».
pub const EMPTY_VIEW_TYPE: &str = "empty";

/// Ce que `embed_kind` lit de l'App : le coffre, le registre d'embeds, les documents.
pub trait EmbedHost {
    /// L'extension du fichier du coffre à ce chemin (sans point, casse d'origine), s'il existe.
    fn vault_file_extension(&self, path: &str) -> Option<String>;
    /// Existe-t-il une fabrique d'embed pour cette extension (déjà en minuscules) ?
    fn has_embed_creator(&self, extension: &str) -> bool;
    /// `documents.view_state_for(key)`.
    fn view_state_for(&self, key: &str) -> Option<ViewState>;
}

/// Ce qui montrera ce document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedKind {
    /// Une fabrique d'embed.
    Creator,
    /// La vue du document, hébergée.
    View,
}

/// Une fabrique d'embed, une vue, ou rien (`None` : l'hôte fait son repli). Pur.
pub fn embed_kind(host: &dyn EmbedHost, key: &str) -> Option<EmbedKind> {
    if is_vault_key(key) {
        let extension = host.vault_file_extension(key)?;
        if host.has_embed_creator(&extension.to_lowercase()) {
            return Some(EmbedKind::Creator);
        }
    }
    match host.view_state_for(key) {
        Some(vs) if vs.type_ != EMPTY_VIEW_TYPE => Some(EmbedKind::View),
        _ => None,
    }
}

/// Un évènement de molette, réduit à ce que lit `wheel_pixels`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelDelta {
    pub delta_y: f64,
    /// 0 = pixels, 1 = lignes, 2 = pages (`WheelEvent.deltaMode`).
    pub delta_mode: u8,
}

/// Le delta d'une molette en pixels (deltaMode : pixels, lignes, pages).
pub fn wheel_pixels(e: WheelDelta, page_height: f64) -> f64 {
    match e.delta_mode {
        1 => e.delta_y * 16.0,
        2 => e.delta_y * page_height,
        _ => e.delta_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Un coffre réduit à ce qu'embed_kind lit : fichiers, fabriques d'embed, fournisseurs.
    struct App {
        fichiers: Vec<&'static str>,
        fabriques: Vec<&'static str>,
        vues: HashMap<&'static str, &'static str>,
    }

    impl EmbedHost for App {
        fn vault_file_extension(&self, path: &str) -> Option<String> {
            self.fichiers
                .contains(&path)
                .then(|| path.rsplit('.').next().unwrap_or("").to_owned())
        }
        fn has_embed_creator(&self, ext: &str) -> bool {
            self.fabriques.contains(&ext)
        }
        fn view_state_for(&self, k: &str) -> Option<ViewState> {
            let ext = match k.split_once(':') {
                Some((schema, _)) => format!("{schema}:"),
                None => k.rsplit('.').next().unwrap_or("").to_owned(),
            };
            (k.contains(':') || self.fichiers.contains(&k))
                .then(|| ViewState::new(self.vues.get(ext.as_str()).copied().unwrap_or("empty")))
        }
    }

    fn app() -> App {
        App {
            fichiers: vec!["n.md", "img.png", "doc.pdf", "x.zip"],
            fabriques: vec!["md", "png"],
            vues: HashMap::from([("pdf", "pdf"), ("chat:", "chat")]),
        }
    }

    #[test]
    fn une_fabrique_dembed_pour_lextension_note_image_elle() {
        let a = app();
        assert_eq!(embed_kind(&a, "n.md"), Some(EmbedKind::Creator));
        assert_eq!(embed_kind(&a, "img.png"), Some(EmbedKind::Creator));
    }

    #[test]
    fn sinon_la_vue_du_document_sans_code_par_vue_pdf_document_de_plugin() {
        let a = app();
        assert_eq!(embed_kind(&a, "doc.pdf"), Some(EmbedKind::View));
        assert_eq!(embed_kind(&a, "chat:agent/abc"), Some(EmbedKind::View));
    }

    #[test]
    fn ni_fabrique_ni_vue_vue_vide_ou_introuvable_rien_lhote_fait_son_repli() {
        let a = app();
        assert_eq!(embed_kind(&a, "x.zip"), None);
        assert_eq!(embed_kind(&a, "absent.md"), None);
    }

    #[test]
    fn pixels_lignes_pages() {
        let d = |delta_y, delta_mode| WheelDelta {
            delta_y,
            delta_mode,
        };
        assert_eq!(wheel_pixels(d(30.0, 0), 500.0), 30.0);
        assert_eq!(wheel_pixels(d(3.0, 1), 500.0), 48.0);
        assert_eq!(wheel_pixels(d(1.0, 2), 500.0), 500.0);
    }

    // ─── Ajouté ───
    #[test]
    fn lextension_est_comparee_en_minuscules() {
        struct Maj;
        impl EmbedHost for Maj {
            fn vault_file_extension(&self, _: &str) -> Option<String> {
                Some("PNG".into())
            }
            fn has_embed_creator(&self, ext: &str) -> bool {
                ext == "png"
            }
            fn view_state_for(&self, _: &str) -> Option<ViewState> {
                None
            }
        }
        assert_eq!(embed_kind(&Maj, "A.PNG"), Some(EmbedKind::Creator));
    }
}
