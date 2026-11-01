// Port de app/src/ui/views/agent/side-chat/textAccess.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Lire n'importe quelle TextSurface — une note comme un PDF — par son contrat
//  seul (get_line / line_count / offset_to_pos ; get_value/get_range quand elle est
//  éditable). Aucun type de vue n'est nommé ici.
//
//  ★ Les positions sont des index en unités UTF-16 (ADR-004), comme en TS : les
//    `ch` et les offsets sont convertis ici quand on découpe un `&str`.
// ═══════════════════════════════════════════════════════════════════════════

use crate::side_chat_prompt::DocumentSource;
use crate::utf16::{len_utf16, slice_utf16};

/// Une position {ligne, colonne} ; `ch` en unités UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub ch: usize,
}

/// Le minimum de `TextSurface` / `Editor` lu ici.
// TODO(contrat) : remplacer par fragment_text::TextSurface (et son extension éditable).
pub trait SurfaceLecture {
    fn line_count(&self) -> usize;
    fn get_line(&self, n: usize) -> String;
    fn offset_to_pos(&self, offset: usize) -> Pos;
    /// `Some` quand la surface est éditable (`isEditable`) : le texte entre deux offsets.
    fn get_range(&self, _from: usize, _to: usize) -> Option<String> {
        None
    }
    /// `Some` quand la surface est éditable : tout son texte.
    fn get_value(&self) -> Option<String> {
        None
    }
    /// Le texte entier d'une surface non éditable (`getText`) ; `None` = pas encore.
    fn get_text(&self) -> Option<String> {
        None
    }
    /// Le fichier EST la source (une page HTML) : l'agent le lit.
    fn file_is_source(&self) -> bool {
        false
    }
}

/// Ce que `documentSource` demande à l'app.
// TODO(contrat) : remplacer par fragment_core::App (documents.title, vault.adapter).
pub trait HoteDocuments {
    /// Le titre d'une clé de document hors coffre (une conversation).
    fn title(&self, key: &str) -> String;
    /// La racine disque du coffre (adaptateur de fichiers) ; `None` si l'adaptateur n'en a pas.
    fn base_path(&self) -> Option<String>;
}

/// Un chemin du coffre (sans schéma) ? (`parseDocumentKey(key).scheme === ''`)
pub fn is_vault_key(key: &str) -> bool {
    !matches!(key.find(':'), Some(i) if i > 0)
}

/// Le texte entre deux offsets, par unités quand la surface n'expose pas `get_range`.
pub fn text_between(surface: &dyn SurfaceLecture, from: usize, to: usize) -> String {
    if to <= from {
        return String::new();
    }
    if let Some(t) = surface.get_range(from, to) {
        return t;
    }
    let a = surface.offset_to_pos(from);
    let b = surface.offset_to_pos(to);
    let mut parts: Vec<String> = Vec::new();
    for n in a.line..=b.line {
        let t = surface.get_line(n);
        let debut = if n == a.line { a.ch } else { 0 };
        let fin = if n == b.line { b.ch } else { len_utf16(&t) };
        parts.push(slice_utf16(&t, debut, fin).to_string());
    }
    parts.join("\n")
}

/// Les unités de la surface (lignes d'une note, pages d'un PDF), dans l'ordre.
fn unites(surface: &dyn SurfaceLecture) -> Vec<String> {
    (0..surface.line_count())
        .map(|i| surface.get_line(i))
        .collect()
}

/// Le document tel que l'agent le reçoit : son texte quand on l'a en entier ;
/// sinon (pages d'un PDF pas encore lues) le fichier lui-même, que l'agent lit.
///
/// `path` est la CLÉ du document ; hors du coffre (une conversation), il n'y a
/// pas de fichier à lire et l'agent voit son TITRE — le texte est alors toujours
/// complet.
pub fn document_source(
    app: &dyn HoteDocuments,
    surface: &dyn SurfaceLecture,
    path: &str,
) -> DocumentSource {
    let label = if is_vault_key(path) {
        path.to_string()
    } else {
        app.title(path)
    };
    if surface.file_is_source() && is_vault_key(path) {
        return fichier(app, path);
    }
    if let Some(valeur) = surface.get_value() {
        return DocumentSource::Text {
            path: label,
            text: valeur,
        };
    }
    if let Some(entier) = surface.get_text() {
        return DocumentSource::Text {
            path: label,
            text: entier,
        };
    }
    let u = unites(surface);
    if !is_vault_key(path) || u.iter().all(|t| !t.trim().is_empty()) {
        return DocumentSource::Text {
            path: label,
            text: u.join("\n\n"),
        };
    }
    fichier(app, path)
}

/// Le fichier lui-même, que l'agent lit (Read) — un PDF pas encore peint, la source d'une page HTML.
pub fn fichier(app: &dyn HoteDocuments, path: &str) -> DocumentSource {
    let base = app.base_path().unwrap_or_default();
    let absolute_path = if base.is_empty() {
        path.to_string()
    } else {
        format!("{}/{path}", base.trim_end_matches(['/', '\\']))
    };
    DocumentSource::File {
        path: path.to_string(),
        absolute_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une surface textuelle en lecture seule (une conversation, un PDF) : une ligne par unité.
    struct Lecture {
        units: Vec<String>,
        complet: bool,
    }

    fn lecture(units: &[&str], complet: bool) -> Lecture {
        Lecture {
            units: units.iter().map(|s| s.to_string()).collect(),
            complet,
        }
    }

    impl SurfaceLecture for Lecture {
        fn line_count(&self) -> usize {
            self.units.len()
        }
        fn get_line(&self, n: usize) -> String {
            self.units.get(n).cloned().unwrap_or_default()
        }
        fn offset_to_pos(&self, o: usize) -> Pos {
            Pos {
                line: o / 1000,
                ch: o % 1000,
            }
        }
        fn get_text(&self) -> Option<String> {
            if self.complet {
                Some(self.units.join("\n\n"))
            } else {
                None
            }
        }
    }

    struct App(&'static str);

    impl HoteDocuments for App {
        fn title(&self, _key: &str) -> String {
            self.0.to_string()
        }
        fn base_path(&self) -> Option<String> {
            None
        }
    }

    #[test]
    fn une_conversation_le_texte_entier_etiquete_par_son_titre_pas_de_fichier_a_lire() {
        let d = document_source(
            &App("Ma conversation"),
            &lecture(&["Question", "Réponse"], true),
            "chat:agent/x",
        );
        assert_eq!(
            d,
            DocumentSource::Text {
                path: "Ma conversation".into(),
                text: "Question\n\nRéponse".into()
            }
        );
    }

    #[test]
    fn un_pdf_du_coffre_ses_pages_si_toutes_sont_lues_sinon_le_fichier() {
        assert_eq!(
            document_source(&App("x"), &lecture(&["p1", "p2"], false), "a.pdf"),
            DocumentSource::Text {
                path: "a.pdf".into(),
                text: "p1\n\np2".into()
            }
        );
        assert!(
            matches!(document_source(&App("x"), &lecture(&["p1", ""], false), "a.pdf"), DocumentSource::File { ref path, .. } if path == "a.pdf")
        );
    }

    #[test]
    fn text_between_par_unites_en_utf16() {
        let s = lecture(&["a😀bc", "déf"], false);
        // de (ligne 0, ch 1) à (ligne 1, ch 2) : « 😀bc » + « dé »
        assert_eq!(text_between(&s, 1, 1002), "😀bc\ndé");
        assert_eq!(text_between(&s, 5, 5), "");
    }
}
