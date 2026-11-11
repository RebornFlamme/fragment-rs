// Port de app/src/ui/settings/SettingsWindow.ts (la fenêtre : titre, rail, contenu) et de
// app/src/ui/core/settings/{Setting.ts, settings.css} (la ligne de réglage, ses contrôles).
//! La fenêtre de réglages, rendue depuis des définitions DÉCLARATIVES.
//!
//! Un onglet fournit `Vec<SettingDefinition>` (fragment-core) ; la fenêtre les relit à CHAQUE
//! rendu. Un réglage changé ailleurs (zoom par Ctrl+=, par la palette) se voit donc au prochain
//! dessin, sans la mécanique « ré-afficher l'onglet si la piste ment » du TS.
//!
//! ★ Pas porté ici (voir `parite.toml`) : la saisie de la recherche (champ statique), l'ouverture de la
//!   liste déroulante et le sélecteur de couleur natif, les définitions page / liste / arbre / action /
//!   libre, l'espacement des lettres des titres de groupe (GPUI n'a pas de `letter-spacing`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_core::{
    SettingControl, SettingDefinition, SettingDefinitionControl, SettingDefinitionItem,
    SettingDropdownControl, SettingSliderControl,
};
use fragment_theme::{Aplat, Couleur, Theme};
use gpui_kit::{
    AbsoluteLength, AnyElement, App, Bounds, ClickEvent, Context, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Window, canvas, div, svg,
};

use super::curseur::{Curseur, DIAMETRE_POUCE, HAUTEUR_PISTE, LARGEUR_PISTE};
use crate::echelle::{Echelle, calees, u};
use crate::flottant::{bordure, texte};
use crate::icones;
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::theme_global::{couleur, theme_ou_defaut};

/// Les définitions d'un onglet, relues à chaque rendu (le thème courant en argument : l'accent du thème).
pub type Definitions = Rc<dyn Fn(&Theme) -> Vec<SettingDefinition>>;

/// Les trois groupes du rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionOnglet {
    Options,
    Core,
    Community,
}

impl SectionOnglet {
    fn titre(self) -> &'static str {
        match self {
            Self::Options => "Options",
            Self::Core => "Core plugins",
            Self::Community => "Community plugins",
        }
    }
}

/// Un onglet du rail. `definitions: None` : onglet d'une tranche à venir (rail seulement).
#[derive(Clone)]
pub struct OngletReglages {
    pub id: String,
    pub nom: String,
    pub icone: Option<String>,
    pub section: SectionOnglet,
    pub definitions: Option<Definitions>,
}

impl OngletReglages {
    pub fn new(id: &str, nom: &str, icone: &str, section: SectionOnglet) -> Self {
        Self {
            id: id.to_owned(),
            nom: nom.to_owned(),
            icone: Some(icone.to_owned()),
            section,
            definitions: None,
        }
    }

    pub fn avec_definitions(mut self, d: Definitions) -> Self {
        self.definitions = Some(d);
        self
    }
}

/// Le glisser d'un curseur en cours.
struct Glisse {
    cle: String,
    curseur: Curseur,
    gauche: f32,
    on_change: Option<Rc<dyn Fn(f64)>>,
}

/// La fenêtre de réglages.
pub struct FenetreReglages {
    titre: SharedString,
    onglets: Vec<OngletReglages>,
    actif: usize,
    glisse: Option<Glisse>,
    /// Boîtes des pistes (fenêtre), relevées à la peinture : le pointeur s'y mesure.
    bornes: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// La largeur de la fenêtre (px logiques), relevée au rendu : la largeur des cartes en dépend.
    largeur: f32,
}

/// Un morceau du contenu et ses marges verticales (qui se FUSIONNENT entre voisins, comme en CSS).
struct Bloc {
    haut: f32,
    bas: f32,
    el: AnyElement,
}

