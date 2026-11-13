//! Les commandes de scène de la couche (contrat de docs/tranches/T5.md §5) :
//!
//! - `pixels:widget-test` `{kind: rigide|flex|seuil|vue, from, to?, side, height}` : pose un widget
//!   de test dans la couche exposée aux scènes. Le rigide vit dans SON calque (le « globe » d'un
//!   autre calque), les autres dans le calque des essais ;
//! - `pixels:activer` `{id}` : active le widget `w<rang>` (rang de sa commande `pixels:widget-test`
//!   dans la scène, 0 pour la première).
//!
//! La couche visée est celle que le montage d'une vue expose ([`exposer_aux_scenes`]) ; sans couche
//! exposée, les commandes ne font rien.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{Command, CommandManager};
use fragment_layers::types::GutterSide;
use serde_json::Value;

use super::couche::{CalqueWidgets, CoucheWidgets};
use super::essai::{SorteEssai, SpecEssai, WidgetEssai, poser_essai};
use crate::flottant::{chaine, ecouter_args, nombre};

#[derive(Default)]
struct EtatScene {
    couche: Option<Weak<CoucheWidgets>>,
    rigides: Option<CalqueWidgets>,
    essais: Option<CalqueWidgets>,
    widgets: Vec<WidgetEssai>,
}

impl EtatScene {
    fn vider(&mut self) {
        self.widgets.clear();
        for c in [self.rigides.take(), self.essais.take()]
            .into_iter()
            .flatten()
        {
            c.fermer();
        }
    }
}

thread_local! {
    static SCENE: RefCell<EtatScene> = RefCell::new(EtatScene::default());
}

/// La couche que visent les commandes de scène (posée par le montage de la vue ouverte).
pub fn exposer_aux_scenes(couche: &Rc<CoucheWidgets>) {
    SCENE.with(|s| {
        let mut s = s.borrow_mut();
        s.vider();
        s.couche = Some(Rc::downgrade(couche));
    });
}

/// Le côté de marge d'un argument (`left` | `right`, défaut : droite).
fn cote(args: &Value) -> GutterSide {
    match chaine(args, "side") {
        Some("left") => GutterSide::Left,
        _ => GutterSide::Right,
    }
}

fn position(args: &Value, cle: &str) -> Option<usize> {
    args.get(cle).and_then(Value::as_u64).map(|n| n as usize)
}

/// Pose le widget de test décrit par `args` dans la couche exposée.
fn poser(args: &Value) {
    let Some(sorte) = chaine(args, "kind").and_then(SorteEssai::depuis_nom) else {
        return;
    };
    let Some(from) = position(args, "from") else {
        return;
    };
    let hauteur = f64::from(nombre(args, "height").unwrap_or(100.0));
    let spec = SpecEssai::marge(sorte, from, position(args, "to"), cote(args), hauteur);
    SCENE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(couche) = s.couche.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let w = {
            let calque = if sorte == SorteEssai::Rigide {
                s.rigides.get_or_insert_with(|| couche.nouveau_calque())
            } else {
                s.essais.get_or_insert_with(|| couche.nouveau_calque())
            };
            poser_essai(&couche, calque, spec)
        };
        s.widgets.push(w);
    });
}

/// Active le widget `w<rang>`.
fn activer(args: &Value) {
    let Some(rang) = chaine(args, "id")
        .and_then(|id| id.strip_prefix('w'))
        .and_then(|n| n.parse::<usize>().ok())
    else {
        return;
    };
    let handle = SCENE.with(|s| s.borrow().widgets.get(rang).map(|w| Rc::clone(&w.handle)));
    if let Some(h) = handle {
        h.set_active(true);
    }
}

/// Enregistre `pixels:widget-test` et `pixels:activer`. Chaque scène repart sans widget.
pub fn commandes_de_scene(commandes: &CommandManager) {
    SCENE.with(|s| s.borrow_mut().vider());
    let args = ecouter_args(commandes);
    let a = Rc::clone(&args);
    let poser_cmd = Command::new("pixels:widget-test", "Pixels : poser un widget de test")
        .with_callback(move || poser(&a.borrow().clone()));
    let _retrait = commandes.add_command(poser_cmd, "pixels");
    let activer_cmd = Command::new("pixels:activer", "Pixels : activer un widget")
        .with_callback(move || activer(&args.borrow().clone()));
    let _retrait = commandes.add_command(activer_cmd, "pixels");
}
