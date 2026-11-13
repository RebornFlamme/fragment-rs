// Port de app/src/ui/views/agent/bar/registerBar.ts (Bars : mount, vueCible, setHidden, la ligne du
// menu « Calques », la commande `agent:toggle-bar`).
//! Les barres de toutes les vues, et ce que le plugin agent en fait : le raccourci global Mod+J, la
//! ligne sous « Agent » du menu « Calques », le masquage partout.
//!
//! ★ Mod+J est une commande GLOBALE (portée racine) : aucune portée de vue ne le prend, il marche
//!   depuis le texte du document comme depuis le champ de la barre (sa portée chaîne à la racine).
//! ★ Depuis la conversation que la barre vient d'ouvrir à côté (la feuille active est un chat), il
//!   vise la barre du DERNIER document actif de la zone centrale.
//!
//! Écart : la ligne du menu n'a pas son propre œil (le modèle `LayerMenuChild` n'en porte pas) ;
//! l'œil est une ACTION de la ligne « Agent » (« Masquer / Afficher la barre »).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use fragment_core::{Command, CommandManager, Disposer, Hotkey, ItemId, Modifier};
use fragment_layers::menu_calques::LayerMenuChild;
use fragment_layers::types::LayerMenuSection;

use super::modele::{BarreAgent, CALQUE_AGENT, Cible, ContexteBarre, EtatBarre};

/// `agent:toggle-bar`.
pub const COMMANDE_BASCULER: &str = "agent:toggle-bar";
/// L'id de la ligne de la barre dans le menu « Calques ».
pub const ID_LIGNE_MENU: &str = "agent-bar";
/// Le propriétaire des enregistrements.
pub const PLUGIN_AGENT: &str = "agent";
/// Le titre de la ligne du menu.
pub const TITRE_LIGNE: &str = "Barre de l’agent";

/// Remet le calque « Agent » sur une vue où il était masqué (la barre se remonte).
pub type RemettreCalque = Rc<dyn Fn(ItemId)>;

/// Les barres des vues.
pub struct Barres {
    ctx: ContexteBarre,
    barres: RefCell<Vec<(ItemId, Rc<BarreAgent>)>>,
    /// Le dernier DOCUMENT actif (suivi par `active-leaf-change`).
    dernier: Cell<Option<ItemId>>,
    /// Les vues dont la barre, remontée, doit s'ouvrir (le calque a été remis par le raccourci).
    a_ouvrir: RefCell<HashSet<ItemId>>,
    remettre: RemettreCalque,
    moi: Weak<Barres>,
}

impl Barres {
    #[must_use]
    pub fn new(ctx: ContexteBarre, remettre: RemettreCalque) -> Rc<Self> {
        Rc::new_cyclic(|moi| Self {
            ctx,
            barres: RefCell::new(Vec::new()),
            dernier: Cell::new(None),
            a_ouvrir: RefCell::default(),
            remettre,
            moi: moi.clone(),
        })
    }

    /// La barre d'une vue.
    #[must_use]
    pub fn barre(&self, vue: ItemId) -> Option<Rc<BarreAgent>> {
        self.barres
            .borrow()
            .iter()
            .find(|(v, _)| *v == vue)
            .map(|(_, b)| b.clone())
    }

    /// Combien de barres sont montées.
    #[must_use]
    pub fn nombre(&self) -> usize {
        self.barres.borrow().len()
    }

    /// La barre d'une vue, montée par le calque `agent` : toute vue de DOCUMENT d'une feuille du
    /// workspace (note, PDF, image, page HTML, vue tierce) — pas une conversation : elle a déjà
    /// son champ. `None` si la vue n'en veut pas.
    pub fn monter(&self, vue: ItemId) -> Option<Rc<BarreAgent>> {
        if !self.ctx.hote.est_document(vue) {
            return None;
        }
        let ouvrir = self.a_ouvrir.borrow_mut().remove(&vue);
        let depart = if ouvrir || self.ctx.reglages.borrow().depart_ouverte {
            EtatBarre::Ouverte
        } else {
            EtatBarre::Repliee
        };
        let barre = BarreAgent::new(vue, self.ctx.clone(), depart);
        self.barres.borrow_mut().push((vue, barre.clone()));
        Some(barre)
    }

    /// Le démontage du calque (ou de la vue).
    pub fn demonter(&self, vue: ItemId) {
        let retiree: Vec<_> = {
            let mut b = self.barres.borrow_mut();
            let (out, reste): (Vec<_>, Vec<_>) = b.drain(..).partition(|(v, _)| *v == vue);
            *b = reste;
            out
        };
        for (_, b) in retiree {
            b.detruire();
        }
    }

    /// `active-leaf-change` : se souvenir du dernier document actif.
    pub fn feuille_active_changee(&self, feuille: Option<ItemId>) {
        if let Some(f) = feuille
            && self.ctx.hote.est_document(f)
        {
            self.dernier.set(Some(f));
        }
    }

