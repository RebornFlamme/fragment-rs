// Port de app/src/ui/views/agent/controls/{controls.ts,controls.css} et
// app/src/ui/views/agent/policy/{permissionCard.ts,permission.css} (le rendu)
//! Le rendu GPUI des contrôles du champ, de la puce de contexte et de la carte de permission.
//!
//! Les constantes viennent de `controls.css` et `permission.css` (voir `docs/constantes-rendu.md`,
//! sections `ui/views/agent/controls/controls.css` et `ui/views/agent/policy/permission.css`) ;
//! les jetons `--chat-*` sont des alias de la palette : `--chat-fg` = `text_normal`,
//! `--chat-muted` = `text_muted`, `--chat-subtle` = `background_secondary`, `--chat-border` =
//! `tab_outline_color`, `--chat-bg` = `workspace_bg`, `--chat-accent` = `color_accent`,
//! `--chat-radius` = `radius_ml`, `--chat-radius-sm` = `radius_sm`.
//!
//! Le branchement : [`slots_gpui`] rend les emplacements du champ, [`zone_contexte`] la bande de
//! la puce, [`RenduPermission`] le part `agent-permission` ; l'hôte les pose dans le
//! [`RegistreFil`](crate::chat_fil::RegistreFil).
//!
//! ★ Écarts assumés (voir la PR) : pas d'anneau de focus clavier (`:focus-visible`) sur les boutons ; pas d'infobulle sur les listes (le nom accessible porte le
//! texte, l'infobulle est de la couche de l'hôte) ; le plan du mode plan s'affiche en texte brut
//! tant que l'hôte ne fournit pas de rendu Markdown ([`RenduPermission::avec_rendu_plan`]).

use std::cell::Cell;
use std::rc::Rc;

use fragment_agent::contrat::PermissionDecision;
use fragment_chat::api::ChatPart;
use fragment_chat::api::ChatSlotContext;
use fragment_core::{CommandManager, Keymap};
use fragment_theme::{Couleur, Theme};
use gpui_kit::base::input::{Input, InputState};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, ElementId, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, point, px,
};

use super::carte::{
    ActionCarte, BoutonCarte, CarteModele, MarqueReponse, PART_PERMISSION, VarianteBouton,
};
use super::contexte::{PuceContexte, VuePuce};
use super::modele::{Controle, Controles, EtatAnneau, NiveauUsage, SourceControles};
use crate::chat_fil::registre::{PartVivant, RenduDePart, SlotChat};
use crate::echelle::u;
use crate::icones;
use crate::menu::Menu;
use crate::polices::{FAMILLE_MONOSPACE, famille_interface};
use crate::theme_global::{CouleurGpui, couleur, theme_ou_defaut};

/// Rafraîchit la vue après un choix (la liste ouverte vit hors de l'arbre d'éléments).
pub type Rafraichir = Rc<dyn Fn()>;

/// Rend le Markdown du plan (le moteur de la note, branché par l'hôte).
pub type RenduPlan = Rc<dyn Fn(&str, &mut Window, &mut App) -> AnyElement>;

/// Les jetons de couleur des contrôles et de la carte.
struct Encres {
    fg: CouleurGpui,
    muted: CouleurGpui,
    subtle: CouleurGpui,
    bordure: CouleurGpui,
    fond: CouleurGpui,
    erreur: CouleurGpui,
    survol: CouleurGpui,
    accent: CouleurGpui,
    sur_accent: CouleurGpui,
    anneau_fond: CouleurGpui,
    orange: CouleurGpui,
    secondaire: CouleurGpui,
    bordure_attente: CouleurGpui,
}

