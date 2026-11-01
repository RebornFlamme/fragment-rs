// Port de app/src/ui/core/workspace/LeafHistory.ts
//
// L'historique de navigation d'UNE feuille — les boutons ← → du view-header. Deux
// piles, la mécanique d'un navigateur : naviguer empile en arrière et VIDE l'avant ;
// revenir déplace l'état courant d'une pile à l'autre.
//
// Pur (aucun Vault, aucun DOM) : `is_alive` est injecté pour sauter les entrées dont le
// document a disparu depuis.

use serde_json::Value;

use crate::document_key::{DocumentKey, move_key};
use crate::workspace::ViewState;

/// Une entrée d'historique de navigation : ce que la feuille affichait.
///
/// ★ `doc` est la CLÉ du document : une note, un PDF, une conversation… L'historique
///   navigue entre DOCUMENTS, pas seulement entre fichiers. Un renommage la recale
///   (`rename_document`, branché par le Workspace sur le flux des documents) ; au
///   retour, l'état de vue est relu chez le fournisseur du document.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub state: ViewState,
    pub e_state: Option<Value>,
    pub doc: Option<DocumentKey>,
}

/// Au-delà, la plus vieille entrée tombe — un historique n'est pas un journal.
pub const HISTORY_MAX: usize = 50;

pub struct LeafHistory {
    pub back_history: Vec<HistoryEntry>,
    pub forward_history: Vec<HistoryEntry>,
    is_alive: Box<dyn Fn(&str) -> bool>,
}

impl Default for LeafHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LeafHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeafHistory")
            .field("back_history", &self.back_history)
            .field("forward_history", &self.forward_history)
            .finish()
    }
}

impl LeafHistory {
    /// Historique où tout document est vivant.
    pub fn new() -> Self {
        Self::with_is_alive(|_| true)
    }

    pub fn with_is_alive(is_alive: impl Fn(&str) -> bool + 'static) -> Self {
        Self {
            back_history: Vec::new(),
            forward_history: Vec::new(),
            is_alive: Box::new(is_alive),
        }
    }

    /// Une navigation neuve : l'état quitté part en arrière, l'avant n'a plus de sens.
    pub fn push(&mut self, entry: HistoryEntry) {
        self.back_history.push(entry);
        if self.back_history.len() > HISTORY_MAX {
            self.back_history.remove(0);
        }
        self.forward_history.clear();
    }

    pub fn can_go_back(&self) -> bool {
        self.back_history.iter().any(|e| self.valide(e))
    }

    pub fn can_go_forward(&self) -> bool {
        self.forward_history.iter().any(|e| self.valide(e))
    }

    /// La cible du retour (ou `None`) ; `current` part dans l'avant.
    pub fn take_back(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        Self::deplacer(
            &*self.is_alive,
            &mut self.back_history,
            &mut self.forward_history,
            current,
        )
    }

    /// La cible de l'avance (ou `None`) ; `current` part dans l'arrière.
    pub fn take_forward(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        Self::deplacer(
            &*self.is_alive,
            &mut self.forward_history,
            &mut self.back_history,
            current,
        )
    }

    /// Un document (ou un dossier : tout ce qui est dessous) a été renommé.
    pub fn rename_document(&mut self, old_base: &str, new_base: &str) {
        for e in self
            .back_history
            .iter_mut()
            .chain(self.forward_history.iter_mut())
        {
            if let Some(moved) = e.doc.as_ref().and_then(|d| move_key(d, old_base, new_base)) {
                e.doc = Some(moved);
            }
        }
    }

    fn deplacer(
        is_alive: &dyn Fn(&str) -> bool,
        source: &mut Vec<HistoryEntry>,
        cible: &mut Vec<HistoryEntry>,
        current: HistoryEntry,
    ) -> Option<HistoryEntry> {
        // Les entrées mortes (document supprimé) sont jetées en passant.
        let mut entry = source.pop();
        while let Some(e) = &entry {
            if Self::valide_avec(is_alive, e) {
                break;
            }
            entry = source.pop();
        }
        let entry = entry?;
        cible.push(current);
        Some(entry)
    }

    fn valide(&self, entry: &HistoryEntry) -> bool {
        Self::valide_avec(&*self.is_alive, entry)
    }

