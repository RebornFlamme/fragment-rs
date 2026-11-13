// Port de app/src/ui/views/chat/scroll/search.ts (la barre, `chercher`, `suivant`, `dessiner`,
// `fermer`, `relancer`) et de scroll.css (`.chat-search*`, `.range-highlight.chat-search-match`)
//! Ctrl+F dans la conversation : une barre en haut du fil, les occurrences SURLIGNÉES sur la surface
//! de la conversation par le calque de `fr-t5-surlignages` (le chemin de tout passage), Entrée /
//! Maj+Entrée = suivant / précédent, Échap ferme.
//!
//! - [`EtatRecherche`] : le modèle pur (champ, occurrences, courante, plages à surligner) ;
//! - [`HoteRecherche`] : ce que la barre demande à la vue chat (le texte de la surface, amener une
//!   position à l'écran, l'intention de lecture, rendre le focus) ; [`HoteChat`] en est
//!   l'implémentation sur la `ChatSurface` et le `Defileur` ;
//! - [`RechercheChat`] : la barre montée (entité GPUI), qui tient le focus de son champ.
//!
//! ★ Ouvrir la recherche est une INTENTION DE LECTURE : le suivi du flux s'arrête
//!   ([`HoteRecherche::ouverte`]), sinon le prochain jeton ramènerait le lecteur au bas du fil.
//! ★ La recherche ne voit pas les accents ni la casse (`search_model`) ; ses positions sont celles
//!   de la surface à pas fixe (`SurfaceText::to_pos`), jamais des index dans le texte.
//! ★ Le clavier du champ est l'écouteur de frappes de sa barre (pas une portée de plus) : la barre
//!   n'existe que pour l'écran du chat, et c'est le montage qui lie Ctrl+F à [`RechercheChat::ouvrir`].

use std::rc::Rc;

use fragment_chat::search_model::{find_all, step};
use fragment_text::{HighlightRange, SurfaceText};
use fragment_theme::Couleur;
use gpui_kit::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement as _, Render, Styled as _, Window, div,
};

use crate::chat_defilement::Defileur;
use crate::chat_surface::ChatSurface;
use crate::echelle::u;
use crate::flottant::texte;
use crate::icones;
use crate::surlignages::CalqueSurlignages;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La clef des plages de la recherche dans le calque (`CLE`).
pub const CLE_RECHERCHE: &str = "chat-search";
/// La classe d'une occurrence, et celle de l'occurrence courante.
pub const CLASSE_OCCURRENCE: &str = "chat-search-match";
pub const CLASSE_COURANTE: &str = "chat-search-match is-current";
/// Le texte du champ vide.
pub const MARQUE: &str = "Chercher dans la conversation…";
/// Au plus ce nombre d'occurrences (la valeur par défaut de `findAll`).
pub const OCCURRENCES_MAX: usize = 1000;

/// Les contrôles de la barre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoutonRecherche {
    Precedent,
    Suivant,
    Fermer,
}

impl BoutonRecherche {
    pub const TOUS: [BoutonRecherche; 3] = [
        BoutonRecherche::Precedent,
        BoutonRecherche::Suivant,
        BoutonRecherche::Fermer,
    ];

    /// L'`aria-label` du bouton.
    #[must_use]
    pub fn libelle(self) -> &'static str {
        match self {
            BoutonRecherche::Precedent => "Précédent",
            BoutonRecherche::Suivant => "Suivant",
            BoutonRecherche::Fermer => "Fermer",
        }
    }

    /// Le nom de l'icône.
    #[must_use]
    pub fn icone(self) -> &'static str {
        match self {
            BoutonRecherche::Precedent => "arrow-up",
            BoutonRecherche::Suivant => "arrow-down",
            BoutonRecherche::Fermer => "x",
        }
    }
}

/// Le modèle de la barre : le champ, les occurrences (positions de la surface), la courante.
#[derive(Debug, Default)]
pub struct EtatRecherche {
    ouverte: bool,
    valeur: String,
    /// Tout le champ est sélectionné (ouverture, Ctrl+F) : la prochaine frappe le remplace.
    tout_selectionne: bool,
    trouves: Vec<(usize, usize)>,
    courant: Option<usize>,
}

impl EtatRecherche {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn ouverte(&self) -> bool {
        self.ouverte
    }