impl Encres {
    fn new(t: &Theme) -> Self {
        let p = &t.palette;
        Self {
            fg: couleur(p.text_normal),
            muted: couleur(p.text_muted),
            subtle: couleur(p.background_secondary),
            bordure: couleur(p.tab_outline_color),
            fond: couleur(p.workspace_bg),
            erreur: couleur(p.text_error),
            survol: couleur(p.background_modifier_hover),
            accent: couleur(p.interactive_accent),
            sur_accent: couleur(p.text_on_accent),
            anneau_fond: couleur(p.background_modifier_border),
            orange: couleur(p.color_orange),
            secondaire: couleur(p.background_primary_alt),
            // `color-mix(in srgb, var(--color-accent) 45%, var(--chat-border))`.
            bordure_attente: couleur(p.color_accent.melange(45.0, p.tab_outline_color)),
        }
    }
}

fn vide() -> AnyElement {
    div().hidden().into_any_element()
}

// ── les listes du champ ──────────────────────────────────────────────────────────────────────

/// Un bouton compact qui ouvre une liste sous la forme d'un menu : un libellé et un chevron.
fn bouton_liste(
    c: Controle,
    controles: &Rc<Controles>,
    keymap: &Keymap,
    rafraichir: &Rafraichir,
    t: &Theme,
    cx: &App,
) -> AnyElement {
    let etat = controles.etat(c);
    if etat.masque {
        return vide();
    }
    let e = Encres::new(t);
    let f = &t.forme;
    let origine: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
    let mesure = origine.clone();
    let (fg, muted, survol) = (e.fg, e.muted, e.subtle);
    let mut b = div()
        .id(ElementId::from(SharedString::from(format!(
            "agent-control-{}",
            c.classe()
        ))))
        .role(Role::Button)
        .aria_label(etat.info.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(u(cx, f.size_2_1))
        .max_w(u(cx, 160.0))
        .px(u(cx, f.size_2_3))
        .py(u(cx, f.size_2_1))
        .rounded(u(cx, f.radius_sm))
        .text_color(muted)
        .text_size(u(cx, t.typo.font_ui_smaller))
        .whitespace_nowrap();
    b = famille_interface(b);
    // Là où le menu s'ouvre : au bord haut-gauche du bouton, mesuré à la peinture.
    b = b.child(
        canvas(
            move |bornes, _, _| {
                mesure.set((f32::from(bornes.origin.x), f32::from(bornes.origin.y)));
            },
            |_, (), _, _| (),
        )
        .absolute()
        .size_full(),
    );
    b = b.child(
        div()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .child(etat.libelle.clone()),
    );
    if etat.desactive {
        return b.opacity(0.6).into_any_element();
    }
    let chevron = crate::icone::icone_image()
        .path(icones::chemin("chevron-down"))
        .size(u(cx, f.icon_xs))
        .flex_none();
    let (controles, keymap, rafraichir) = (controles.clone(), keymap.clone(), rafraichir.clone());
    b.child(chevron)
        .cursor_pointer()
        .hover(move |s| s.bg(survol).text_color(fg))
        .on_click(move |_, window, _cx| {
            let mut menu = Menu::new();
            for entree in controles.entrees(c) {
                let (ctl, rafr, id) = (controles.clone(), rafraichir.clone(), entree.id.clone());
                menu.add_item(|i| {
                    i.set_title(&entree.titre)
                        .set_checked(entree.coche)
                        .set_disabled(entree.desactive)
                        .on_click(move || {
                            ctl.choisir(c, &id);
                            rafr();
                        });
                });
            }
            let (x, y) = origine.get();
            menu.show_at_position(x, y - 4.0, &keymap);
            window.refresh();
        })
        .into_any_element()
}

// ── l'anneau d'usage ─────────────────────────────────────────────────────────────────────────

/// Le tracé de l'anneau : une piste pleine et l'arc de la fenêtre remplie, qui part du haut et
/// tourne dans le sens horaire (la viewBox 20 × 20 du TS, rayon 7, trait de 2,5).
fn anneau(a: &EtatAnneau, e: &Encres, cote: f32) -> AnyElement {
    let fraction = a.fraction.clamp(0.0, 1.0) as f32;
    let arc = match a.niveau {
        NiveauUsage::Bas => e.muted,
        NiveauUsage::Moyen => e.orange,
        NiveauUsage::Haut => e.erreur,
    };
    let piste = e.anneau_fond;
    canvas(
        |_, _, _| (),
        move |bornes, (), fenetre, _| {
            let echelle = f32::from(bornes.size.width).min(f32::from(bornes.size.height)) / 20.0;
            let (rayon, trait_) = (7.0 * echelle, 2.5 * echelle);
            let centre = point(
                bornes.origin.x + bornes.size.width / 2.0,
                bornes.origin.y + bornes.size.height / 2.0,
            );
            // 0° = le haut (le TS tourne le svg de -90°).
            let sur = |deg: f32| {
                let a = (deg - 90.0).to_radians();
                point(
                    centre.x + px(rayon * a.cos()),
                    centre.y + px(rayon * a.sin()),
                )
            };
            let rayons = point(px(rayon), px(rayon));
            let mut cercle = PathBuilder::stroke(px(trait_));
            cercle.move_to(sur(0.0));
            cercle.arc_to(rayons, px(0.0), false, true, sur(180.0));
            cercle.arc_to(rayons, px(0.0), false, true, sur(360.0));
            if let Ok(chemin) = cercle.build() {
                fenetre.paint_path(chemin, piste);
            }
            if fraction <= 0.0 {
                return;
            }
            let mut partie = PathBuilder::stroke(px(trait_));
            partie.move_to(sur(0.0));
            if fraction >= 0.999 {
                partie.arc_to(rayons, px(0.0), false, true, sur(180.0));
                partie.arc_to(rayons, px(0.0), false, true, sur(360.0));
            } else {
                let angle = 360.0 * fraction;
                partie.arc_to(rayons, px(0.0), angle > 180.0, true, sur(angle));
            }
            if let Ok(chemin) = partie.build() {
                fenetre.paint_path(chemin, arc);
            }
        },
    )
    .size(px(cote))
    .into_any_element()
}

fn rendu_anneau(controles: &Controles, t: &Theme, cx: &App) -> AnyElement {
    let Some(a) = controles.anneau() else {
        return vide();
    };
    let e = Encres::new(t);
    div()
        .id("agent-usage")
        .role(Role::Image)
        .aria_label(a.label.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .child(anneau(&a, &e, u(cx, t.forme.icon_m).into()))
        .into_any_element()
}

/// Les emplacements `chat:composer-tools@1` de l'agent : moteur, modèle, effort, mode, anneau
/// d'usage. `source` est le fournisseur agent ; `rafraichir` redessine la vue après un choix.
#[must_use]
pub fn slots_gpui(
    source: Rc<dyn SourceControles>,
    keymap: Keymap,
    rafraichir: Rafraichir,
) -> Vec<SlotChat> {
    super::modele::slots_controles()
        .into_iter()
        .map(|slot| {
            let id = slot.id.clone();
            let (source, keymap, rafraichir) = (source.clone(), keymap.clone(), rafraichir.clone());
            let rendre = move |ctx: &ChatSlotContext, _w: &mut Window, cx: &mut App| {
                let controles = Rc::new(Controles::new(source.clone(), ctx.conversation.clone()));
                let t = theme_ou_defaut(cx);
                match Controle::TOUS.into_iter().find(|c| c.id_slot() == id) {
                    Some(c) => bouton_liste(c, &controles, &keymap, &rafraichir, &t, cx),
                    None => rendu_anneau(&controles, &t, cx),
                }
            };
            SlotChat::new(slot, Rc::new(rendre))
        })
        .collect()
}

// ── la puce de contexte ──────────────────────────────────────────────────────────────────────

fn puce(v: &VuePuce, p: &PuceContexte, e: &Encres, t: &Theme, cx: &App) -> AnyElement {
    let f = &t.forme;
    let retirer = p.clone();
    let survol = e.survol;
    let fg = e.fg;
    let mut chip = div()
        .id("agent-context-chip")
        .role(Role::Group)
        .aria_label(v.nom.clone())
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_2))
        .max_w_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .pt(u(cx, f.size_2_1))
        .pb(u(cx, f.size_2_1))
        .pr(u(cx, f.size_2_1))
        .pl(u(cx, f.size_2_3))
        .border(px(f.border_width))
        .border_color(e.bordure)
        .rounded(px(999.0))
        .bg(e.subtle)
        .text_color(e.fg)
        .text_size(u(cx, t.typo.font_ui_smaller));
    chip = famille_interface(chip);
    chip = chip
        .child(
            crate::icone::icone_image()
                .path(icones::chemin(&v.icone))
                .size(u(cx, f.icon_xs))
                .flex_none(),
        )
        .child(div().child(v.titre.clone()));
    if let Some(passage) = &v.passage {
        chip = chip.child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_color(e.muted)
                .child(passage.clone()),
        );
    }
    chip.child(
        div()
            .id("agent-context-remove")
            .role(Role::Button)
            .aria_label("Retirer le contexte")
            .flex()
            .flex_none()
            .p(u(cx, f.size_2_1))
            .rounded_full()
            .text_color(e.muted)
            .cursor_pointer()
            .hover(move |s| s.bg(survol).text_color(fg))
            .on_click(move |_, window, _cx| {
                retirer.retirer();
                window.refresh();
            })
            .child(
                crate::icone::icone_image()
                    .path(icones::chemin("x"))
                    .size(u(cx, f.icon_xs))
                    .flex_none(),
            ),
    )
    .into_any_element()
}