    fn valide_avec(is_alive: &dyn Fn(&str) -> bool, entry: &HistoryEntry) -> bool {
        entry.doc.as_ref().is_none_or(|d| is_alive(d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entree(doc: &str) -> HistoryEntry {
        HistoryEntry {
            state: ViewState::with_state("markdown", json!({"file": doc})),
            e_state: None,
            doc: Some(DocumentKey::new(doc)),
        }
    }

    fn doc_of(e: Option<HistoryEntry>) -> Option<String> {
        e.and_then(|e| e.doc).map(DocumentKey::into_string)
    }

    fn docs(v: &[HistoryEntry]) -> Vec<String> {
        v.iter()
            .map(|e| e.doc.clone().unwrap().into_string())
            .collect()
    }

    #[test]
    fn vide_au_depart_ni_arriere_ni_avant() {
        let mut h = LeafHistory::new();
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
        assert!(h.take_back(entree("a.md")).is_none());
    }

    #[test]
    fn arriere_puis_avant_rejouent_les_etats_dans_lordre() {
        let mut h = LeafHistory::new();
        h.push(entree("a.md")); // on quitte a pour b
        h.push(entree("b.md")); // on quitte b pour c
        assert_eq!(doc_of(h.take_back(entree("c.md"))).as_deref(), Some("b.md"));
        assert_eq!(doc_of(h.take_back(entree("b.md"))).as_deref(), Some("a.md"));
        assert!(!h.can_go_back());
        assert_eq!(
            doc_of(h.take_forward(entree("a.md"))).as_deref(),
            Some("b.md")
        );
        assert_eq!(
            doc_of(h.take_forward(entree("b.md"))).as_deref(),
            Some("c.md")
        );
        assert!(!h.can_go_forward());
    }

    #[test]
    fn une_navigation_neuve_vide_lavant() {
        let mut h = LeafHistory::new();
        h.push(entree("a.md"));
        h.take_back(entree("b.md")); // b part dans l'avant
        assert!(h.can_go_forward());
        h.push(entree("a.md"));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn saute_les_entrees_dont_le_document_a_disparu() {
        let mut h = LeafHistory::with_is_alive(|d| d != "b.md");
        h.push(entree("a.md"));
        h.push(entree("b.md"));
        assert_eq!(doc_of(h.take_back(entree("c.md"))).as_deref(), Some("a.md"));
    }

    #[test]
    fn note_conversation_des_documents_de_schemas_differents() {
        let mut h = LeafHistory::new();
        h.push(HistoryEntry {
            state: ViewState::with_state("chat", json!({"provider": "agent", "conversation": "x"})),
            e_state: None,
            doc: Some(DocumentKey::new("chat:agent/x")),
        });
        assert_eq!(
            doc_of(h.take_back(entree("n.md"))).as_deref(),
            Some("chat:agent/x")
        );
    }

    #[test]
    fn rename_document_recale_le_document_et_ce_qui_est_sous_un_dossier() {
        let mut h = LeafHistory::new();
        h.push(entree("dir/a.md"));
        h.push(entree("b.md"));
        h.take_back(entree("dir/c.md")); // dir/c.md part dans l'avant
        h.rename_document("dir", "neuf");
        h.rename_document("b.md", "bb.md");
        assert_eq!(docs(&h.back_history), vec!["neuf/a.md"]);
        assert_eq!(docs(&h.forward_history), vec!["neuf/c.md"]);
        assert_eq!(
            doc_of(h.take_forward(entree("bb.md"))).as_deref(),
            Some("neuf/c.md")
        );
    }

    #[test]
    fn borne_la_pile_arriere() {
        let mut h = LeafHistory::new();
        for i in 0..HISTORY_MAX + 10 {
            h.push(entree(&format!("{i}.md")));
        }
        assert_eq!(h.back_history.len(), HISTORY_MAX);
        assert_eq!(h.back_history[0].doc.as_deref(), Some("10.md"));
    }

    // ─── Ajoutés ───
    #[test]
    fn une_entree_sans_document_est_toujours_vivante() {
        let mut h = LeafHistory::with_is_alive(|_| false);
        h.push(HistoryEntry {
            state: ViewState::new("empty"),
            e_state: None,
            doc: None,
        });
        assert!(h.can_go_back());
    }
}
