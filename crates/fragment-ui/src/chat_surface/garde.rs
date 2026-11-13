// Port de la règle « une frappe dans un champ de la vue lui appartient » (la garde `enChamp` des
// raccourcis de portée de vue, app/src/ui/views/annotation/AnnotationPlugin.ts, et la note
// « / », Mod+Z, Échap du CLAUDE.md de Fragment pour ChatSurface).
//! La garde du champ : « / », Ctrl+Z et Échap tapés dans le champ d'une conversation restent au
//! champ, même quand la vue (devenue un document : calques, annotation, lecteur) lie ces touches
//! dans sa portée.
//!
//! ★ Le `Keymap` du cœur passe AVANT GPUI (l'intercepteur de `clavier`) et ses handlers ne voient
//!   pas la cible de la frappe : la garde la relève donc à chaque image, dans la pile des contextes
//!   clavier de l'élément qui a le focus. Un champ de saisie GPUI s'y annonce par le contexte
//!   `Input` ; l'éditeur en lecture seule d'un message aussi, mais avec `mode=editor` — et lui
//!   n'est PAS un champ (dans Fragment, un éditeur non éditable n'est ni `input` ni `textarea` :
//!   Ctrl+Z y revient aux traits de l'annotation).

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::{KeyEvent, KeymapContext, KeymapResult};
use gpui_kit::Window;

/// Le contexte clavier d'un champ de saisie de gpui-kit.
const CONTEXTE_CHAMP: &str = "Input";

/// La garde partagée (clonable) : le focus est-il dans un champ de saisie ?
#[derive(Clone, Default)]
pub struct GardeChamp {
    en_champ: Rc<Cell<bool>>,
}

impl GardeChamp {
    /// Le focus est-il dans un champ de saisie (relevé à la dernière image) ?
    #[must_use]
    pub fn en_champ(&self) -> bool {
        self.en_champ.get()
    }

    /// Relève où est le focus (à chaque image, par `plan_observe`).
    pub fn actualiser(&self, window: &Window) {
        self.en_champ.set(focus_dans_un_champ(window));
    }

    /// Enveloppe un handler de portée : il ne voit plus les frappes tapées dans un champ (il rend
    /// `None`, la frappe continue jusqu'au champ).
    pub fn proteger(
        &self,
        f: impl Fn(&KeyEvent, &KeymapContext) -> KeymapResult + 'static,
    ) -> impl Fn(&KeyEvent, &KeymapContext) -> KeymapResult + 'static {
        let garde = self.clone();
        move |e, c| if garde.en_champ() { None } else { f(e, c) }
    }
}

/// Le focus de `window` est-il dans un champ de saisie (et pas dans un éditeur de document) ?
#[must_use]
pub fn focus_dans_un_champ(window: &Window) -> bool {
    window
        .context_stack()
        .iter()
        .any(|c| c.contains(CONTEXTE_CHAMP) && c.get("mode").is_none_or(|m| m != "editor"))
}