/// La zone de la puce, au-dessus du texte du champ (`.agent-context-zone`).
#[must_use]
pub fn zone_contexte(puce_etat: PuceContexte) -> crate::chat_fil::registre::ZoneChamp {
    Rc::new(move |_window, cx| {
        let Some(v) = puce_etat.vue() else {
            return vide();
        };
        let t = theme_ou_defaut(cx);
        let e = Encres::new(&t);
        div()
            .flex()
            .flex_wrap()
            .gap(u(cx, t.forme.size_2_2))
            .child(puce(&v, &puce_etat, &e, &t, cx))
            .into_any_element()
    })
}

// ── la carte de permission ───────────────────────────────────────────────────────────────────

/// Ce que la carte renvoie au fournisseur : l'id de la demande et la décision.
pub type Reponse = Rc<dyn Fn(&str, PermissionDecision)>;

/// La vue d'une carte : une seule entité, mise à jour à chaque version du part.
pub struct CarteVue {
    modele: CarteModele,
    pourquoi: Entity<InputState>,
    repondre: Reponse,
    rendu_plan: Option<RenduPlan>,
}

impl CarteVue {
    fn new(
        modele: CarteModele,
        repondre: Reponse,
        rendu_plan: Option<RenduPlan>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let invite = modele.invite_pourquoi().to_owned();
        let pourquoi = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_placeholder(invite, window, cx);
            s
        });
        Self {
            modele,
            pourquoi,
            repondre,
            rendu_plan,
        }
    }

    /// Une nouvelle version du part : la carte ne change que si son état, sa demande ou sa note
    /// ont changé (sinon le « pourquoi » tapé reste).
    fn mettre_a_jour(&mut self, modele: CarteModele, window: &mut Window, cx: &mut Context<Self>) {
        if self.modele.mettre_a_jour(modele.data) {
            let invite = self.modele.invite_pourquoi().to_owned();
            self.pourquoi.update(cx, |s, cx| {
                s.set_value(String::new(), window, cx);
                s.set_placeholder(invite, window, cx);
            });
            cx.notify();
        }
    }

    /// Le modèle courant (tests, hôte).
    #[must_use]
    pub fn modele(&self) -> &CarteModele {
        &self.modele
    }

    fn cliquer(&mut self, action: ActionCarte, cx: &mut Context<Self>) {
        self.modele.pourquoi = self.pourquoi.read(cx).value().to_string();
        if let Some(d) = self.modele.decision(action) {
            (self.repondre)(&self.modele.data.id, d);
        }
    }

    fn bouton(&self, b: &BoutonCarte, e: &Encres, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let f = &t.forme;
        let action = b.action;
        let (subtle, accent) = (e.subtle, e.accent);
        let mut el = div()
            .id(SharedString::from(format!(
                "agent-permission-button-{}",
                b.libelle
            )))
            .role(Role::Button)
            .px(u(cx, f.size_4_3))
            .py(u(cx, f.size_2_2))
            .border(px(f.border_width))
            .rounded(u(cx, f.button_radius))
            .cursor_pointer()
            .child(b.libelle);
        el = match b.variante {
            VarianteBouton::Principal => {
                el.bg(accent).border_color(accent).text_color(e.sur_accent)
            }
            VarianteBouton::Normal => el
                .bg(e.fond)
                .border_color(e.bordure)
                .text_color(e.fg)
                .hover(move |s| s.bg(subtle)),
            VarianteBouton::Refus => el
                .bg(e.fond)
                .border_color(e.bordure)
                .text_color(e.erreur)
                .hover(move |s| s.bg(subtle)),
        };
        el.on_click(cx.listener(move |this, _, _, cx| this.cliquer(action, cx)))
            .into_any_element()
    }
}