/// Hauteur de `.settings-titlebar` (`height: 32px`), SANS la bordure basse : ★ la boîte est en
/// `content-box` (le défaut de Fragment), la bordure s'ajoute : 33 px en tout. Comptée dedans, tout
/// le rail et le contenu montaient d'un pixel.
const TITRE_HAUTEUR: f32 = 32.0;
/// Largeur du contenu du rail (`.vertical-tab-header { width }`, hors rembourrage).
const RAIL_LARGEUR: f32 = 220.0;
/// `.vertical-tab-content { max-width }`.
const CONTENU_LARGEUR: f32 = 760.0;
/// La piste d'une bascule (`.checkbox-container { width: 42px }`).
const LARGEUR_BASCULE: f32 = 42.0;

/// La largeur totale du rail : contenu, rembourrage et bordure droite (`content-box`).
fn largeur_rail(t: &Theme) -> f32 {
    RAIL_LARGEUR + 2.0 * t.forme.size_4_2 + t.forme.border_width
}

/// ★ La règle de flexbox de Chromium (CSS Flexbox §9.7) pour la ligne d'une carte : l'information
/// (`flex: 1 1 auto`, marge droite) et les contrôles (`flex: 0 1 auto`), dans `place` px. Chacun
/// est `(base, min)` : largeur max-content et min-content ; l'information a en plus sa marge. Rend
/// les largeurs de CONTENU (marge exclue).
///
/// - Place en trop : l'information seule grandit (`flex-grow: 1`).
/// - Place en moins : chacun rétrécit au prorata de sa base (`flex-shrink` × base), sans passer sous
///   son min-content (`min-width: auto`) ; un élément bloqué rend sa part à l'autre.
pub fn repartir(place: f32, info: (f32, f32, f32), ctl: (f32, f32)) -> (f32, f32) {
    let ((ib, imin, marge), (cb, cmin)) = (info, ctl);
    let libre = place - (ib + marge + cb);
    if libre >= 0.0 {
        return (ib + libre, cb);
    }
    let total = ib + cb;
    if total <= 0.0 {
        return (ib, cb);
    }
    let (mut i, mut c) = (ib + libre * ib / total, cb + libre * cb / total);
    if i < imin {
        i = imin;
        c = (place - marge - i).max(cmin);
    } else if c < cmin {
        c = cmin;
        i = (place - marge - c).max(imin);
    }
    (i, c)
}

impl FenetreReglages {
    pub fn new(titre: impl Into<SharedString>, onglets: Vec<OngletReglages>) -> Self {
        Self {
            titre: titre.into(),
            onglets,
            actif: 0,
            glisse: None,
            bornes: Rc::default(),
            largeur: 0.0,
        }
    }

    /// Active l'onglet `id` (sans effet s'il n'existe pas).
    pub fn activer(&mut self, id: &str) {
        if let Some(i) = self.onglets.iter().position(|o| o.id == id) {
            self.actif = i;
        }
    }

    // ── Le rail ────────────────────────────────────────────────────────────

