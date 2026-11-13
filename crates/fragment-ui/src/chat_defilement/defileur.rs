// Port de app/src/ui/views/chat/scroll/ScrollController.ts (le côté DOM : écoute des gestes,
// mesures, écriture de la position) et de `.chat-messages` / `.chat-tail` (scroll.css).
//! Le DÉFILEUR d'une conversation (une entité GPUI) : le contenu (le plan du fil), puis la RÉSERVE
//! (`.chat-tail`), HORS du contenu — les positions de texte et les calques ne la voient pas.
//!
//! ★ Les mesures se prennent APRÈS la mise en page : la colonne rapporte les boîtes de ses messages
//! (`on_children_prepainted`, par [`Defileur::mesure_colonne`]) puis le défileur, dont l'écouteur
//! passe en dernier, fait la passe du [`ControleurDefilement`]. Une position écrite là vaut pour
//! l'image SUIVANTE (GPUI a déjà placé les enfants de celle-ci) : la passe redemande une image
//! (`defer` + `notify`, un rafraîchissement pendant le dessin serait perdu). Une image de retard,
//! notée en dette dans `parite.d/fr-t7-defilement.toml`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use fragment_chat::follow_model::{FollowMode, ReadingAnchor, Sens};
use gpui_kit::{
    AnyView, App, Bounds, Context, ElementId, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Window, div, point, px,
};

use super::controleur::{Boite, ControleurDefilement, Hote, Mesures, Position, contexte_px};
use super::pastille::{EtatPastille, etat_pastille};
use crate::chat_fil::ListeMessages;

/// L'identifiant du défileur (`.chat-messages`).
pub const ID_DEFILEUR: &str = "chat-messages";
/// L'identifiant de la réserve (`.chat-tail`).
pub const ID_RESERVE: &str = "chat-tail";

/// Le rapport d'une colonne : les boîtes de ses messages, dans l'ordre, après mise en page.
pub type MesureColonne = Rc<dyn Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App)>;

/// Une demande rejouée à chaque passe jusqu'à ce qu'elle aboutisse (le message n'est peut-être pas
/// encore là : `reprendre` de ChatView).
#[derive(Clone, Debug, PartialEq)]
enum Demande {
    Reveler(String),
    RevelerDerniere(String),
    Restaurer(ReadingAnchor),
    Position(Position),
}

/// Le défileur.
pub struct Defileur {
    contenu: AnyView,
    poignee: ScrollHandle,
    controleur: ControleurDefilement,
    /// `(id, question)` des messages affichés, dans l'ordre des enfants de la colonne.
    messages: Vec<(String, bool)>,
    colonne: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    hote: Hote,
    focus: FocusHandle,
    origine: Instant,
    demande: Option<Demande>,
    /// Les mesures de la dernière passe.
    mesures: Mesures,
    /// Combien de passes ont été faites (les tests comptent).
    passes: usize,
}

impl Defileur {
    /// Un défileur autour de `contenu`. `taille_police` : la taille du texte de la colonne en px
    /// logiques (le contexte gardé au-dessus d'une question placée en dépend).
    pub fn new(contenu: AnyView, taille_police: f64, cx: &mut Context<Self>) -> Self {
        Defileur {
            contenu,
            poignee: ScrollHandle::new(),
            controleur: ControleurDefilement::new(contexte_px(taille_police)),
            messages: Vec::new(),
            colonne: Rc::new(RefCell::new(Vec::new())),
            hote: Hote::default(),
            focus: cx.focus_handle(),
            origine: cx.background_executor().now(),
            demande: None,
            mesures: Mesures::default(),
            passes: 0,
        }
    }

    /// Un défileur autour de la liste des messages : y branche la mesure de la colonne.
    pub fn pour_liste(
        liste: &Entity<ListeMessages>,
        taille_police: f64,
        cx: &mut Context<Self>,
    ) -> Self {
        let d = Self::new(liste.clone().into(), taille_police, cx);
        let mesure = Self::mesure_colonne_de(&d.colonne);
        liste.update(cx, |l, cx| l.poser_mesure(mesure, cx));
        d
    }

