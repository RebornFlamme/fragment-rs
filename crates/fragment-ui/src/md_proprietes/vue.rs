// Port de app/src/ui/views/markdown/properties/PropertiesWidget.ts (rendu, champs, focus, glisser) et
// de properties.css (constantes de mise en page, jetons de thème).
//! La vue des Properties : une entité GPUI qui peint la table clé / valeur et traduit chaque geste en
//! appel du contrôleur pur ([`super::modele::Proprietes`]).
//!
//! ★ Chaque édition est UNE transaction isolée de l'éditeur : `EditorState::replace` enregistre un
//!   pas d'annulation `Atomic`, qui ne fusionne jamais avec la frappe voisine (le `isolateHistory`
//!   du TS). Annuler « Clear properties » restitue donc tout le bloc d'un coup.
//! ★ Les champs sont reconstruits quand le bloc du document change (annulation, modification
//!   externe) ; le focus demandé par le contrôleur est posé à l'image suivante.
//!
//! Dette (à fermer aux scènes `t3-proprietes*`) : les largeurs des colonnes et le détail des
//! champs de saisie sont ceux de la vue, pas encore mesurés contre Fragment au pixel.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::Keymap;
use fragment_markdown::frontmatter_model::{self as fm, Property, PropertyType, PropertyValue};
use gpui_kit::base::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, canvas, div, px,
};

use super::modele::{
    Action, Effet, EntreeMenu, Focus, Proprietes, RegistreTypes, Sortie, affichage_date,
    affichage_nombre, cible_de_depot, est_coche, est_un_glisser, icone_du_type, lire_bloc,
    type_affiche, valeur_nombre, valeur_texte,
};
use crate::echelle::u;
use crate::editeur::EditeurLive;
use crate::icones;
use crate::menu::Menu;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Hauteur minimale d'une ligne (`.metadata-property`).
const HAUTEUR_LIGNE: f32 = 30.0;
/// Hauteur du bouton « Add property ».
const HAUTEUR_AJOUT: f32 = 28.0;
/// Largeur de la case icône : 16 px d'icône plus deux fois l'inset (`--metadata-icon-inset`).
const LARGEUR_ICONE: f32 = 24.0;
/// Côté de la case à cocher.
const COTE_CASE: f32 = 15.0;
/// Rembourrage horizontal des champs (`--metadata-value-pad`).
const REMBOURRAGE_VALEUR: f32 = 6.0;
/// Corps du contenu : 0,9375 em du texte de la note (16 px).
const CORPS_CONTENU: f32 = 15.0;

/// Ce que la vue demande à son propriétaire (le shell qui monte la vue Markdown).
#[derive(Clone)]
pub struct CrochetsProprietes {
    /// Une notice (clé en double, presse-papiers vide).
    pub notifier: Rc<dyn Fn(&str)>,
    /// Déplier les Properties dans l'hôte (on ajoute dans un bloc replié).
    pub deplier: Rc<dyn Fn()>,
    /// Les raccourcis, pour les menus.
    pub keymap: Rc<Keymap>,
}

/// L'éditeur de la valeur d'une ligne.
enum Valeur {
    /// Texte, nombre, date : un champ, rendu au type affiché.
    Champ(Entity<InputState>),
    /// Liste ou tags : des pastilles et un champ d'ajout.
    Liste(Entity<InputState>),
    Case,
}

struct Ligne {
    cle: Entity<InputState>,
    valeur: Valeur,
    type_: PropertyType,
}

/// Un appui sur une icône, en attente de savoir si c'est un clic ou un glisser.
struct Appui {
    ligne: usize,
    depart: (f32, f32),
    glisse: bool,
    cible: usize,
}