    #[must_use]
    pub fn valeur(&self) -> &str {
        &self.valeur
    }

    #[must_use]
    pub fn tout_selectionne(&self) -> bool {
        self.tout_selectionne
    }

    /// Une requête non vide (`trim`) est-elle saisie ?
    #[must_use]
    pub fn a_une_requete(&self) -> bool {
        !self.valeur.trim().is_empty()
    }

    /// Ctrl+F : ouvre (ou re-sélectionne le champ). Vrai si une requête gardée est à relancer.
    pub fn ouvrir(&mut self) -> bool {
        self.ouverte = true;
        self.tout_selectionne = true;
        self.a_une_requete()
    }

    /// Échap / ✕ : ferme, oublie les occurrences (la requête reste dans le champ).
    pub fn fermer(&mut self) {
        self.ouverte = false;
        self.tout_selectionne = false;
        self.trouves.clear();
        self.courant = None;
    }

    /// Du texte tapé. Vrai si le champ a changé.
    pub fn saisir(&mut self, texte: &str) -> bool {
        if texte.is_empty() || texte.chars().any(char::is_control) {
            return false;
        }
        if self.tout_selectionne {
            self.valeur.clear();
            self.tout_selectionne = false;
        }
        self.valeur.push_str(texte);
        true
    }

    /// Retour arrière. Vrai si le champ a changé.
    pub fn effacer(&mut self) -> bool {
        let avant = self.valeur.len();
        if self.tout_selectionne {
            self.valeur.clear();
            self.tout_selectionne = false;
        } else {
            self.valeur.pop();
        }
        self.valeur.len() != avant
    }

    /// Cherche la requête dans le texte de la surface. `aller` : la première occurrence devient la
    /// courante (une frappe) ; sinon (un message arrivé) la courante est gardée si elle existe
    /// encore. `texte` absent : aucune occurrence.
    pub fn chercher(&mut self, texte: Option<&SurfaceText>, aller: bool) {
        self.trouves = texte
            .map(|t| {
                find_all(&t.text, &self.valeur, OCCURRENCES_MAX)
                    .into_iter()
                    .map(|o| (t.to_pos(o.from), t.to_pos(o.to)))
                    .collect()
            })
            .unwrap_or_default();
        if self.courant.is_some_and(|c| c >= self.trouves.len()) {
            self.courant = None;
        }
        if aller {
            self.courant = (!self.trouves.is_empty()).then_some(0);
        }
    }

    /// L'occurrence suivante ou précédente (cyclique).
    pub fn suivant(&mut self, en_avant: bool) {
        self.courant = step(self.trouves.len(), self.courant, en_avant);
    }

    #[must_use]
    pub fn nombre(&self) -> usize {
        self.trouves.len()
    }

    #[must_use]
    pub fn courant(&self) -> Option<usize> {
        self.courant
    }

    /// La position de l'occurrence courante dans la surface.
    #[must_use]
    pub fn position_courante(&self) -> Option<usize> {
        self.courant
            .and_then(|c| self.trouves.get(c))
            .map(|&(de, _)| de)
    }

    /// Le compteur : `1 / 3`, `Aucun résultat`, ou rien tant que le champ est vide.
    #[must_use]
    pub fn compteur(&self) -> String {
        if !self.a_une_requete() {
            return String::new();
        }
        match (self.trouves.len(), self.courant) {
            (0, _) => "Aucun résultat".to_owned(),
            (n, c) => format!("{} / {n}", c.map_or(0, |c| c + 1)),
        }
    }

    /// `has-no-results` : une requête sans occurrence.
    #[must_use]
    pub fn aucun_resultat(&self) -> bool {
        self.a_une_requete() && self.trouves.is_empty()
    }

    /// Les plages à poser dans le calque, la courante marquée.
    #[must_use]
    pub fn plages(&self) -> Vec<HighlightRange> {
        self.trouves
            .iter()
            .enumerate()
            .map(|(i, &(from, to))| HighlightRange {
                from,
                to,
                cls: Some(
                    if self.courant == Some(i) {
                        CLASSE_COURANTE
                    } else {
                        CLASSE_OCCURRENCE
                    }
                    .to_owned(),
                ),
            })
            .collect()
    }
}

