// Port de app/src/ui/views/chat/components/post.ts (les décisions de mise en page) et de post.css
// (les constantes : gouttière, lignes, container query).
//! Le modèle PUR de la présentation « post » : ce que la vue décide sans GPUI.
//!
//! Le tweet d'une conversation est dessiné par [`super::vue::PostVue`] ; ici vivent les chiffres et
//! les libellés qui se testent seuls : mode compact, hauteur coupée, détection du dépassement,
//! heure relative, ligne de citation, puce de pièces jointes.
//!
//! La mise en forme du modèle de conversation (question, réponse, compteur) est déjà portée dans
//! `fragment_chat::post_model` ; on la réexporte.

pub use fragment_chat::post_model::{PostMessage, PostModel, post_model, relative_time};

use fragment_chat::calendrier::Fuseau;

/// Le seuil du mode compact : `@container (max-width: 360px)`, mesuré sur la boîte de CONTENU du
/// post (le conteneur d'inline-size est `.chat-post`, rembourré de 16 px de chaque côté).
pub const SEUIL_COMPACT: f32 = 360.0;

/// Le remplissage horizontal du post (`var(--size-4-4)`).
pub const REMPLISSAGE_H: f32 = 16.0;

/// Le côté d'un avatar en grand (40 px) et en compact (28 px).
pub const AVATAR_GRAND: f32 = 40.0;
pub const AVATAR_COMPACT: f32 = 28.0;

/// La hauteur de la ligne de fondu : la dernière ligne de texte, 20 px (18 en compact).
pub const FONDU: f32 = 20.0;

/// Lignes montrées : 6 pour le tweet, 4 pour la réponse (`--chat-post-lines`).
pub const LIGNES_TWEET: u32 = 6;
pub const LIGNES_REPONSE: u32 = 4;

/// La tolérance du constat « plus haut que sa limite » (`scrollHeight > clientHeight + 1`).
pub const TOLERANCE_COUPE: f32 = 1.0;

/// Le conteneur de la présentation, dit étroit : la largeur du post moins ses deux remplissages
/// tient dans [`SEUIL_COMPACT`].
#[must_use]
pub fn est_compact(largeur_post: f32) -> bool {
    largeur_post - 2.0 * REMPLISSAGE_H <= SEUIL_COMPACT
}

/// Le côté de l'avatar selon le mode.
#[must_use]
pub fn cote_avatar(compact: bool) -> f32 {
    if compact {
        AVATAR_COMPACT
    } else {
        AVATAR_GRAND
    }
}

/// La hauteur d'une ligne du corps : 20 px, 18 px en compact.
#[must_use]
pub fn interligne(compact: bool) -> f32 {
    if compact { 18.0 } else { 20.0 }
}

/// Le nombre de lignes montrées du corps d'une rangée.
#[must_use]
pub fn lignes_max(reponse: bool) -> u32 {
    if reponse {
        LIGNES_REPONSE
    } else {
        LIGNES_TWEET
    }
}

/// La hauteur maximale du corps : `lignes × interligne`.
#[must_use]
pub fn hauteur_max(reponse: bool, compact: bool) -> f32 {
    // Quelques lignes : exact en f32.
    #[allow(clippy::cast_precision_loss)]
    let n = lignes_max(reponse) as f32;
    n * interligne(compact)
}

/// Le texte est-il COUPÉ ? Le fondu ne se pose que s'il dépasse sa limite.
#[must_use]
pub fn est_coupe(hauteur_contenu: f32, reponse: bool, compact: bool) -> bool {
    hauteur_contenu > hauteur_max(reponse, compact) + TOLERANCE_COUPE
}

/// La hauteur que le corps occupe : celle du contenu, plafonnée.
#[must_use]
pub fn hauteur_visible(hauteur_contenu: f32, reponse: bool, compact: bool) -> f32 {
    hauteur_contenu.min(hauteur_max(reponse, compact))
}

/// La source d'une conversation : le document et, s'il y en a un, le passage cité.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origine {
    pub titre: String,
    pub icone: String,
    pub citation: Option<String>,
}

/// Le passage en UNE ligne : blancs réduits à une espace, entre chevrons français. Vide : rien.
#[must_use]
pub fn texte_citation(citation: &str) -> Option<String> {
    let t = citation.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        None
    } else {
        Some(format!("« {t} »"))
    }
}

/// La puce des pièces jointes : « Pièce jointe », « 3 pièces jointes », rien sans pièce.
#[must_use]
pub fn libelle_pieces(n: usize) -> Option<String> {
    match n {
        0 => None,
        1 => Some("Pièce jointe".to_owned()),
        n => Some(format!("{n} pièces jointes")),
    }
}

/// L'heure d'un post : « · 2 h », l'horloge `maintenant` étant celle de l'hôte (figée en test).
#[must_use]
pub fn libelle_heure(at: i64, maintenant: i64, fuseau: &dyn Fuseau) -> String {
    format!("· {}", relative_time(at, maintenant, fuseau))
}

/// Le texte de la conversation vide.
pub const LIBELLE_VIDE: &str = "Conversation vide";

/// Le lien qui mène au fil entier, quand d'autres messages suivent la 1re réponse.
pub const LIBELLE_FIL: &str = "Afficher ce fil";