/// La vue des Properties d'un éditeur.
pub struct VueProprietes {
    editeur: Entity<EditeurLive>,
    ctl: Proprietes,
    types: Rc<dyn RegistreTypes>,
    crochets: CrochetsProprietes,
    replie: bool,
    lignes: Vec<Ligne>,
    /// Les propriétés et types que montrent les champs actuels.
    vu: Option<Vec<(Property, PropertyType)>>,
    attente: Option<Entity<InputState>>,
    /// Le menu de type de la ligne d'ajout est ouvert : le blur de sa clé ne l'abandonne pas.
    menu_attente_ouvert: Rc<RefCell<bool>>,
    focus_demande: Option<Focus>,
    appui: Option<Appui>,
    /// Haut et bas de chaque ligne, relevés à la dernière image (fenêtre).
    boites: Rc<RefCell<Vec<(f32, f32)>>>,
    abonnements: Vec<Subscription>,
}

impl VueProprietes {
    pub fn new(
        editeur: Entity<EditeurLive>,
        types: Rc<dyn RegistreTypes>,
        crochets: CrochetsProprietes,
        cx: &mut Context<Self>,
    ) -> Self {
        let lecture_seule = editeur.read(cx).lecture_seule();
        // Un changement du texte (annulation, édition externe) redessine les champs.
        let abonnements = vec![cx.observe(&editeur, |_, _, cx| cx.notify())];
        Self {
            editeur,
            ctl: Proprietes::new(lecture_seule),
            types,
            crochets,
            replie: false,
            lignes: Vec::new(),
            vu: None,
            attente: None,
            menu_attente_ouvert: Rc::new(RefCell::new(false)),
            focus_demande: None,
            appui: None,
            boites: Rc::new(RefCell::new(Vec::new())),
            abonnements,
        }
    }

    /// Le reflet de l'état de repli de l'hôte.
    pub fn poser_replie(&mut self, replie: bool, cx: &mut Context<Self>) {
        if self.replie != replie {
            self.replie = replie;
            cx.notify();
        }
    }

    /// ↑ depuis le texte : le focus entre dans les Properties.
    pub fn focus_derniere(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let texte = self.texte(cx);
        match self.ctl.focus_derniere(&texte) {
            Some(s) => {
                self.appliquer(s, window, cx);
                true
            }
            None => false,
        }
    }

