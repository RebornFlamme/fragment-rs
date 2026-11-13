// Port de `monter` et `attachmentsExtension` (app/src/ui/views/chat/components/attachments.ts).
//! L'état des puces d'UN champ et l'extension `chat:composer@1` qui le porte.
//!
//! - [`EtatPieces`] : les pièces posées (coller, déposer), retirées (✕), prises à l'envoi, gardées
//!   (`save`) et rendues (`restore`) quand la vue reconstruit le champ ;
//! - [`ExtensionPieces`] : `create` monte un état neuf et le rend visible à la zone (les puces) ;
//!   [`ExtensionPieces::champ`] la range dans le registre du fil à la priorité −100.
//!
//! ★ Une extension par registre : la zone des puces lit l'état du DERNIER champ monté. Un registre
//! partagé par deux champs vivants se tromperait de puces ; le montage donne un registre par vue.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use fragment_chat::api::{
    AttachmentPartData, ChatAttachment, ChatComposerExtension, ChatComposerHandle, ChatSlotContext,
    ComposerTake,
};
use gpui_kit::RenderImage;
use serde_json::{Map, Value};

use super::donnees::{part_attachment, part_data, piece_depuis_json, piece_json};
use super::vue::{vignette_de, zone_puces};
use crate::chat_fil::ExtensionChamp;

/// L'id de l'extension (registre `chat:composer@1`, et `ChatProvider::without`).
pub const ATTACHMENTS_EXTENSION: &str = "attachments";

/// Sa priorité : tout plugin peut la remplacer (même id, priorité plus haute).
pub const PRIORITE_PIECES: i32 = -100;

/// Une pièce posée dans le champ : les octets, ce qu'on en montre, sa vignette.
#[derive(Clone)]
pub struct Piece {
    pub att: ChatAttachment,
    pub data: AttachmentPartData,
    /// La vignette décodée d'une image (PNG) ; absente : icône et nom.
    pub vignette: Option<Arc<RenderImage>>,
}

/// Les puces d'un champ.
#[derive(Default)]
pub struct EtatPieces {
    pieces: Vec<Piece>,
}

impl EtatPieces {
    /// Pose des pièces (coller, déposer, `restore`).
    pub fn ajouter(&mut self, atts: impl IntoIterator<Item = ChatAttachment>) {
        for att in atts {
            let data = part_data(&att);
            let vignette = vignette_de(&att);
            self.pieces.push(Piece {
                att,
                data,
                vignette,
            });
        }
    }

    /// Retire la puce de ce rang (le ✕) ; `true` si elle existait.
    pub fn retirer(&mut self, rang: usize) -> bool {
        if rang < self.pieces.len() {
            self.pieces.remove(rang);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Une pièce seule suffit à envoyer.
    #[must_use]
    pub fn pending(&self) -> bool {
        !self.pieces.is_empty()
    }

    /// À l'envoi : les octets pour le fournisseur (`attachments`) et un part `attachment` par pièce.
    /// Vide ensuite. `None` sans pièce.
    pub fn prendre(&mut self) -> Option<ComposerTake> {
        if self.pieces.is_empty() {
            return None;
        }
        let prises = std::mem::take(&mut self.pieces);
        let mut opts = Map::new();
        opts.insert(
            "attachments".into(),
            Value::Array(prises.iter().map(|p| piece_json(&p.att)).collect()),
        );
        Some(ComposerTake {
            opts: Some(opts),
            parts: prises.iter().map(|p| part_attachment(&p.att)).collect(),
        })
    }

    /// L'état gardé : les pièces en JSON (`save`). Vide : `None`.
    #[must_use]
    pub fn sauver(&self) -> Option<Value> {
        (!self.pieces.is_empty())
            .then(|| Value::Array(self.pieces.iter().map(|p| piece_json(&p.att)).collect()))
    }

    /// Rend un état gardé : ignoré s'il n'est pas, en entier, une liste de pièces (`estPieces`).
    pub fn restaurer(&mut self, etat: &Value) {
        let Some(liste) = etat.as_array() else {
            return;
        };
        let atts: Option<Vec<ChatAttachment>> = liste.iter().map(piece_depuis_json).collect();
        if let Some(atts) = atts {
            self.ajouter(atts);
        }
    }
}

/// L'état partagé entre la poignée et la zone.
pub type EtatPartage = Rc<RefCell<EtatPieces>>;

/// La poignée d'un champ : le comportement du champ, sur l'état partagé.
struct PoigneePieces(EtatPartage);

impl ChatComposerHandle for PoigneePieces {
    fn pending(&self) -> bool {
        self.0.borrow().pending()
    }

    fn take(&mut self) -> Option<ComposerTake> {
        self.0.borrow_mut().prendre()
    }

    fn save(&self) -> Option<Value> {
        self.0.borrow().sauver()
    }

    fn restore(&mut self, etat: Value) {
        self.0.borrow_mut().restaurer(&etat);
    }
}

/// L'extension `attachments` : la contribution du plugin chat à `chat:composer@1`.
#[derive(Clone, Default)]
pub struct ExtensionPieces {
    courant: Rc<RefCell<Option<EtatPartage>>>,
}

impl ExtensionPieces {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'état du dernier champ monté : là où le collage et le dépôt posent leurs pièces.
    #[must_use]
    pub fn etat(&self) -> Option<EtatPartage> {
        self.courant.borrow().clone()
    }

    /// Pose des pièces dans le dernier champ monté ; `false` sans champ.
    pub fn ajouter(&self, atts: Vec<ChatAttachment>) -> bool {
        match self.etat() {
            Some(e) => {
                e.borrow_mut().ajouter(atts);
                true
            }
            None => false,
        }
    }

    /// L'entrée du registre du fil : l'extension (priorité [`PRIORITE_PIECES`]) et sa zone de puces.
    #[must_use]
    pub fn champ(&self) -> ExtensionChamp {
        ExtensionChamp::new(Rc::new(self.clone()))
            .avec_priorite(PRIORITE_PIECES)
            .avec_zone(zone_puces(self.courant.clone()))
    }
}

impl ChatComposerExtension for ExtensionPieces {
    fn id(&self) -> &str {
        ATTACHMENTS_EXTENSION
    }

    fn create(&self, _ctx: &ChatSlotContext) -> Box<dyn ChatComposerHandle> {
        let etat: EtatPartage = Rc::default();
        *self.courant.borrow_mut() = Some(etat.clone());
        Box::new(PoigneePieces(etat))
    }
}