impl Render for CarteVue {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        let e = Encres::new(&t);
        let f = &t.forme;
        let mut carte = div()
            .id("agent-permission")
            .flex()
            .flex_col()
            .my(u(cx, f.size_4_1))
            .text_color(e.fg)
            .text_size(u(cx, t.typo.font_ui_small));
        carte = famille_interface(carte);
        // ── Répondue : une ligne discrète.
        if let Some(ligne) = self.modele.ligne_repondue() {
            let mark = match ligne.marque {
                MarqueReponse::Coche => ("check", e.muted),
                MarqueReponse::Croix => ("x", e.erreur),
            };
            return carte
                .flex_row()
                .items_center()
                .gap(u(cx, f.size_2_3))
                .px(u(cx, f.size_4_2))
                .py(u(cx, f.size_2_2))
                .rounded(u(cx, f.radius_ml))
                .border(px(f.border_width))
                .border_color(couleur(Couleur::TRANSPARENT))
                .text_color(e.muted)
                .child(
                    crate::icone::icone_image()
                        .path(icones::chemin(mark.0))
                        .size(u(cx, f.icon_s))
                        .flex_none()
                        .text_color(mark.1),
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(ligne.texte),
                )
                .into_any_element();
        }
        // ── En attente.
        carte = carte
            .gap(u(cx, f.size_4_2))
            .p(u(cx, f.size_4_3))
            .border(px(f.border_width))
            .border_color(e.bordure_attente)
            .rounded(u(cx, f.radius_ml))
            .bg(e.secondaire);
        let d = &self.modele.description;
        carte = carte.child(
            div()
                .flex()
                .items_center()
                .gap(u(cx, f.size_4_2))
                .font_weight(FontWeight(f32::from(
                    u16::try_from(t.typo.font_semibold).unwrap_or(600),
                )))
                .child(
                    crate::icone::icone_image()
                        .path(icones::chemin(&d.icon))
                        .size(u(cx, f.icon_s))
                        .flex_none()
                        .text_color(e.muted),
                )
                .child(d.verb.clone()),
        );
        if let Some(plan) = self.modele.plan() {
            let corps = match &self.rendu_plan {
                Some(rendre) => rendre(plan, window, cx),
                None => div().child(plan.to_owned()).into_any_element(),
            };
            carte = carte.child(
                div()
                    .id("agent-permission-plan")
                    .max_h(u(cx, 320.0))
                    .overflow_y_scroll()
                    .px(u(cx, f.size_4_3))
                    .py(u(cx, f.size_4_2))
                    .rounded(u(cx, f.code_radius))
                    .bg(e.subtle)
                    .child(corps),
            );
        } else if let Some(detail) = self.modele.detail() {
            let mut code = div()
                .id("agent-permission-detail")
                .max_h(u(cx, 200.0))
                .overflow_y_scroll()
                .px(u(cx, f.size_4_3))
                .py(u(cx, f.size_4_2))
                .rounded(u(cx, f.code_radius))
                .bg(e.subtle)
                .font_family(FAMILLE_MONOSPACE)
                .text_size(u(cx, t.typo.font_ui_smaller));
            code = code.child(detail.to_owned());
            carte = carte.child(code);
        }
        if let Some(raison) = self.modele.raison() {
            carte = carte.child(div().text_color(e.muted).child(raison.to_owned()));
        }
        let mut actions = div().flex().flex_wrap().gap(u(cx, f.size_4_2));
        for b in self.modele.boutons() {
            actions = actions.child(self.bouton(&b, &e, &t, cx));
        }
        carte
            .child(actions)
            .child(
                div()
                    .px(u(cx, f.size_4_2))
                    .py(u(cx, f.size_2_2))
                    .border(px(f.border_width))
                    .border_color(e.bordure)
                    .rounded(u(cx, f.input_radius))
                    .bg(e.fond)
                    .text_color(e.fg)
                    .child(Input::new(&self.pourquoi)),
            )
            .into_any_element()
    }
}