    /// `---` vient de créer le bloc : ouvre une propriété.
    pub fn apres_creation_du_bloc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.ctl.apres_creation_du_bloc();
        self.appliquer(s, window, cx);
    }

    fn texte(&self, cx: &Context<Self>) -> String {
        self.editeur.read(cx).texte().to_owned()
    }

    // ─── Application des sorties du contrôleur ────────────────────────────

    /// Applique l'édition (UNE transaction) puis les effets.
    fn appliquer(&mut self, s: Sortie, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = s.edition {
            let etat = self.editeur.read(cx).etat().clone();
            etat.update(cx, |st, cx| {
                let avant = st.selected_range();
                st.set_selected_range(e.de..e.a, cx);
                st.replace(e.texte.clone(), window, cx);
                let delta = e.texte.len() as isize - (e.a - e.de) as isize;
                let decale = |p: usize| {
                    if p >= e.a {
                        p.saturating_add_signed(delta)
                    } else {
                        p
                    }
                };
                let c = e
                    .curseur
                    .unwrap_or_else(|| decale(avant.end).min(st.text().len()));
                st.set_selected_range(c..c, cx);
            });
        }
        for effet in s.effets {
            match effet {
                Effet::Notice(m) => (self.crochets.notifier)(&m),
                Effet::Deplier => (self.crochets.deplier)(),
                Effet::Focus(Focus::Texte) => {
                    let etat = self.editeur.read(cx).etat().clone();
                    etat.update(cx, |st, cx| st.focus(window, cx));
                }
                Effet::Focus(f) => self.focus_demande = Some(f),
                Effet::Presse(t) => cx.write_to_clipboard(ClipboardItem::new_string(t)),
            }
        }
        cx.notify();
    }

    // ─── Les champs ───────────────────────────────────────────────────────

    fn champ(
        &mut self,
        valeur: &str,
        vide: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let e = cx.new(|cx| InputState::new(window, cx));
        e.update(cx, |s, cx| {
            s.set_value(valeur.to_owned(), window, cx);
            s.set_placeholder(vide.to_owned(), window, cx);
            s.set_readonly(self.ctl.lecture_seule(), cx);
        });
        e
    }

    /// Reconstruit les champs si le bloc ou les types ont changé.
    fn synchroniser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let texte = self.texte(cx);
        let Some(bloc) = lire_bloc(&texte) else {
            return;
        };
        let courant: Vec<(Property, PropertyType)> = bloc
            .props
            .iter()
            .map(|p| (p.clone(), type_affiche(self.types.as_ref(), p)))
            .collect();
        if self.vu.as_ref() == Some(&courant) {
            return;
        }
        self.abonnements.truncate(1);
        let mut lignes = Vec::new();
        for (i, (p, t)) in courant.iter().enumerate() {
            let cle = self.champ(&p.key, "", window, cx);
            self.abonner_cle(&cle, Some(i), window, cx);
            let valeur = match t {
                PropertyType::Checkbox => Valeur::Case,
                PropertyType::Multitext | PropertyType::Tags => {
                    let c = self.champ(
                        "",
                        if fm::as_list(&p.value).is_empty() {
                            "Empty"
                        } else {
                            ""
                        },
                        window,
                        cx,
                    );
                    self.abonner_liste(&c, i, *t, window, cx);
                    Valeur::Liste(c)
                }
                _ => {
                    let affiche = match t {
                        PropertyType::Number => affichage_nombre(&p.value),
                        PropertyType::Date => affichage_date(&p.value, false),
                        PropertyType::Datetime => affichage_date(&p.value, true),
                        _ => fm::as_text(&p.value),
                    };
                    let c = self.champ(&affiche, "Empty", window, cx);
                    self.abonner_valeur(&c, i, *t, window, cx);
                    Valeur::Champ(c)
                }
            };
            lignes.push(Ligne {
                cle,
                valeur,
                type_: *t,
            });
        }
        self.lignes = lignes;
        self.vu = Some(courant);
        if self.attente.is_some() && self.ctl.attente().is_none() {
            self.attente = None;
        }
    }

    fn abonner_cle(
        &mut self,
        c: &Entity<InputState>,
        i: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = cx.subscribe_in(c, window, move |this, e, ev: &InputEvent, window, cx| {
            let entree = matches!(ev, InputEvent::PressEnter { .. });
            if !entree && !matches!(ev, InputEvent::Blur) {
                if let (Some(_), InputEvent::Change) = (i, ev) {
                    return;
                }
                if i.is_none() && matches!(ev, InputEvent::Change) {
                    let v = e.read(cx).value().to_string();
                    this.ctl.saisir_cle_attente(&v);
                }
                return;
            }
            let cle = e.read(cx).value().to_string();
            let texte = this.texte(cx);
            let s = match i {
                Some(i) => this.ctl.valider_cle(&texte, i, &cle, entree),
                None => {
                    if !entree && *this.menu_attente_ouvert.borrow() {
                        return;
                    }
                    if this.ctl.attente().is_none() {
                        return;
                    }
                    let types = this.types.clone();
                    this.ctl
                        .valider_cle_attente(&texte, types.as_ref(), &cle, entree)
                }
            };
            this.appliquer(s, window, cx);
        });
        self.abonnements.push(s);
    }

    fn abonner_valeur(
        &mut self,
        c: &Entity<InputState>,
        i: usize,
        t: PropertyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = cx.subscribe_in(c, window, move |this, e, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let saisie = e.read(cx).value().to_string();
            let valeur = match t {
                PropertyType::Number => match valeur_nombre(&saisie) {
                    Some(v) => v,
                    None => return,
                },
                PropertyType::Date | PropertyType::Datetime => valeur_texte(&saisie),
                _ => valeur_texte(&saisie),
            };
            let texte = this.texte(cx);
            let Some(bloc) = lire_bloc(&texte) else {
                return;
            };
            // Rien n'a changé : pas d'écriture (un blur ne doit pas salir l'historique).
            if bloc.props.get(i).is_none_or(|p| p.value == valeur) {
                return;
            }
            let s = this.ctl.definir_valeur(&texte, i, valeur);
            this.appliquer(s, window, cx);
        });
        self.abonnements.push(s);
    }

    fn abonner_liste(
        &mut self,
        c: &Entity<InputState>,
        i: usize,
        t: PropertyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = cx.subscribe_in(c, window, move |this, e, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let saisie = e.read(cx).value().to_string();
            if saisie.trim().is_empty() {
                return;
            }
            let texte = this.texte(cx);
            let s = this.ctl.ajouter_element(&texte, i, t, &saisie);
            this.appliquer(s, window, cx);
        });
        self.abonnements.push(s);
    }

    // ─── Menus ────────────────────────────────────────────────────────────

    /// Ouvre un menu à `(x, y)` ; ses actions repassent par le contrôleur, hors de l'appel de clic.
    fn ouvrir_menu(
        &mut self,
        entrees: &[EntreeMenu],
        x: f32,
        y: f32,
        a_la_fermeture: Option<Rc<dyn Fn()>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if entrees.is_empty() {
            return;
        }
        let vue = cx.entity().downgrade();
        let asynchrone = window.to_async(cx);
        let declencher: Rc<dyn Fn(Action)> = Rc::new(move |a: Action| {
            let vue = vue.clone();
            asynchrone
                .spawn(async move |cx| {
                    let _ = cx.update(|window, cx| {
                        let _ = vue.update(cx, |this, cx| this.executer(&a, window, cx));
                    });
                })
                .detach();
        });
        let mut menu = Menu::new();
        if entrees.iter().any(|e| !e.section.is_empty()) {
            menu.add_sections(&["type", "clipboard", "danger"]);
        }
        remplir(&mut menu, entrees, &declencher);
        if let Some(f) = a_la_fermeture {
            menu.on_hide(move || f());
        }
        menu.show_at_position(x, y, &self.crochets.keymap);
    }

    pub fn executer(&mut self, a: &Action, window: &mut Window, cx: &mut Context<Self>) {
        let texte = self.texte(cx);
        let presse = cx
            .read_from_clipboard()
            .and_then(|c| c.text())
            .unwrap_or_default();
        let types = self.types.clone();
        let s = self.ctl.executer(a, &texte, types.as_ref(), &presse);
        self.appliquer(s, window, cx);
    }

    fn menu_de_ligne(
        &mut self,
        i: usize,
        x: f32,
        y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let texte = self.texte(cx);
        let entrees = self.ctl.menu_ligne(&texte, self.types.as_ref(), i);
        self.ouvrir_menu(&entrees, x, y, None, window, cx);
    }

    /// Le clic sur l'icône de la ligne d'ajout.
    fn icone_attente(&mut self, x: f32, y: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(champ) = self.attente.clone() else {
            return;
        };
        let cle = champ.read(cx).value().to_string();
        if cle.trim().is_empty() {
            // Pas encore de clé : on choisit le type quand même, il sera appliqué à la validation.
            let entrees = self.ctl.menu_attente();
            *self.menu_attente_ouvert.borrow_mut() = true;
            let drapeau = self.menu_attente_ouvert.clone();
            self.ouvrir_menu(
                &entrees,
                x,
                y,
                Some(Rc::new(move || *drapeau.borrow_mut() = false)),
                window,
                cx,
            );
            return;
        }
        let texte = self.texte(cx);
        let types = self.types.clone();
        let (s, ouvrir) = self.ctl.icone_de_l_attente(&texte, types.as_ref(), &cle);
        self.appliquer(s, window, cx);
        if let Some(i) = ouvrir {
            let texte = self.texte(cx);
            let entrees = self.ctl.menu_ligne(&texte, self.types.as_ref(), i);
            self.ouvrir_menu(&entrees, x, y, None, window, cx);
        }
    }

    // ─── Glisser ──────────────────────────────────────────────────────────

    fn bouger(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(a) = &mut self.appui else {
            return;
        };
        let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
        if !a.glisse && est_un_glisser(x - a.depart.0, y - a.depart.1) {
            a.glisse = true;
        }
        if a.glisse {
            let boites = self.boites.borrow();
            let hauts: Vec<f32> = boites.iter().map(|b| b.0).collect();
            let bas = boites.last().map_or(0.0, |b| b.1);
            a.cible = cible_de_depot(&hauts, bas, y);
            cx.notify();
        }
    }

    fn lacher(&mut self, ev: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.appui.take() else {
            return;
        };
        if a.glisse {
            let texte = self.texte(cx);
            let s = self.ctl.deplacer(&texte, a.ligne, a.cible);
            self.appliquer(s, window, cx);
        } else {
            let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
            self.menu_de_ligne(a.ligne, x, y + 4.0, window, cx);
        }
        cx.notify();
    }
}

