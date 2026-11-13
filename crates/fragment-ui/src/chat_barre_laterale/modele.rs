// Port de app/src/ui/views/chat/components/sidebar.ts (l'état et la liste ; le regroupement et le
// filtre sont dans `fragment_chat::history_model`).
//! Le modèle PUR de la barre des conversations : ce qu'elle montre, dans l'ordre, pour une liste,
//! une requête, une conversation active et une horloge.

use fragment_chat::api::ChatConversationInfo;
use fragment_chat::calendrier::Fuseau;
use fragment_chat::history_model::{filter_by_title, group_by_day};

/// La largeur de la barre ouverte (`.chat-sidebar-zone > .chat-sidebar`, chat-view.css:198).
pub const LARGEUR_BARRE: f32 = 260.0;

/// Le titre montré : « Sans titre » quand il est vide.
#[must_use]
pub fn titre_affiche(titre: &str) -> &str {
    if titre.is_empty() {
        "Sans titre"
    } else {
        titre
    }
}

/// Le message quand la liste visible est vide.
#[must_use]
pub fn message_vide(requete: &str) -> &'static str {
    if requete.trim().is_empty() {
        "Aucune conversation."
    } else {
        "Aucun résultat."
    }
}

/// Une ligne de la liste : l'étiquette d'un groupe, ou une conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ligne {
    Etiquette(String),
    Conversation {
        id: String,
        titre: String,
        /// `data-state="active"` : la conversation affichée.
        active: bool,
    },
}

/// La signature d'une liste : change quand un id, un titre ou une date change, pas autrement.
/// C'est elle qui décide de redessiner quand le fournisseur annonce un changement.
#[must_use]
pub fn signature(items: &[ChatConversationInfo]) -> String {
    let mut s = String::new();
    for it in items {
        s.push_str(&it.id);
        s.push('\u{1}');
        s.push_str(&it.title);
        s.push('\u{1}');
        s.push_str(&it.updated_at.to_string());
        s.push('\u{2}');
    }
    s
}

/// L'état de la barre : la liste du fournisseur, la requête, la conversation active.
#[derive(Clone, Debug, Default)]
pub struct EtatBarre {
    items: Vec<ChatConversationInfo>,
    signature: String,
    pub requete: String,
    pub active: String,
}

impl EtatBarre {
    /// Pose la liste et la conversation active. Rend `true` si quelque chose a changé (donc s'il
    /// faut redessiner) : une liste de même signature et la même active ne changent rien.
    pub fn mettre_a_jour(&mut self, items: &[ChatConversationInfo], active: &str) -> bool {
        let sig = signature(items);
        let change = sig != self.signature || active != self.active;
        if sig != self.signature {
            self.items = items.to_vec();
            self.signature = sig;
        }
        if active != self.active {
            self.active = active.to_owned();
        }
        change
    }

    /// Le nombre de conversations, filtre non compris.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Aucune conversation du tout.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Les lignes à dessiner : filtrées sans accents ni casse, groupées par jour.
    #[must_use]
    pub fn lignes(&self, maintenant: i64, fuseau: &dyn Fuseau) -> Vec<Ligne> {
        let visibles = filter_by_title(&self.items, &self.requete);
        let mut lignes = Vec::new();
        for g in group_by_day(&visibles, maintenant, fuseau) {
            lignes.push(Ligne::Etiquette(g.label));
            for c in g.items {
                lignes.push(Ligne::Conversation {
                    active: c.id == self.active,
                    titre: titre_affiche(&c.title).to_owned(),
                    id: c.id,
                });
            }
        }
        lignes
    }
}