/// Le part vivant d'une carte : mis à jour à chaque version du part.
pub struct PartPermission {
    vue: Entity<CarteVue>,
}

impl PartPermission {
    /// L'entité de la carte (tests, hôte).
    #[must_use]
    pub fn carte(&self) -> &Entity<CarteVue> {
        &self.vue
    }
}

impl PartVivant for PartPermission {
    fn vue(&self) -> AnyView {
        self.vue.clone().into()
    }

    fn mettre_a_jour(
        &mut self,
        part: &ChatPart,
        _ctx: &ChatSlotContext,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let Some(modele) = CarteModele::depuis_part(part) else {
            return false;
        };
        self.vue
            .update(cx, |v, cx| v.mettre_a_jour(modele, window, cx));
        true
    }
}

/// Le rendu du type de part `agent-permission` (`chat:part@1`).
pub struct RenduPermission {
    repondre: Reponse,
    rendu_plan: Option<RenduPlan>,
}

impl RenduPermission {
    /// `repondre` reçoit l'id de la demande et la décision : le fournisseur la tient en attente.
    #[must_use]
    pub fn new(repondre: Reponse) -> Self {
        Self {
            repondre,
            rendu_plan: None,
        }
    }

    /// Branche le rendu Markdown du plan.
    #[must_use]
    pub fn avec_rendu_plan(mut self, rendu: RenduPlan) -> Self {
        self.rendu_plan = Some(rendu);
        self
    }
}

impl RenduDePart for RenduPermission {
    fn type_part(&self) -> &str {
        PART_PERMISSION
    }

    fn creer(
        &self,
        part: &ChatPart,
        _ctx: &ChatSlotContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>> {
        let modele = CarteModele::depuis_part(part)?;
        let (repondre, plan) = (self.repondre.clone(), self.rendu_plan.clone());
        let vue = cx.new(|cx| CarteVue::new(modele, repondre, plan, window, cx));
        Some(Box::new(PartPermission { vue }))
    }
}

/// Enregistre les commandes de scène `pixels:agent_controles` : aucune ici (les scènes
/// `t7-permission-carte` et `t7-controles` sont posées par `fr-t7-scenes-b`, qui monte ces
/// composants).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