/// Traduit des entrées pures en entrées du menu de l'interface.
fn remplir(menu: &mut Menu, entrees: &[EntreeMenu], declencher: &Rc<dyn Fn(Action)>) {
    for e in entrees {
        menu.add_item(|it| {
            it.set_title(&e.titre)
                .set_icon(e.icone)
                .set_checked(e.coche)
                .set_warning(e.avertissement);
            if !e.section.is_empty() {
                it.set_section(e.section);
            }
            if e.sous_menu.is_empty() {
                if let Some(a) = e.action.clone() {
                    let d = declencher.clone();
                    it.on_click(move || d(a.clone()));
                }
            } else {
                remplir(it.set_submenu(), &e.sous_menu, declencher);
            }
        });
    }
}

impl Render for VueProprietes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.synchroniser(window, cx);
        // Le focus demandé par le contrôleur, une fois les champs en place.
        if let Some(f) = self.focus_demande.take() {
            match f {
                Focus::CleAttente => {
                    if let Some(c) = &self.attente {
                        c.update(cx, |s, cx| s.focus(window, cx));
                    }
                }
                Focus::Valeur(i) => {
                    let champ = self.lignes.get(i).and_then(|l| match &l.valeur {
                        Valeur::Champ(c) | Valeur::Liste(c) => Some(c.clone()),
                        Valeur::Case => None,
                    });
                    if let Some(c) = champ {
                        c.update(cx, |s, cx| s.focus(window, cx));
                    }
                }
                Focus::Texte => {}
            }
        }
        // La ligne d'ajout : son champ naît avec elle.
        if self.ctl.attente().is_some() && self.attente.is_none() {
            let c = self.champ("", "", window, cx);
            self.abonner_cle(&c, None, window, cx);
            self.attente = Some(c);
            if self.focus_demande.is_none() {
                self.focus_demande = Some(Focus::CleAttente);
                cx.notify();
            }
        } else if self.ctl.attente().is_none() {
            self.attente = None;
        }

        let theme = theme_ou_defaut(cx);
        let p = &theme.palette;
        let texte = couleur(p.text_normal);
        let faible = couleur(p.text_faint);
        let accent = couleur(p.interactive_accent);
        let survol = couleur(p.background_modifier_hover);
        let bordure = couleur(p.background_modifier_border);
        let lecture_seule = self.ctl.lecture_seule();
        let cible = self.appui.as_ref().filter(|a| a.glisse).map(|a| a.cible);
        let n = self.lignes.len();
        self.boites.borrow_mut().resize(n, (0.0, 0.0));

        let mut liste = div().flex().flex_col().gap(px(2.0));
        for i in 0..n {
            let (cle, type_, valeur) = {
                let l = &self.lignes[i];
                (l.cle.clone(), l.type_, self.valeur_en_element(i, cx))
            };
            let boites = self.boites.clone();
            let en_glisse = self
                .appui
                .as_ref()
                .is_some_and(|a| a.glisse && a.ligne == i);
            let ligne = div()
                .id(("propriete", i))
                .relative()
                .flex()
                .items_center()
                .min_h(u(cx, HAUTEUR_LIGNE))
                .opacity(if en_glisse { 0.5 } else { 1.0 })
                .hover(move |s| s.bg(survol))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
                        this.menu_de_ligne(i, x, y, window, cx);
                    }),
                )
                .child(
                    canvas(
                        move |b, _, _| {
                            if let Some(c) = boites.borrow_mut().get_mut(i) {
                                *c = (f32::from(b.origin.y), f32::from(b.origin.y + b.size.height));
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .child(
                    div()
                        .id(("icone", i))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .w(u(cx, LARGEUR_ICONE))
                        .h(u(cx, 24.0))
                        .rounded(u(cx, theme.forme.radius_s))
                        .hover(move |s| s.bg(bordure))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                if this.ctl.lecture_seule() {
                                    return;
                                }
                                this.appui = Some(Appui {
                                    ligne: i,
                                    depart: (f32::from(ev.position.x), f32::from(ev.position.y)),
                                    glisse: false,
                                    cible: i,
                                });
                            }),
                        )
                        .child(
                            crate::icone::icone_image()
                                .path(icones::chemin(icone_du_type(type_)))
                                .size(u(cx, theme.forme.icon_s))
                                .text_color(faible),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(relative_cle())
                        .h_full()
                        .child(Input::new(&cle)),
                )
                .child(div().flex_1().min_w(px(0.0)).child(valeur));
            liste = liste.child(ligne);
        }

        let ajout = (!lecture_seule).then(|| {
            let en_cours = self.attente.clone().map(|c| {
                let t = self.ctl.type_de_l_attente();
                div()
                    .id("propriete-attente")
                    .flex()
                    .items_center()
                    .min_h(u(cx, HAUTEUR_LIGNE))
                    .child(
                        div()
                            .id("icone-attente")
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .w(u(cx, LARGEUR_ICONE))
                            .h(u(cx, 24.0))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    let (x, y) =
                                        (f32::from(ev.position.x), f32::from(ev.position.y));
                                    this.icone_attente(x, y + 4.0, window, cx);
                                }),
                            )
                            .child(
                                crate::icone::icone_image()
                                    .path(icones::chemin(icone_du_type(t)))
                                    .size(u(cx, theme.forme.icon_s))
                                    .text_color(faible),
                            ),
                    )
                    .child(div().flex_none().w(relative_cle()).child(Input::new(&c)))
            });
            div().flex().flex_col().children(en_cours).child(
                div()
                    .id("ajouter-propriete")
                    .flex()
                    .items_center()
                    .h(u(cx, HAUTEUR_AJOUT))
                    .mt(u(cx, 8.0))
                    .rounded(u(cx, theme.forme.input_radius))
                    .text_color(faible)
                    .hover(move |s| s.bg(survol).text_color(texte))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let replie = this.replie;
                        let s = this.ctl.ajouter(replie);
                        this.appliquer(s, window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .w(u(cx, LARGEUR_ICONE))
                            .child(
                                crate::icone::icone_image()
                                    .path(icones::chemin("plus"))
                                    .size(u(cx, theme.forme.icon_s)),
                            ),
                    )
                    .child(div().ml(u(cx, REMBOURRAGE_VALEUR)).child("Add property")),
            )
        });

        let barre = cible.map(|c| {
            div()
                .absolute()
                .h(px(2.0))
                .w_full()
                .bg(accent)
                .rounded(px(1.0))
                .top(px(c as f32 * (HAUTEUR_LIGNE + 2.0)))
        });

        div()
            .id("proprietes")
            .relative()
            .flex()
            .flex_col()
            .font_family(FAMILLE_INTERFACE)
            .text_color(texte)
            .text_size(u(cx, CORPS_CONTENU))
            .pt(u(cx, 6.4))
            .pb(u(cx, 19.2))
            .pl(u(cx, 6.0))
            .pr(u(cx, 2.0))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.bouger(ev, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, window, cx| this.lacher(ev, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    let entrees = this.ctl.menu_bloc();
                    let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
                    this.ouvrir_menu(&entrees, x, y, None, window, cx);
                }),
            )
            .child(
                div()
                    .pt(u(cx, 3.2))
                    .pb(u(cx, 8.0))
                    .text_size(u(cx, 16.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Properties"),
            )
            .when_not_replie(self.replie, liste, ajout)
            .children(barre)
    }
}