    fn mesure_colonne_de(colonne: &Rc<RefCell<Vec<Bounds<Pixels>>>>) -> MesureColonne {
        let c = Rc::clone(colonne);
        Rc::new(move |boites, _, _| *c.borrow_mut() = boites)
    }

    /// Le rapport à brancher sur la colonne des messages (`on_children_prepainted`).
    #[must_use]
    pub fn mesure_colonne(&self) -> MesureColonne {
        Self::mesure_colonne_de(&self.colonne)
    }

    fn maintenant(&self, cx: &App) -> f64 {
        cx.background_executor()
            .now()
            .saturating_duration_since(self.origine)
            .as_secs_f64()
            * 1000.0
    }

    // ── Ce que la vue lui dit ──────────────────────────────────────────────

    /// Les messages affichés (`(id, question)`), une réponse en cours, la réserve permise (onglet,
    /// panneau). À appeler après chaque mise à jour du fil.
    pub fn poser_fil(
        &mut self,
        messages: Vec<(String, bool)>,
        streaming: bool,
        reserve: bool,
        cx: &mut Context<Self>,
    ) {
        self.messages = messages;
        self.hote = Hote { streaming, reserve };
        cx.notify();
    }

    /// La taille du texte de la colonne a changé.
    pub fn poser_taille_police(&mut self, taille_police: f64, cx: &mut Context<Self>) {
        self.controleur.poser_contexte(contexte_px(taille_police));
        cx.notify();
    }

    /// Un geste de l'utilisateur hors du défileur (Ctrl+F, lien suivi, focus dans le fil) : la
    /// lecture est à lui.
    pub fn intention(&mut self, cx: &mut Context<Self>) {
        let t = self.maintenant(cx);
        self.controleur.intention(t);
        self.demande = None;
        cx.notify();
    }

    /// L'utilisateur vient d'envoyer.
    pub fn envoye(&mut self, cx: &mut Context<Self>) {
        self.controleur.envoye(&self.mesures);
        self.demande = None;
        cx.notify();
    }

    /// Le tour est fini (réponse, arrêt, erreur) : aucun déplacement.
    pub fn tour_fini(&mut self, cx: &mut Context<Self>) {
        self.hote.streaming = false;
        self.controleur.tour_fini();
        cx.notify();
    }

    /// Aller au plus récent (pastille, Fin), puis suivre.
    pub fn aller_au_plus_recent(&mut self, cx: &mut Context<Self>) {
        self.demande = None;
        if let Some(t) = self.controleur.aller_au_plus_recent(&self.mesures) {
            self.ecrire(t);
        }
        cx.notify();
    }

    /// Placer un message en haut (rail, lien, Alt+↑/↓) : rejoué tant qu'il n'est pas là.
    pub fn reveler(&mut self, id: &str, cx: &mut Context<Self>) {
        self.demande = Some(Demande::Reveler(id.to_owned()));
        self.servir();
        cx.notify();
    }

    /// Ouvrir sur la DERNIÈRE question en haut (une conversation ouverte sans position).
    pub fn reveler_derniere(&mut self, id: &str, cx: &mut Context<Self>) {
        self.demande = Some(Demande::RevelerDerniere(id.to_owned()));
        self.servir();
        cx.notify();
    }

    /// Le tour suivant / précédent de la lecture (Alt+↓ / Alt+↑) ; faux s'il n'y en a pas.
    pub fn tour_voisin(&mut self, sens: Sens, cx: &mut Context<Self>) -> bool {
        match self.controleur.tour_voisin(&self.mesures, sens) {
            Some(id) => {
                self.reveler(&id, cx);
                true
            }
            None => false,
        }
    }

    /// Une autre conversation : plus de réserve, en haut.
    pub fn oublier_reserve(&mut self, cx: &mut Context<Self>) {
        self.controleur.oublier_reserve();
        self.demande = None;
        cx.notify();
    }

    /// L'ancre de lecture (avant une reconstruction).
    #[must_use]
    pub fn ancre(&self) -> Option<ReadingAnchor> {
        ControleurDefilement::ancre(&self.mesures)
    }