    fn rail(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let (f, ty, c) = (&t.forme, &t.typo, &t.palette);
        // Les blocs, dans l'ordre, avec leur hauteur EXACTE (logique) : la recherche (marges 2 et 6
        // autour d'un champ de 33,5), puis par groupe son titre (14 + 18 + 4) et ses lignes
        // (6 + 19,5 + 6). Chromium les arrondit arête par arête : on calcule les entiers à poser.
        let champ = 2.0 * f.border_width + 2.0 * f.size_2_3 + ty.font_ui_medium * 1.5;
        let ligne = 2.0 * f.size_2_3 + ty.font_ui_medium * 1.5;
        let titre = f.size_2_7 + ty.font_ui_small * 1.5 + f.size_4_1;
        let mut hauteurs = vec![f.size_2_1, champ, f.size_2_3];
        let mut groupes: Vec<(SectionOnglet, Vec<usize>)> = Vec::new();
        for s in [
            SectionOnglet::Options,
            SectionOnglet::Core,
            SectionOnglet::Community,
        ] {
            let idx: Vec<usize> = (0..self.onglets.len())
                .filter(|&i| self.onglets[i].section == s)
                .collect();
            if idx.is_empty() {
                continue;
            }
            hauteurs.push(titre);
            hauteurs.extend(std::iter::repeat_n(ligne, idx.len()));
            groupes.push((s, idx));
        }
        // L'arête haute du rail : barre de titre + rembourrage du haut.
        let origine = TITRE_HAUTEUR + f.border_width + f.size_4_3;
        let mut h = calees(cx, origine, &hauteurs).into_iter();
        let mut suivant = || h.next().unwrap_or_default();

        let mut colonne = div().flex().flex_col();
        // La recherche : champ statique (la saisie vient avec l'index de recherche).
        colonne = colonne.child(div().h(suivant()));
        let haut_champ = suivant();
        colonne = colonne.child(
            div()
                .h(haut_champ)
                .mx(u(cx, f.size_2_3))
                .child(self.champ_recherche(t, cx)),
        );
        colonne = colonne.child(div().h(suivant()));

        for (section, idx) in groupes {
            colonne = colonne.child(texte(
                div()
                    .h(suivant())
                    .pt(u(cx, f.size_2_7))
                    .px(u(cx, f.size_2_5))
                    .text_color(couleur(c.text_muted))
                    .child(section.titre().to_uppercase()),
                cx,
                ty.font_ui_small,
                1.5,
            ));
            for i in idx {
                let o = &self.onglets[i];
                let actif = i == self.actif;
                let survol = couleur(c.aplat(Aplat::Survol, c.background_secondary));
                let encre = couleur(if actif { c.text_normal } else { c.text_muted });
                let mut item = div()
                    .id(("onglet", i))
                    .h(suivant())
                    .flex()
                    .items_center()
                    .gap(u(cx, f.size_4_2))
                    .px(u(cx, f.size_2_5))
                    .rounded(u(cx, f.radius_sm))
                    .cursor_pointer()
                    .text_color(encre);
                item = if actif {
                    item.bg(couleur(c.aplat(Aplat::SurvolActif, c.background_secondary)))
                } else {
                    item.hover(move |s| s.bg(survol))
                };
                if let Some(nom) = &o.icone {
                    item = item.child(
                        svg()
                            .path(icones::chemin(nom))
                            .size(u(cx, f.icon_s))
                            .flex_none()
                            .text_color(encre),
                    );
                }
                let item = texte(item.child(o.nom.clone()), cx, ty.font_ui_medium, 1.5).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.actif = i;
                        this.glisse = None;
                        cx.notify();
                    }),
                );
                colonne = colonne.child(item);
            }
        }

        let boite = div()
            .flex_none()
            .w(u(cx, RAIL_LARGEUR + 2.0 * f.size_4_2 + f.border_width))
            .h_full()
            .py(u(cx, f.size_4_3))
            .px(u(cx, f.size_4_2))
            .bg(couleur(c.background_secondary))
            .child(colonne);
        cote_droit(boite, u(cx, f.border_width))
            .border_color(couleur(c.tab_outline_color))
            .into_any_element()
    }

    /// Le champ « Search settings... » : bordure, loupe, texte indicatif.
    fn champ_recherche(&self, t: &Theme, cx: &Context<Self>) -> AnyElement {
        let (f, ty, c) = (&t.forme, &t.typo, &t.palette);
        let champ = div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .pl(u(cx, f.size_2_15))
            .rounded(u(cx, f.radius_sm))
            .bg(couleur(c.background_secondary))
            .text_color(couleur(c.text_muted))
            .child(
                div().absolute().left(u(cx, 9.0)).child(
                    svg()
                        .path(icones::chemin("search"))
                        .size(u(cx, f.icon_xs))
                        .text_color(couleur(c.text_muted)),
                ),
            )
            .child("Search settings...");
        let champ =
            bordure(champ, u(cx, f.border_width)).border_color(couleur(c.tab_outline_color));
        texte(champ, cx, ty.font_ui_medium, 1.5).into_any_element()
    }

    // ── Le contenu de l'onglet actif ───────────────────────────────────────

    fn contenu(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let f = &t.forme;
        let defs = self
            .onglets
            .get(self.actif)
            .and_then(|o| o.definitions.as_ref())
            .map(|d| d(t))
            .unwrap_or_default();
        let mut blocs: Vec<Bloc> = Vec::new();
        for (i, d) in defs.iter().enumerate() {
            self.definition(d, &i.to_string(), i == 0, &mut blocs, t, cx);
        }
        // Les marges de blocs voisins se fusionnent (la plus grande gagne) ; la première et la
        // dernière restent dans le rembourrage.
        let mut colonne = div().flex().flex_col();
        let mut precedent: Option<f32> = None;
        for b in blocs {
            let ecart = precedent.map_or(0.0, |bas| bas.max(b.haut));
            colonne = colonne.child(div().mt(u(cx, ecart)).child(b.el));
            precedent = Some(b.bas);
        }
        div()
            .id("reglages-contenu")
            .flex_1()
            .min_w(u(cx, 0.0))
            .h_full()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(u(cx, CONTENU_LARGEUR + 2.0 * f.size_4_6))
                    .py(u(cx, f.size_4_5))
                    .px(u(cx, f.size_4_6))
                    .child(colonne),
            )
            .into_any_element()
    }

    /// Ajoute les blocs d'une définition. `premier` : `:first-child` (l'en-tête perd sa marge haute).
    fn definition(
        &self,
        d: &SettingDefinition,
        chemin: &str,
        premier: bool,
        blocs: &mut Vec<Bloc>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) {
        let f = &t.forme;
        match d {
            SettingDefinition::Control(SettingDefinitionControl {
                name,
                description,
                control,
                ..
            }) => {
                let largeurs = Self::largeur_controle(control, t, cx).into_iter().collect();
                let ctl = self.controle(&format!("{chemin}-0"), control, t, cx);
                let carte = self.carte(
                    name,
                    description.as_deref(),
                    ctl.into_iter().collect(),
                    largeurs,
                    t,
                    cx,
                );
                blocs.push(Bloc {
                    haut: 0.0,
                    bas: f.size_2_5,
                    el: carte,
                });
            }
            SettingDefinition::Item(SettingDefinitionItem {
                name,
                description,
                controls,
                ..
            }) => {
                let ctl: Vec<AnyElement> = controls
                    .iter()
                    .enumerate()
                    .filter_map(|(k, c)| self.controle(&format!("{chemin}-{k}"), c, t, cx))
                    .collect();
                let largeurs = controls
                    .iter()
                    .filter_map(|c| Self::largeur_controle(c, t, cx))
                    .collect();
                let carte = self.carte(name, description.as_deref(), ctl, largeurs, t, cx);
                blocs.push(Bloc {
                    haut: 0.0,
                    bas: f.size_2_5,
                    el: carte,
                });
            }
            SettingDefinition::Group(g) => {
                blocs.push(self.entete(&g.name, premier, t, cx));
                for (i, sous) in g.items.iter().enumerate() {
                    self.definition(sous, &format!("{chemin}.{i}"), false, blocs, t, cx);
                }
            }
            // Pages, listes, arbres, actions, rendus libres : avec les onglets qui les emploient.
            _ => {}
        }
    }

    /// `.setting-item-heading` : un titre, sans carte.
    fn entete(&self, nom: &str, premier: bool, t: &Theme, cx: &Context<Self>) -> Bloc {
        let (f, c) = (&t.forme, &t.palette);
        let el = div()
            .pt(u(cx, f.size_2_5))
            .px(u(cx, f.size_2_1))
            .pb(u(cx, f.size_4_1))
            .text_color(couleur(c.text_normal))
            .child(nom.to_owned());
        Bloc {
            haut: if premier { 0.0 } else { f.size_4_2 },
            bas: f.size_4_1,
            el: texte(el, cx, 16.0, 1.5)
                .font_weight(FontWeight::SEMIBOLD)
                .into_any_element(),
        }
    }

    /// `.setting-item` : une carte, infos à gauche, contrôles à droite.
    fn carte(
        &self,
        nom: &str,
        desc: Option<&str>,
        controles: Vec<AnyElement>,
        largeurs: Vec<f32>,
        t: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (f, ty, c) = (&t.forme, &t.typo, &t.palette);
        // ★ La répartition de la ligne, calculée comme Chromium (flexbox §9.7) au lieu d'être laissée à
        //   taffy : le texte de GPUI n'a pas de largeur min-content (le mot le plus long), la
        //   description refusait donc de se replier et poussait les contrôles.
        let texte_de = |t: &str, taille: f32| largeur_texte(cx, t, FontWeight::NORMAL, taille);
        let mots = |t: &str, taille: f32| {
            t.split_whitespace()
                .map(|m| texte_de(m, taille))
                .fold(0.0_f32, f32::max)
        };
        let desc_pleine = desc.filter(|d| !d.is_empty());
        let info_base = desc_pleine
            .map_or(0.0, |d| texte_de(d, ty.font_ui_small))
            .max(texte_de(nom, ty.font_ui_normal));
        let info_min = desc_pleine
            .map_or(0.0, |d| mots(d, ty.font_ui_small))
            .max(mots(nom, ty.font_ui_normal));
        let n = largeurs.len();
        let ctl_base = largeurs.iter().sum::<f32>() + f.size_4_2 * n.saturating_sub(1) as f32;
        let ctl_min = largeurs.iter().copied().fold(0.0_f32, f32::max);
        let colonne = (self.largeur - largeur_rail(t)).min(CONTENU_LARGEUR + 2.0 * f.size_4_6)
            - 2.0 * f.size_4_6;
        let interieur = colonne - 2.0 * f.size_4_4;
        let (info_l, ctl_l) = repartir(
            interieur - if n > 0 { f.size_4_4 } else { 0.0 },
            (info_base, info_min, f.size_4_5),
            (ctl_base, ctl_min),
        );
        let mut info = div()
            .flex_none()
            .w(u(cx, info_l))
            .mr(u(cx, f.size_4_5))
            .child(texte(
                div()
                    .text_color(couleur(c.text_normal))
                    .child(nom.to_owned()),
                cx,
                ty.font_ui_normal,
                1.5,
            ));
        if let Some(d) = desc.filter(|d| !d.is_empty()) {
            info = info.child(texte(
                div()
                    .mt(u(cx, f.size_2_1))
                    .text_color(couleur(c.text_muted))
                    .child(d.to_owned()),
                cx,
                ty.font_ui_small,
                1.4,
            ));
        }
        let controles = div()
            .flex()
            .flex_wrap()
            .flex_none()
            .w(u(cx, ctl_l))
            .items_center()
            .justify_end()
            .gap(u(cx, f.size_4_2))
            .children(controles);
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(u(cx, f.size_4_4))
            .py(u(cx, f.size_2_7))
            .px(u(cx, f.size_4_4))
            .rounded(u(cx, f.radius_ml))
            .bg(couleur(c.workspace_bg))
            .child(info)
            .child(controles)
            .into_any_element()
    }

    // ── Les contrôles ──────────────────────────────────────────────────────

    /// La largeur max-content (px logiques) d'un contrôle rendu par [`Self::controle`] ; `None` pour
    /// ceux qui ne sont pas rendus.
    fn largeur_controle(c: &SettingControl, t: &Theme, cx: &App) -> Option<f32> {
        let (f, ty) = (&t.forme, &t.typo);
        match c {
            SettingControl::Slider(s) => {
                let etiquette = s.format.as_ref().map(|fmt| {
                    let texte = fmt(s.value);
                    largeur_texte(cx, &texte, FontWeight::NORMAL, ty.font_ui_medium)
                        .max(4.5 * ty.font_ui_medium)
                });
                Some(etiquette.map_or(0.0, |e| e + f.size_4_2) + LARGEUR_PISTE)
            }
            SettingControl::Dropdown(d) => {
                let texte_max = d
                    .options
                    .iter()
                    .map(|(_, l)| largeur_texte(cx, l, FontWeight::NORMAL, ty.font_ui_medium))
                    .fold(0.0_f32, f32::max);
                Some(texte_max + f.size_4_2 + f.size_4_7 + 2.0 * f.border_width)
            }
            SettingControl::Color(_) | SettingControl::ExtraButton(_) => Some(f.size_4_7),
            SettingControl::Toggle(_) => Some(LARGEUR_BASCULE),
            _ => None,
        }
    }

    fn controle(
        &self,
        cle: &str,
        c: &SettingControl,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (f, p) = (&t.forme, &t.palette);
        match c {
            SettingControl::Slider(s) => Some(self.curseur(cle, s, t, cx)),
            SettingControl::Dropdown(d) => Some(Self::liste(d, t, cx)),
            SettingControl::Color(k) => {
                let pastille = div()
                    .size(u(cx, f.size_4_7))
                    .flex_none()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(couleur(couleur_depuis_hex(&k.value)));
                Some(
                    bordure(pastille, u(cx, f.border_width))
                        .border_color(couleur(p.tab_outline_color))
                        .into_any_element(),
                )
            }
            SettingControl::ExtraButton(b) => {
                let action = b.on_click.clone();
                let survol = couleur(p.background_modifier_hover);
                let normal = couleur(p.text_normal);
                let el = div()
                    .id(SharedString::from(format!("extra-{cle}")))
                    .size(u(cx, f.size_4_7))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(u(cx, f.radius_sm))
                    .cursor_pointer()
                    .text_color(couleur(p.text_muted))
                    .hover(move |s| s.bg(survol).text_color(normal))
                    .child(
                        svg()
                            .path(icones::chemin(&b.icon))
                            .size(u(cx, f.icon_s))
                            .text_color(couleur(p.text_muted)),
                    )
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        action();
                        cx.refresh_windows();
                    }));
                Some(el.into_any_element())
            }
            SettingControl::Toggle(b) => {
                let valeur = b.value;
                let rappel = b.on_change.clone();
                let piste = div()
                    .id(SharedString::from(format!("bascule-{cle}")))
                    .relative()
                    .flex_none()
                    .w(u(cx, LARGEUR_BASCULE))
                    .h(u(cx, 24.0))
                    .rounded(u(cx, f.radius_l))
                    .cursor_pointer()
                    .bg(couleur(if valeur {
                        p.color_accent
                    } else {
                        p.background_modifier_border_hover
                    }))
                    .child(
                        div()
                            .absolute()
                            .top(u(cx, 3.0))
                            .left(u(cx, if valeur { 21.0 } else { 3.0 }))
                            .size(u(cx, 18.0))
                            .rounded_full()
                            .bg(couleur(p.toggle_thumb_color)),
                    )
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        if let Some(r) = &rappel {
                            r(!valeur);
                        }
                        cx.refresh_windows();
                    }));
                Some(piste.into_any_element())
            }
            // Texte, nombre, fichier, secret… : avec les onglets qui les emploient.
            _ => None,
        }
    }

    /// `select.dropdown`, fermé : la largeur est celle de la plus longue option.
    fn liste(d: &SettingDropdownControl, t: &Theme, cx: &Context<Self>) -> AnyElement {
        let (f, ty, p) = (&t.forme, &t.typo, &t.palette);
        let actuel = d
            .options
            .iter()
            .find(|(v, _)| *v == d.value)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let ligne = 18.0;
        // La largeur de la boîte : le plus long libellé + rembourrage et bordures, arrondie comme Chromium.
        let texte_max = d
            .options
            .iter()
            .map(|(_, l)| largeur_texte(cx, l, FontWeight::NORMAL, ty.font_ui_medium))
            .fold(0.0_f32, f32::max);
        let ajout = f.size_4_2 + f.size_4_7 + 2.0 * f.border_width;
        let boite = div()
            .relative()
            .flex_none()
            .w(largeur_ajustee(cx, texte_max, ajout))
            .h(u(cx, 2.0 * f.size_2_3 + 2.0 * f.border_width + ligne))
            .flex()
            .items_center()
            .pl(u(cx, f.size_4_2))
            .pr(u(cx, f.size_4_7))
            .rounded(u(cx, f.radius_sm))
            .bg(couleur(p.background_secondary))
            .text_color(couleur(p.text_normal))
            .child(
                div()
                    .absolute()
                    .left(u(cx, f.size_4_2 + f.border_width))
                    .child(actuel),
            )
            .child(
                div().absolute().right(u(cx, f.size_2_3)).child(
                    svg()
                        .path(icones::chemin("chevrons-up-down"))
                        .size(u(cx, f.icon_s))
                        // ★ Le CSS écrit ici un gris littéral (#8f8e89, dans une URL de données) qui
                        // n'est pas un jeton : le plus proche est `text_muted` (écart suivi).
                        .text_color(couleur(p.text_muted)),
                ),
            );
        let boite =
            bordure(boite, u(cx, f.border_width)).border_color(couleur(p.tab_outline_color));
        texte(boite, cx, ty.font_ui_medium, 1.5).into_any_element()
    }

    /// `input.slider` + l'étiquette de valeur (`.slider-value`).
    fn curseur(
        &self,
        cle: &str,
        s: &SettingSliderControl,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (f, ty, p) = (&t.forme, &t.typo, &t.palette);
        let instant = s.instant.unwrap_or(true);
        let (min, max, pas, valeur) = (s.min, s.max, s.step.unwrap_or(1.0), s.value);
        let modele = match &self.glisse {
            Some(g) if g.cle == cle => g.curseur.clone(),
            _ => Curseur::new(min, max, pas, instant, valeur),
        };
        let centre = Curseur::centre_pouce(modele.fraction());
        let etiquette = s.format.as_ref().map(|fmt| fmt(modele.valeur_affichee()));

        let bornes = self.bornes.clone();
        let (cle_canvas, cle_appui) = (cle.to_owned(), cle.to_owned());
        let on_change = s.on_change.clone();

        let piste = div()
            .id(SharedString::from(format!("curseur-{cle}")))
            .relative()
            .flex_none()
            .w(u(cx, LARGEUR_PISTE))
            .h(u(cx, DIAMETRE_POUCE))
            .flex()
            .items_center()
            .cursor_pointer()
            .child(
                canvas(
                    move |b, _, _| {
                        bornes.borrow_mut().insert(cle_canvas.clone(), b);
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .w_full()
                    .h(u(cx, HAUTEUR_PISTE))
                    .rounded(u(cx, f.radius_xxs))
                    .bg(couleur(p.tab_outline_color)),
            )
            .child(
                div()
                    .absolute()
                    .left(u(cx, centre - DIAMETRE_POUCE / 2.0))
                    .size(u(cx, DIAMETRE_POUCE))
                    .rounded_full()
                    .bg(couleur(p.color_accent)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    let gauche = this
                        .bornes
                        .borrow()
                        .get(&cle_appui)
                        .map_or(0.0, |b| f32::from(b.origin.x));
                    let mut curseur = Curseur::new(min, max, pas, instant, valeur);
                    if let (Some(v), Some(r)) = (
                        curseur.appuyer(f32::from(ev.position.x), gauche),
                        &on_change,
                    ) {
                        r(v);
                    }
                    this.glisse = Some(Glisse {
                        cle: cle_appui.clone(),
                        curseur,
                        gauche,
                        on_change: on_change.clone(),
                    });
                    cx.notify();
                }),
            );

        let mut ligne = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(u(cx, f.size_4_2));
        if let Some(e) = etiquette {
            ligne = ligne.child(texte(
                div()
                    .min_w(u(cx, 4.5 * ty.font_ui_medium))
                    .text_right()
                    .text_color(couleur(p.text_muted))
                    .child(e),
                cx,
                ty.font_ui_medium,
                1.5,
            ));
        }
        ligne.child(piste).into_any_element()
    }

    // ── La barre de titre ──────────────────────────────────────────────────

    fn barre_titre(&self, t: &Theme, cx: &Context<Self>) -> AnyElement {
        let (f, ty, c) = (&t.forme, &t.typo, &t.palette);
        let titre = texte(
            div()
                .h(u(cx, TITRE_HAUTEUR + f.border_width))
                .flex()
                .items_center()
                .justify_center()
                .px(u(cx, f.size_2_7))
                .bg(couleur(c.background_secondary))
                .text_color(couleur(c.text_muted))
                .child(self.titre.clone()),
            cx,
            ty.font_ui_medium,
            1.5,
        );
        let titre =
            cote_bas(titre, u(cx, f.border_width)).border_color(couleur(c.tab_outline_color));
        // Les boutons de fenêtre : ceux de la fenêtre principale (`WindowControls`, le même composant
        // chez Fragment) : 46 × 34, trait fin, centrés à la Chromium, posés sur le coin haut droit.
        let boutons = crate::splits::boutons::rendre_boutons(cx);
        div()
            .relative()
            .flex_none()
            .child(titre)
            .child(boutons)
            .into_any_element()
    }
}