/// Ce que la barre demande à la vue chat.
pub trait HoteRecherche {
    /// Le texte de la surface de la conversation (`None` : pas encore lisible).
    fn texte(&self, cx: &App) -> Option<SurfaceText>;
    /// Amener la position (de la surface) à l'écran.
    fn reveler(&self, pos: usize, cx: &mut App);
    /// La recherche s'ouvre : une intention de lecture (le suivi s'arrête).
    fn ouverte(&self, cx: &mut App);
    /// La recherche se ferme : rendre le focus (au champ de la conversation).
    fn fermee(&self, window: &mut Window, cx: &mut App);
}

/// L'hôte réel : la surface de la conversation (texte, positions) et le défileur (intention,
/// placement). Le focus rendu à la fermeture est celui que `rendre_focus` désigne.
pub struct HoteChat {
    pub surface: Entity<ChatSurface>,
    pub defileur: Entity<Defileur>,
    pub rendre_focus: Option<FocusHandle>,
}

impl HoteRecherche for HoteChat {
    fn texte(&self, cx: &App) -> Option<SurfaceText> {
        use fragment_text::TextSurface as _;
        match self.surface.read(cx).surface(cx).get_text() {
            fragment_text::FullText::Text(t) => Some(t),
            _ => None,
        }
    }

    fn reveler(&self, pos: usize, cx: &mut App) {
        // Le message qui porte la position, en haut (ligne de base : pas de recentrage au tiers).
        let msg = {
            let s = self.surface.read(cx);
            fragment_chat::chat_surface_model::unit_at(s.unites(), pos).map(|i| s.unites()[i].msg)
        };
        let id = msg.and_then(|m| {
            self.defileur
                .read(cx)
                .mesures()
                .boites
                .get(m)
                .map(|b| b.id.clone())
        });
        if let Some(id) = id {
            self.defileur.update(cx, |d, cx| d.reveler(&id, cx));
        }
    }

    fn ouverte(&self, cx: &mut App) {
        self.defileur.update(cx, |d, cx| d.intention(cx));
    }

    fn fermee(&self, window: &mut Window, cx: &mut App) {
        if let Some(f) = &self.rendre_focus {
            window.focus(f, cx);
        }
    }
}

/// La barre montée.
pub struct RechercheChat {
    etat: EtatRecherche,
    calque: Rc<CalqueSurlignages>,
    hote: Rc<dyn HoteRecherche>,
    focus: FocusHandle,
}