    /// Rendre une ancre de lecture (après une reconstruction), rejouée tant que son message manque.
    pub fn restaurer(&mut self, a: ReadingAnchor, cx: &mut Context<Self>) {
        self.demande = Some(Demande::Restaurer(a));
        self.servir();
        cx.notify();
    }

    /// La position de la vue (`lirePosition`) : `{message: rang, dy}`.
    #[must_use]
    pub fn position(&self) -> Option<Position> {
        ControleurDefilement::position(&self.mesures)
    }

    /// Aller à une position de la vue (`allerAPosition`), rejouée tant que le message manque.
    pub fn aller_a_position(&mut self, p: Position, cx: &mut Context<Self>) {
        self.demande = Some(Demande::Position(p));
        self.servir();
        cx.notify();
    }

    // ── Lectures ───────────────────────────────────────────────────────────

    #[must_use]
    pub fn mode(&self) -> FollowMode {
        self.controleur.mode()
    }

    /// Le contexte gardé au-dessus d'une question placée (`contextPx`).
    #[must_use]
    pub fn contexte_px(&self) -> f64 {
        self.controleur.contexte()
    }

    /// La hauteur de la réserve.
    #[must_use]
    pub fn reserve(&self) -> f64 {
        self.controleur.reserve()
    }

    /// Le bas du contenu est-il à l'écran ?
    #[must_use]
    pub fn bas_visible(&self) -> bool {
        ControleurDefilement::bas_visible(&self.mesures)
    }

    /// La position (`scrollTop`).
    #[must_use]
    pub fn scroll_top(&self) -> f64 {
        -f64::from(f32::from(self.poignee.offset().y))
    }

    /// Les mesures de la dernière passe (boîtes en px du contenu).
    #[must_use]
    pub fn mesures(&self) -> &Mesures {
        &self.mesures
    }

    /// Le haut d'un message en px depuis le haut du défileur (à la dernière passe).
    #[must_use]
    pub fn haut_de(&self, id: &str) -> Option<f64> {
        let b = self.mesures.boites.iter().find(|b| b.id == id)?;
        Some(b.top - self.mesures.scroll_top)
    }

    #[must_use]
    pub fn passes(&self) -> usize {
        self.passes
    }

    /// L'état de la pastille : `nouveaux` est le nombre de messages arrivés non lus (voir
    /// [`super::pastille::SuiviLus`]).
    #[must_use]
    pub fn etat_pastille(&self, nouveaux: usize) -> EtatPastille {
        etat_pastille(
            self.bas_visible(),
            self.messages.is_empty(),
            self.hote.streaming,
            nouveaux,
        )
    }

    // ── La mécanique ───────────────────────────────────────────────────────

    fn ecrire(&mut self, t: f64) {
        self.poignee.set_offset(point(px(0.0), px(-(t as f32))));
        self.mesures.scroll_top = t;
    }

    /// Tente la demande en attente sur les dernières mesures ; la garde si elle n'aboutit pas.
    fn servir(&mut self) {
        let Some(d) = self.demande.clone() else {
            return;
        };
        let m = &self.mesures;
        let cible = match &d {
            Demande::Reveler(id) => self.controleur.reveler(id, m),
            Demande::RevelerDerniere(id) => self.controleur.reveler_derniere(id, m, self.hote),
            Demande::Restaurer(a) => self.controleur.restaurer(a, m),
            Demande::Position(p) => self.controleur.aller_a_position(*p, m),
        };
        let Some(t) = cible else {
            return;
        };
        // Atteinte (ou bornée par le bas du contenu, réserve comprise) : servie.
        let max = (m.bas_contenu + self.controleur.reserve() - m.visible).max(0.0);
        let atteinte =
            (t - m.scroll_top).abs() < 2.0 || (t >= max - 1.0 && m.scroll_top >= max - 1.0);
        if (t - m.scroll_top).abs() >= 1.0 {
            self.ecrire(t);
        }
        if atteinte {
            self.demande = None;
        }
    }