impl Render for FenetreReglages {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        self.largeur = f32::from(window.viewport_size().width) / Echelle::courante(cx).zoom();
        let c = &t.palette;
        let corps = div()
            .flex()
            .flex_1()
            .min_h(u(cx, 0.0))
            .child(self.rail(&t, cx))
            .child(self.contenu(&t, cx));
        let racine = div()
            .id("fenetre-reglages")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(couleur(c.background_secondary))
            .text_color(couleur(c.text_normal))
            .child(self.barre_titre(&t, cx))
            .child(corps)
            // Le glisser d'un curseur suit le pointeur dans TOUTE la fenêtre, et finit au relâché.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if let Some(g) = &mut this.glisse
                    && ev.dragging()
                {
                    let x = f32::from(ev.position.x);
                    if let (Some(v), Some(r)) = (g.curseur.deplacer(x, g.gauche), &g.on_change) {
                        r(v);
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if let Some(mut g) = this.glisse.take() {
                        if let (Some(v), Some(r)) = (g.curseur.relacher(), &g.on_change) {
                            r(v);
                        }
                        cx.refresh_windows();
                    }
                }),
            );
        texte(racine, cx, t.typo.font_ui_medium, 1.5)
    }
}

// ── Petits outils ──────────────────────────────────────────────────────────

