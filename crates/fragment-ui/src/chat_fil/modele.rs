// Port de app/src/ui/views/chat/components/thread.ts (la forme d'un message, son état, ce qui se met
// à jour en place) et de app/src/core/workspace/viewIcons.ts (la teinte d'un avatar).
//! Le modèle PUR du fil : ce que la vue décide à partir d'un [`LiveMessage`], sans GPUI.
//!
//! - la **forme** d'un message (auteur, erreur, types des parts) : tant qu'elle ne change pas, le
//!   message est mis à jour EN PLACE ; sinon il est reconstruit ;
//! - son **état** (`streaming` / `error` / `idle`), l'attente (« ••• »), la barre d'actions ;
//! - l'**avatar** : une teinte stable tirée du texte, convertie d'OKLCH en sRGB ;
//! - les **libellés** accessibles (un article par message, le bouton d'envoi).

use fragment_chat::api::{Participant, Role};
use fragment_chat::chat_session::LiveMessage;

/// L'état d'un message (`data-state` du TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatMessage {
    /// Sa réponse s'écrit.
    Streaming,
    /// Le tour a échoué.
    Error,
    /// Au repos.
    Idle,
}

impl EtatMessage {
    /// Le nom de l'état, comme le `data-state` de Fragment.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            EtatMessage::Streaming => "streaming",
            EtatMessage::Error => "error",
            EtatMessage::Idle => "idle",
        }
    }
}

/// L'état d'un message : en flux d'abord, puis erreur.
#[must_use]
pub fn etat(m: &LiveMessage) -> EtatMessage {
    if m.streaming {
        EtatMessage::Streaming
    } else if m.error.is_some() {
        EtatMessage::Error
    } else {
        EtatMessage::Idle
    }
}

/// La forme d'un message, en chaîne : `auteur|erreur|type,type…`.
///
/// ★ Mémorisée en CHAÎNE : la session mute le même `LiveMessage` en place. Comparer à l'objet ne
/// verrait rien et un part neuf laisserait un rendu manquant (messages bloqués en « streaming »).
#[must_use]
pub fn forme(m: &LiveMessage) -> String {
    let types: Vec<&str> = m.parts.iter().map(|p| p.kind.as_str()).collect();
    format!(
        "{}|{}|{}",
        m.author,
        m.error.as_deref().unwrap_or(""),
        types.join(",")
    )
}

/// Un message en flux qui n'a encore aucun part : on montre « ••• ».
#[must_use]
pub fn attend(m: &LiveMessage) -> bool {
    m.streaming && m.parts.is_empty()
}

/// La barre d'actions : une réponse de l'assistant, finie, qui a du texte.
#[must_use]
pub fn avec_actions(m: &LiveMessage, role: Role) -> bool {
    !m.streaming
        && role == Role::Assistant
        && m.parts
            .iter()
            .any(|p| p.kind == "text" && p.text.as_deref().is_some_and(|t| !t.is_empty()))
}