    /// Les mesures de cette image : la colonne a rapporté ses boîtes, la poignée sait où en est le
    /// défilement.
    fn mesurer(&self) -> Mesures {
        let sc = self.poignee.bounds();
        let decalage = f64::from(f32::from(self.poignee.offset().y));
        let haut = f64::from(f32::from(sc.top()));
        // Les enfants du défileur sont mesurés SANS le décalage, ceux de la colonne AVEC.
        let bas_contenu = self
            .poignee
            .bounds_for_item(0)
            .map_or(0.0, |b| f64::from(f32::from(b.bottom())) - haut);
        let origine = haut + decalage;
        let boites = self
            .colonne
            .borrow()
            .iter()
            .zip(&self.messages)
            .map(|(b, (id, question))| Boite {
                id: id.clone(),
                question: *question,
                top: f64::from(f32::from(b.top())) - origine,
                bottom: f64::from(f32::from(b.bottom())) - origine,
            })
            .collect();
        Mesures {
            scroll_top: -decalage,
            visible: f64::from(f32::from(sc.size.height)),
            bas_contenu,
            boites,
        }
    }

    /// Après la mise en page de l'image : la passe du contrôleur, la demande en attente.
    fn apres_mise_en_page(&mut self, cx: &mut Context<Self>) {
        self.passes += 1;
        self.mesures = self.mesurer();
        let reserve_avant = self.controleur.reserve();
        let maintenant = self.maintenant(cx);
        let avant = self.scroll_top();
        if self.demande.is_some() {
            self.servir();
        }
        // La demande servie a déjà écrit : la passe part de la nouvelle position.
        let p = self.controleur.passe(&self.mesures, self.hote, maintenant);
        if let Some(t) = p.ecrire {
            self.ecrire(t);
        }
        let change = self.controleur.prendre_changement();
        if (self.scroll_top() - avant).abs() >= 1.0
            || (p.reserve - reserve_avant).abs() > 0.5
            || change
        {
            // Pendant le dessin, un `notify` serait perdu : après lui.
            let id = cx.entity_id();
            cx.defer(move |cx| cx.notify(id));
        }
    }

    fn touche(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        if e.keystroke.modifiers.alt || e.keystroke.modifiers.control {
            return;
        }
        let page = (self.mesures.visible * 0.875).max(40.0);
        let delta = match k {
            // Fin : aller au plus récent (la vue), pas une intention.
            "end" => {
                self.aller_au_plus_recent(cx);
                cx.stop_propagation();
                return;
            }
            "pageup" => -page,
            "pagedown" | "space" => page,
            "up" => -40.0,
            "down" => 40.0,
            "home" => -self.scroll_top(),
            _ => return,
        };
        self.intention(cx);
        let max =
            (self.mesures.bas_contenu + self.controleur.reserve() - self.mesures.visible).max(0.0);
        let t = (self.scroll_top() + delta).clamp(0.0, max);
        self.poignee.set_offset(point(px(0.0), px(-(t as f32))));
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for Defileur {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let moi = cx.entity().downgrade();
        div()
            // L'écouteur du défileur passe APRÈS celui de la colonne (enfants d'abord).
            .on_children_prepainted(move |_, _, cx| {
                if let Some(d) = moi.upgrade() {
                    d.update(cx, |d, cx| d.apres_mise_en_page(cx));
                }
            })
            .id(ElementId::from(ID_DEFILEUR))
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.poignee)
            // Toute intention : molette, appui (sélection, lien, focus, barre), touches.
            .on_scroll(cx.listener(|d, _, _, cx| d.intention(cx)))
            .capture_any_mouse_down(cx.listener(|d, _, _, cx| d.intention(cx)))
            .on_key_down(cx.listener(|d, e, _, cx| d.touche(e, cx)))
            .child(div().flex_none().w_full().child(self.contenu.clone()))
            .child(
                div()
                    .id(ElementId::from(ID_RESERVE))
                    .flex_none()
                    .w_full()
                    .h(px(self.controleur.reserve() as f32)),
            )
    }
}