/// Une bordure sur le seul côté droit (`bordure` pose les quatre).
fn cote_droit<E: Styled>(mut e: E, largeur: Pixels) -> E {
    e.style().border_widths.right = Some(AbsoluteLength::from(largeur));
    e
}

/// Une bordure sur le seul côté bas.
fn cote_bas<E: Styled>(mut e: E, largeur: Pixels) -> E {
    e.style().border_widths.bottom = Some(AbsoluteLength::from(largeur));
    e
}

/// `#rrggbb` en jeton de couleur (noir si le texte est illisible).
pub fn couleur_depuis_hex(texte: &str) -> Couleur {
    let v = u32::from_str_radix(texte.trim_start_matches('#'), 16).unwrap_or(0);
    Couleur::hex(v)
}

/// Un jeton en `#rrggbb`.
pub fn hex_de(c: Couleur) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::repartir;

    #[test]
    fn la_place_en_trop_va_a_l_information() {
        assert_eq!(
            repartir(500.0, (100.0, 40.0, 20.0), (200.0, 150.0)),
            (280.0, 200.0)
        );
    }

    #[test]
    fn la_place_en_moins_se_retire_au_prorata_des_bases() {
        // 300 + 20 + 300 dans 520 : 100 de trop, 50 de chaque côté (bases égales).
        assert_eq!(
            repartir(520.0, (300.0, 40.0, 20.0), (300.0, 150.0)),
            (250.0, 250.0)
        );
    }

    #[test]
    fn un_element_bloque_a_son_min_content_rend_sa_part_a_l_autre() {
        // Les contrôles tomberaient à 100 sous leur min de 180 : ils s'y arrêtent.
        let (i, c) = repartir(400.0, (500.0, 40.0, 20.0), (125.0, 180.0));
        assert_eq!(c, 180.0);
        assert_eq!(i, 200.0);
    }
}