    /// La vue dont le raccourci bascule la barre : la feuille ACTIVE si c'est un document ; sinon
    /// (une conversation) le DERNIER document actif qui a une barre.
    fn vue_cible(&self) -> Option<ItemId> {
        if let Some(a) = self.ctx.hote.feuille_active()
            && self.ctx.hote.est_document(a)
        {
            return Some(a);
        }
        self.dernier
            .get()
            .filter(|d| self.ctx.hote.est_document(*d) && self.barre(*d).is_some())
    }

    /// Masquer / afficher la barre PARTOUT (persisté) : l'œil de sa ligne du menu « Calques ».
    pub fn poser_masquee(&self, masquee: bool) {
        let r = {
            let mut r = self.ctx.reglages.borrow_mut();
            r.masquee = masquee;
            *r
        };
        (self.ctx.sauver)(&r);
        let toutes: Vec<_> = self
            .barres
            .borrow()
            .iter()
            .map(|(_, b)| b.clone())
            .collect();
        for b in toutes {
            b.poser_masquee(masquee);
        }
    }

    /// La bascule de destination du champ (réglage global).
    pub fn basculer_cible(&self) -> Cible {
        self.ctx.basculer_cible()
    }

    /// La destination courante d'Entrée.
    #[must_use]
    pub fn cible(&self) -> Cible {
        self.ctx.reglages.borrow().cible
    }

    /// « Replier » dans la barre du champ de la conversation modèle `conversation`.
    pub fn replier_modele(&self, conversation: &str) {
        let b = self
            .barres
            .borrow()
            .iter()
            .find(|(_, b)| b.modele().as_deref() == Some(conversation))
            .map(|(_, b)| b.clone());
        if let Some(b) = b {
            b.replier();
        }
    }

    /// La barre est-elle masquée partout ?
    #[must_use]
    pub fn masquee(&self) -> bool {
        self.ctx.reglages.borrow().masquee
    }

    /// Le geste de la commande `agent:toggle-bar`. `checking` : seulement dire si elle s'applique.
    /// `None` : pas de document visé.
    pub fn basculer_commande(&self, checking: bool) -> Option<bool> {
        let vue = self.vue_cible()?;
        if checking {
            return Some(true);
        }
        // Masquée (l'œil de sa ligne) : on l'affiche, ouverte.
        let masquee = self.masquee();
        if masquee {
            self.poser_masquee(false);
        }
        if let Some(b) = self.barre(vue) {
            if masquee {
                b.definir_etat(EtatBarre::Ouverte, true);
            } else {
                b.basculer();
            }
        } else {
            // Calque « Agent » masqué : on le remet, la barre s'ouvre à son montage.
            self.a_ouvrir.borrow_mut().insert(vue);
            (self.remettre)(vue);
        }
        Some(true)
    }

    /// Inscrit la commande `agent:toggle-bar` (Mod+J).
    pub fn enregistrer(self: &Rc<Self>, commandes: &CommandManager) -> Disposer {
        let moi = Rc::downgrade(self);
        commandes.add_command(
            Command::new(COMMANDE_BASCULER, "Ouvrir / replier la barre de l’agent")
                .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], "j")])
                .with_check_callback(move |checking| moi.upgrade()?.basculer_commande(checking)),
            PLUGIN_AGENT,
        )
    }

    // ── Le menu « Calques » ─────────────────────────────────────────────────

    /// Ce que la barre ajoute à la ligne `calque` du menu « Calques » de `vue` : sur « Agent », son
    /// œil (une action) et sa ligne, la PREMIÈRE de ses enfants (avant les conversations du
    /// calque, que le montage range après).
    pub fn contribuer(
        &self,
        vue: ItemId,
        calque: &str,
        section: &mut LayerMenuSection,
    ) -> Vec<LayerMenuChild> {
        if calque != CALQUE_AGENT || self.barre(vue).is_none() {
            return Vec::new();
        }
        let masquee = self.masquee();
        let moi = self.moi.clone();
        section.add_action(
            if masquee { "eye-off" } else { "eye" },
            if masquee {
                "Afficher la barre"
            } else {
                "Masquer la barre"
            },
            move || {
                if let Some(b) = moi.upgrade() {
                    b.poser_masquee(!b.masquee());
                }
            },
            false,
        );
        vec![LayerMenuChild {
            id: ID_LIGNE_MENU.to_owned(),
            title: TITRE_LIGNE.to_owned(),
            icon: Some("panel-bottom".to_owned()),
        }]
    }

    /// Un clic sur la ligne : la barre s'ouvre (affichée d'abord si elle était masquée), son
    /// champ prend le focus.
    pub fn cliquer_enfant(&self, vue: ItemId, id: &str) {
        if id != ID_LIGNE_MENU {
            return;
        }
        if self.masquee() {
            self.poser_masquee(false);
        }
        if let Some(b) = self.barre(vue) {
            b.definir_etat(EtatBarre::Ouverte, true);
        }
    }
}