/// Ce que copie le bouton « Copier » : tous les textes, séparés par une ligne vide.
#[must_use]
pub fn texte_a_copier(m: &LiveMessage) -> String {
    m.parts
        .iter()
        .filter(|p| p.kind == "text")
        .map(|p| p.text.as_deref().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Le message peut-il être mis à jour en place ? Non si sa forme a changé, si le nombre de parts
/// rendus n'est plus celui des parts, ou si une barre d'actions déjà posée devrait se « déposer »
/// (un tour relancé) : on reconstruit alors.
#[must_use]
pub fn peut_patcher(
    forme_rendue: &str,
    nb_rendus: usize,
    suivant: &LiveMessage,
    actions_posees: bool,
) -> bool {
    forme(suivant) == forme_rendue
        && nb_rendus == suivant.parts.len()
        && !(actions_posees && suivant.streaming)
}

/// Le participant d'un auteur, ou un assistant de repli qui porte l'id (`participants.find(…) ?? …`).
#[must_use]
pub fn auteur_de(participants: &[Participant], id: &str) -> Participant {
    participants
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .unwrap_or_else(|| Participant {
            id: id.to_owned(),
            name: id.to_owned(),
            initials: None,
            role: Role::Assistant,
        })
}

/// Le rôle ARIA de la liste des messages : un `log`, SANS région vive (chaque delta serait annoncé ;
/// les événements passent par l'annonceur de la vue, `fr-t7-defilement`).
pub const ROLE_LISTE: &str = "log";
/// Le rôle ARIA d'un message : un article, nommé par son auteur.
pub const ROLE_MESSAGE: &str = "article";
/// Le libellé de l'attente d'une réponse.
pub const LIBELLE_ATTENTE: &str = "En train d’écrire";

/// Le libellé (et l'infobulle) du bouton du champ : « Arrêter » pendant un tour, sinon « Envoyer ».
#[must_use]
pub fn libelle_bouton(en_cours: bool) -> &'static str {
    if en_cours { "Arrêter" } else { "Envoyer" }
}

// ── L'avatar ────────────────────────────────────────────────────────────────

/// Les teintes des pastilles (angles OKLCH) : corail, ambre, olive, vert, sarcelle, ciel, indigo,
/// magenta (`TEINTES` de viewIcons.ts).
const TEINTES: [u32; 8] = [25, 60, 100, 150, 190, 235, 275, 325];

/// Une teinte stable tirée d'un texte : même texte, même teinte, partout.
#[must_use]
pub fn teinte(texte: &str) -> u32 {
    let mut h: u32 = 0;
    for c in texte.chars() {
        // `charCodeAt` du TS : une unité UTF-16 (un caractère hors du plan de base en donne deux).
        let mut tampon = [0u16; 2];
        for u in c.encode_utf16(&mut tampon) {
            h = (h * 31 + u32::from(*u)) % 9973;
        }
    }
    TEINTES[(h as usize) % TEINTES.len()]
}

/// OKLCH → sRGB 8 bits (matrice de Björn Ottosson, gamma sRGB, bornée au gamut).
#[must_use]
pub fn oklch_vers_rgb(l: f32, c: f32, h_degres: f32) -> [u8; 3] {
    let h = h_degres.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let lin = [
        4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_93 * s3,
        -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_4 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3,
    ];
    lin.map(|v| {
        let v = v.clamp(0.0, 1.0);
        let g = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        // Borné à [0, 255] : la conversion ne peut ni déborder ni être négative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let octet = (g * 255.0).round().clamp(0.0, 255.0) as u8;
        octet
    })
}

/// Ce qu'un avatar montre : une icône (nom connu du registre) ou un court texte teinté.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContenuAvatar {
    /// Un nom d'icône du registre : l'encre du texte sur le fond de l'espace de travail.
    Icone(String),
    /// Des initiales (ou un emoji) sur une pastille teintée par le texte.
    Texte { texte: String, teinte: u32 },
}

/// Le contenu de l'avatar d'un participant. `connue` dit si un nom est une icône du registre.
#[must_use]
pub fn contenu_avatar(p: &Participant, connue: impl Fn(&str) -> bool) -> ContenuAvatar {
    let texte = p
        .initials
        .clone()
        .unwrap_or_else(|| fragment_chat::format::initials_of(&p.name));
    if connue(&texte) {
        ContenuAvatar::Icone(texte)
    } else {
        let teinte = teinte(&texte);
        ContenuAvatar::Texte { texte, teinte }
    }
}

/// Le curseur de flux : le dernier texte d'un message en flux s'« écrit ».
#[must_use]
pub fn curseur_de_flux(m: &LiveMessage) -> bool {
    m.streaming
        && m.parts
            .last()
            .is_some_and(|p| p.kind == "text" && p.text.as_deref().is_some_and(|t| !t.is_empty()))
}