impl RechercheChat {
    /// `calque` : celui de la surface de la conversation.
    pub fn new(
        calque: Rc<CalqueSurlignages>,
        hote: Rc<dyn HoteRecherche>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            etat: EtatRecherche::new(),
            calque,
            hote,
            focus: cx.focus_handle(),
        }
    }

    #[must_use]
    pub fn etat(&self) -> &EtatRecherche {
        &self.etat
    }

    #[must_use]
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Ctrl+F : ouvre la barre, le champ prend le focus et sa requête gardée est relancée.
    pub fn ouvrir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let relancer = self.etat.ouvrir();
        self.hote.ouverte(cx);
        window.focus(&self.focus, cx);
        if relancer {
            self.chercher(true, cx);
        }
        cx.notify();
    }

    /// Échap / ✕ : ferme ; les surlignages disparaissent.
    pub fn fermer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.etat.ouverte() {
            return;
        }
        self.etat.fermer();
        self.calque.supprimer(CLE_RECHERCHE);
        self.hote.fermee(window, cx);
        cx.notify();
    }

    /// Le texte a changé (un message est arrivé) : la barre ouverte relit, sans bouger la courante.
    pub fn relancer(&mut self, cx: &mut Context<Self>) {
        if self.etat.ouverte() && self.etat.a_une_requete() {
            self.chercher(false, cx);
        }
    }

    /// Entrée / Maj+Entrée / ↓ / ↑.
    pub fn suivant(&mut self, en_avant: bool, cx: &mut Context<Self>) {
        if !self.etat.ouverte() {
            return;
        }
        self.etat.suivant(en_avant);
        self.dessiner(true, cx);
    }

    fn chercher(&mut self, aller: bool, cx: &mut Context<Self>) {
        let texte = self.hote.texte(cx);
        self.etat.chercher(texte.as_ref(), aller);
        self.dessiner(aller, cx);
    }

    fn dessiner(&mut self, aller: bool, cx: &mut Context<Self>) {
        self.calque.poser(CLE_RECHERCHE, self.etat.plages());
        if aller && let Some(pos) = self.etat.position_courante() {
            self.hote.reveler(pos, cx);
        }
        cx.notify();
    }

    /// Une frappe dans le champ : Échap, Entrée, Ctrl+F, retour arrière, du texte.
    fn sur_touche(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.etat.ouverte() {
            return;
        }
        let k = &ev.keystroke;
        let m = &k.modifiers;
        match k.key.as_str() {
            "escape" => self.fermer(window, cx),
            "enter" => self.suivant(!m.shift, cx),
            "f" if m.control || m.platform => {
                self.etat.ouvrir();
                cx.notify();
            }
            "backspace" => {
                if self.etat.effacer() {
                    self.chercher(true, cx);
                }
            }
            _ => {
                let raccourci = m.control || m.alt || m.platform;
                match &k.key_char {
                    Some(c) if !raccourci => {
                        if self.etat.saisir(c) {
                            self.chercher(true, cx);
                        }
                    }
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn sur_bouton(&mut self, b: BoutonRecherche, window: &mut Window, cx: &mut Context<Self>) {
        match b {
            BoutonRecherche::Precedent => self.suivant(false, cx),
            BoutonRecherche::Suivant => self.suivant(true, cx),
            BoutonRecherche::Fermer => self.fermer(window, cx),
        }
    }
}

impl Render for RechercheChat {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.etat.ouverte() {
            return div().id("chat-search").into_any_element();
        }
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);

        let loupe = crate::icone::icone_image()
            .path(icones::chemin("search"))
            .size(u(cx, f.icon_xs))
            .flex_none()
            .text_color(couleur(c.text_muted));
        let contenu = if self.etat.valeur().is_empty() {
            div()
                .text_color(couleur(c.text_faint))
                .child(MARQUE.to_owned())
        } else if self.etat.tout_selectionne() {
            div()
                .bg(couleur(c.color_accent.melange(35.0, Couleur::TRANSPARENT)))
                .text_color(couleur(c.text_normal))
                .child(self.etat.valeur().to_owned())
        } else {
            div()
                .text_color(couleur(c.text_normal))
                .child(self.etat.valeur().to_owned())
        };
        let saisie = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w(u(cx, 0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .child(contenu);
        let compteur = div()
            .flex_none()
            .whitespace_nowrap()
            .text_color(couleur(if self.etat.aucun_resultat() {
                c.text_error
            } else {
                c.text_muted
            }))
            .child(self.etat.compteur());

        let mut barre = div()
            .id("chat-search")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.sur_touche(ev, window, cx);
            }))
            // Cliquer dans la barre rend le focus au champ.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                }),
            )
            .flex()
            .items_center()
            .gap(u(cx, f.size_2_2))
            .mx(u(cx, f.size_4_3))
            .my(u(cx, f.size_2_2))
            .py(u(cx, f.size_2_1))
            .px(u(cx, f.size_4_2))
            .bg(couleur(c.background_primary_alt))
            .border(u(cx, f.border_width))
            .border_color(couleur(c.tab_outline_color))
            .rounded(u(cx, f.input_radius))
            .child(loupe)
            .child(saisie)
            .child(compteur);
        for b in BoutonRecherche::TOUS {
            let survol = couleur(c.background_modifier_hover);
            let texte_survol = couleur(c.text_normal);
            barre = barre.child(
                div()
                    .id(("chat-search-bouton", b as usize))
                    .flex()
                    .p(u(cx, f.size_2_1))
                    .rounded(u(cx, f.clickable_icon_radius))
                    .text_color(couleur(c.text_muted))
                    .hover(move |s| s.bg(survol).text_color(texte_survol))
                    // Sans voler le focus du champ.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.sur_bouton(b, window, cx);
                        }),
                    )
                    .child(
                        crate::icone::icone_image()
                            .path(icones::chemin(b.icone()))
                            .size(u(cx, f.icon_xs))
                            .flex_none(),
                    ),
            );
        }
        texte(barre, cx, t.typo.font_ui_small, t.typo.line_height_normal).into_any_element()
    }
}
