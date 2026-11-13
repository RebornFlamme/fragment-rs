//! La commande de scène de ce module (contrat de docs/tranches/T5.md §5) :
//!
//! - `pixels:survol` `{id, from, to, active?}` : un passage survolé — le surlignage de `from` à `to`
//!   sous la clef `id`, en classe `is-passage` (et `is-active` si `active`). Sans plage : retire
//!   la clef.
//!
//! Le calque visé est celui que le montage d'une vue expose ([`exposer_aux_scenes`]) ; sans calque
//! exposé, la commande ne fait rien.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{Command, CommandManager};
use fragment_text::HighlightRange;
use serde_json::Value;

use super::calque::CalqueSurlignages;
use crate::flottant::{chaine, ecouter_args};

thread_local! {
    static CALQUE: RefCell<Option<Weak<CalqueSurlignages>>> = const { RefCell::new(None) };
}

/// Le calque que vise la scène (posé par le montage de la vue ouverte).
pub fn exposer_aux_scenes(calque: &Rc<CalqueSurlignages>) {
    CALQUE.with(|c| *c.borrow_mut() = Some(Rc::downgrade(calque)));
}

fn position(args: &Value, cle: &str) -> Option<usize> {
    args.get(cle).and_then(Value::as_u64).map(|n| n as usize)
}

fn survoler(args: &Value) {
    let Some(id) = chaine(args, "id").filter(|id| !id.is_empty()) else {
        return;
    };
    let Some(calque) = CALQUE.with(|c| c.borrow().as_ref().and_then(Weak::upgrade)) else {
        return;
    };
    let (Some(from), Some(to)) = (position(args, "from"), position(args, "to")) else {
        calque.supprimer(id);
        return;
    };
    let actif = args.get("active").and_then(Value::as_bool).unwrap_or(false);
    let cls = if actif {
        "is-passage is-active"
    } else {
        "is-passage"
    };
    calque.poser(id, vec![HighlightRange::with_cls(from, to, cls)]);
}

/// Enregistre `pixels:survol`.
pub fn commandes_de_scene(commandes: &CommandManager) {
    CALQUE.with(|c| *c.borrow_mut() = None);
    let args = ecouter_args(commandes);
    let cmd = Command::new("pixels:survol", "Pixels : surligner un passage survolé")
        .with_callback(move || survoler(&args.borrow().clone()));
    let _retrait = commandes.add_command(cmd, "pixels");
}