/// La largeur de la colonne des clés.
fn relative_cle() -> gpui_kit::Length {
    gpui_kit::relative(0.3).into()
}

/// `when` écrit à la main : le contenu n'existe pas tant que le bloc est replié.
trait SansContenuReplie: gpui_kit::ParentElement + Sized {
    fn when_not_replie(
        self,
        replie: bool,
        liste: impl IntoElement,
        ajout: Option<impl IntoElement>,
    ) -> Self {
        if replie {
            self
        } else {
            let mut s = self.child(liste);
            if let Some(a) = ajout {
                s = s.child(a);
            }
            s
        }
    }
}

impl<T: gpui_kit::ParentElement + Sized> SansContenuReplie for T {}

impl VueProprietes {
    /// L'éditeur de valeur de la ligne `i`, en élément.
    fn valeur_en_element(&self, i: usize, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = theme_ou_defaut(cx);
        let p = &theme.palette;
        let Some(texte) = self.editeur.read(cx).texte().to_owned().into() else {
            return div().into_any_element();
        };
        let bloc = lire_bloc(&texte);
        let Some(prop) = bloc.as_ref().and_then(|b| b.props.get(i)) else {
            return div().into_any_element();
        };
        let lecture_seule = self.ctl.lecture_seule();
        match &self.lignes[i].valeur {
            Valeur::Case => {
                let coche = est_coche(&prop.value);
                let accent = couleur(p.interactive_accent);
                let bordure = couleur(p.background_modifier_border);
                div()
                    .id(("case", i))
                    .ml(u(cx, REMBOURRAGE_VALEUR))
                    .size(u(cx, COTE_CASE))
                    .rounded(u(cx, 3.0))
                    .border_1()
                    .border_color(if coche { accent } else { bordure })
                    .bg(if coche {
                        accent
                    } else {
                        couleur(p.background_primary)
                    })
                    .when(!lecture_seule, |d| {
                        d.on_click(cx.listener(move |this, _, window, cx| {
                            let texte = this.texte(cx);
                            let courant = lire_bloc(&texte)
                                .and_then(|b| b.props.get(i).map(|p| est_coche(&p.value)))
                                .unwrap_or(false);
                            let s =
                                this.ctl
                                    .definir_valeur(&texte, i, PropertyValue::Bool(!courant));
                            this.appliquer(s, window, cx);
                        }))
                    })
                    .into_any_element()
            }
            Valeur::Champ(c) => div().h_full().child(Input::new(c)).into_any_element(),
            Valeur::Liste(c) => {
                let items = fm::as_list(&prop.value);
                let mut boite = div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(4.0))
                    .py(px(3.0))
                    .px(u(cx, REMBOURRAGE_VALEUR));
                for (j, item) in items.into_iter().enumerate() {
                    boite = boite.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .pl(px(8.0))
                            .pr(px(4.0))
                            .rounded(px(999.0))
                            .bg(couleur(p.background_modifier_hover))
                            .child(div().child(item))
                            .when(!lecture_seule, |d| {
                                d.child(
                                    div()
                                        .id(("retirer", i * 1000 + j))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            let texte = this.texte(cx);
                                            let s = this.ctl.retirer_element(&texte, i, j);
                                            this.appliquer(s, window, cx);
                                        }))
                                        .child(
                                            crate::icone::icone_image()
                                                .path(icones::chemin("x"))
                                                .size(px(12.0))
                                                .text_color(couleur(p.text_faint)),
                                        ),
                                )
                            }),
                    );
                }
                boite
                    .child(div().min_w(px(32.0)).flex_1().child(Input::new(c)))
                    .into_any_element()
            }
        }
    }
}

trait Quand: Sized {
    fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond { f(self) } else { self }
    }
}

impl<T: Sized> Quand for T {}
