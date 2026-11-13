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
    AbsoluteLength, AnyElement, App, Bounds, ClickEvent, Context, Div, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Window, anchored, canvas, deferred, div, px, svg,
};

use super::curseur::{Curseur, DIAMETRE_POUCE, HAUTEUR_PISTE, LARGEUR_PISTE};
use crate::echelle::{Echelle, calees, u};
use crate::flottant::{arrondir, arrondir_coins, bordure, texte};
use crate::icones;
use crate::polices::largeur_texte;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Les définitions d'un onglet, relues à chaque rendu (le thème courant en argument : l'accent du thème).
pub type Definitions = Rc<dyn Fn(&Theme) -> Vec<SettingDefinition>>;

/// Voir [`OngletReglages::apres_changement`].
pub type ApresChangement = Rc<dyn Fn(&mut App)>;

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
    /// Appelé avec le `cx` de la fenêtre après chaque réglage changé par un clic (liste, accent,
    /// bouton) : là où un effet de bord a besoin de la main (ex. le schéma de couleurs).
    pub apres_changement: Option<ApresChangement>,
}

impl OngletReglages {
    pub fn new(id: &str, nom: &str, icone: &str, section: SectionOnglet) -> Self {
        Self {
            id: id.to_owned(),
            nom: nom.to_owned(),
            icone: Some(icone.to_owned()),
            section,
            definitions: None,
            apres_changement: None,
        }
    }

    pub fn avec_apres_changement(mut self, f: ApresChangement) -> Self {
        self.apres_changement = Some(f);
        self
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
    /// La clé du contrôle dont la fenêtre flottante (liste, couleurs) est ouverte.
    ouverte: Option<String>,
}

/// Un bloc du contenu, mesuré : ses marges verticales (qui se FUSIONNENT entre voisins, comme en
/// CSS) et sa hauteur EXACTE (logique, fractionnaire comme celle de Chromium).
struct Mesure {
    haut: f32,
    bas: f32,
    hauteur: f32,
}

/// Un bloc du contenu, posé : son arête haute exacte, l'écart calé qui le précède et sa hauteur calée.
struct Pose {
    haut: f32,
    ecart: Pixels,
    hauteur: Pixels,
}

/// La géométrie d'une carte (px logiques exacts) : voir [`FenetreReglages::geometrie`].
struct GeoCarte {
    info_l: f32,
    ctl_l: f32,
    /// Hauteur de l'information (nom et description).
    hi: f32,
    /// Hauteur du bloc des contrôles (rangées et écarts).
    hc: f32,
    /// Par rangée de contrôles, la hauteur de chacun.
    rangees: Vec<Vec<f32>>,
    hauteurs_rangees: Vec<f32>,
    /// La carte entière, rembourrage compris.
    hauteur: f32,
}

/// Hauteur de `.settings-titlebar` (`height: 32px`), SANS la bordure basse : ★ la boîte est en
/// `content-box` (le défaut de Fragment), la bordure s'ajoute : 33 px en tout. Comptée dedans, tout
/// le rail et le contenu montaient d'un pixel.
const TITRE_HAUTEUR: f32 = 32.0;
/// Le nom de l'arc déclaré de la pastille d'accent : exception nommée d'ADR-009 (en cliquet sous
/// `fr-p4.1`, `xtask/src/pixels/exceptions.rs`), dont il est la clé de cliquet.
const ELEMENT_PASTILLE: &str = "arc de la pastille d'accent";
/// Largeur du contenu du rail (`.vertical-tab-header { width }`, hors rembourrage).
const RAIL_LARGEUR: f32 = 220.0;
/// `.vertical-tab-content { max-width }`.
const CONTENU_LARGEUR: f32 = 760.0;
/// La piste d'une bascule (`.checkbox-container { width: 42px }`).
const LARGEUR_BASCULE: f32 = 42.0;
/// `.search-input-container::before { left: 9px }` : la loupe.
const LOUPE_GAUCHE: f32 = 9.0;
/// `.vertical-tab-header-group-title { letter-spacing: 0.04em }`.
const ESPACEMENT_TITRE_GROUPE: f32 = 0.04;
/// `.checkbox-container { height: 24px }`.
const HAUTEUR_BASCULE: f32 = 24.0;
/// `.setting-item-heading .setting-item-name { font-size: 16px }`.
const TAILLE_ENTETE: f32 = 16.0;
/// `.setting-item-description { line-height: 1.4 }`.
const INTERLIGNE_DESCRIPTION: f32 = 1.4;
/// `select.dropdown { padding: 5px … }` (haut et bas, un littéral du CSS).
const REMBOURRAGE_LISTE: f32 = 5.0;
/// `input[type=range] { margin: 2px }` : feuille de style de l'agent utilisateur de Chromium.
const MARGE_PISTE: f32 = 2.0;

/// Hauteur exacte d'un en-tête de groupe : rembourrage (10, 4) et une ligne de 16 × 1,5.
fn hauteur_entete(t: &Theme) -> f32 {
    t.forme.size_2_5 + TAILLE_ENTETE * 1.5 + t.forme.size_4_1
}

/// Hauteur exacte d'une liste déroulante fermée : rembourrage, bordures, une ligne de 13 × 1,5.
/// ★ 31,5 px, pas 32 : Chromium la peint sur 32 ou 31 lignes selon la fraction de son arête.
fn hauteur_liste(t: &Theme) -> f32 {
    2.0 * REMBOURRAGE_LISTE + 2.0 * t.forme.border_width + t.typo.font_ui_medium * 1.5
}

/// Hauteur exacte de la rangée d'un curseur : l'étiquette (13 × 1,5) ou l'`input` et ses marges.
fn hauteur_curseur(etiquette: bool, t: &Theme) -> f32 {
    let input = HAUTEUR_PISTE + 2.0 * MARGE_PISTE;
    if etiquette {
        input.max(t.typo.font_ui_medium * 1.5)
    } else {
        input
    }
}

/// Hauteur exacte d'un contrôle rendu.
fn hauteur_controle(c: &SettingControl, t: &Theme) -> f32 {
    match c {
        SettingControl::Slider(s) => hauteur_curseur(s.format.is_some(), t),
        SettingControl::Dropdown(_) => hauteur_liste(t),
        SettingControl::Toggle(_) => HAUTEUR_BASCULE,
        _ => t.forme.size_4_7,
    }
}

/// Largeur (logique) d'une liste déroulante fermée. ★ Chromium arrondit la plus longue option au
/// pixel SUPÉRIEUR (`LayoutMenuList`, largeur entière) avant d'ajouter rembourrage et bordures :
/// « Adapt to system » (99,26 px) donne 100 + 38 = 138, l'arrondi au plus proche donnait 137.
fn largeur_liste(d: &SettingDropdownControl, t: &Theme, cx: &App) -> f32 {
    let (f, ty) = (&t.forme, &t.typo);
    let zoom = Echelle::courante(cx).zoom();
    let texte_max = d
        .options
        .iter()
        .map(|(_, l)| largeur_texte(cx, l, FontWeight::NORMAL, ty.font_ui_medium))
        .fold(0.0_f32, f32::max);
    (texte_max * zoom).ceil() / zoom + f.size_4_2 + f.size_4_7 + 2.0 * f.border_width
}

/// Une longueur ramenée à l'unité de Chromium (`LayoutUnit`, 1/64 px, tronquée) : 16,8 devient
/// 16,796875. ★ La différence compte : une arête à x,5 tout juste s'arrondit vers le haut, à x,49 vers le bas.
fn unite_chromium(cx: &App, v: f32) -> f32 {
    let zoom = Echelle::courante(cx).zoom();
    (v * zoom * 64.0).floor() / 64.0 / zoom
}

/// La distance peinte entre deux arêtes exactes (logiques) : `arrondi(b) − arrondi(a)`.
fn ecart_cale(cx: &App, a: f32, b: f32) -> Pixels {
    calees(cx, a, &[b - a]).first().copied().unwrap_or_default()
}

/// Le nombre de lignes d'un texte replié dans `largeur` (mots entiers, comme Chromium).
fn lignes(cx: &App, texte: &str, taille: f32, largeur: f32) -> usize {
    let mut n = 0;
    let mut ligne = String::new();
    for mot in texte.split_whitespace() {
        let essai = if ligne.is_empty() {
            mot.to_owned()
        } else {
            format!("{ligne} {mot}")
        };
        if !ligne.is_empty() && largeur_texte(cx, &essai, FontWeight::NORMAL, taille) > largeur {
            n += 1;
            ligne = mot.to_owned();
        } else {
            ligne = essai;
        }
    }
    (n + usize::from(!ligne.is_empty())).max(1)
}

/// ★ `letter-spacing`, que GPUI n'a pas : une boîte par lettre, chacune à l'arrondi de l'abscisse de
/// Chromium (avance crénée du préfixe + `i` × espacement, l'espacement suivant AUSSI la dernière
/// lettre). Chromium pose les glyphes au pixel entier sous Windows (pas de positionnement
/// sous-pixel) : les lettres retombent donc où il les peint. Sans cela « OPTIONS » finissait 3 px
/// trop tôt (0,48 px × 7).
fn espacer_lettres(cx: &App, texte: &str, taille: f32, espacement: f32) -> AnyElement {
    let mut bords = vec![0.0_f32];
    for (i, (k, ch)) in texte.char_indices().enumerate() {
        let fin = k + ch.len_utf8();
        bords.push(
            largeur_texte(cx, &texte[..fin], FontWeight::NORMAL, taille)
                + (i + 1) as f32 * espacement,
        );
    }
    let mut largeurs = Vec::with_capacity(bords.len());
    for w in bords.windows(2) {
        largeurs.push(w[1] - w[0]);
    }
    let calees_ = calees(cx, 0.0, &largeurs);
    let mut ligne = div().flex().flex_none().whitespace_nowrap();
    for (ch, l) in texte.chars().zip(calees_) {
        ligne = ligne.child(div().flex_none().w(l).child(ch.to_string()));
    }
    ligne.into_any_element()
}

/// Pose un bloc : l'écart calé qui le précède, puis le bloc à sa hauteur calée.
fn poser(p: &Pose, el: AnyElement, sortie: &mut Vec<AnyElement>) {
    sortie.push(div().h(p.ecart).into_any_element());
    sortie.push(div().h(p.hauteur).child(el).into_any_element());
}

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
            ouverte: None,
        }
    }

    /// Active l'onglet `id` (sans effet s'il n'existe pas).
    pub fn activer(&mut self, id: &str) {
        if let Some(i) = self.onglets.iter().position(|o| o.id == id) {
            self.actif = i;
            self.ouverte = None;
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
                    .child(espacer_lettres(
                        cx,
                        &section.titre().to_uppercase(),
                        ty.font_ui_small,
                        ESPACEMENT_TITRE_GROUPE * ty.font_ui_small,
                    )),
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
                    // ★ Coins DÉCLARÉS (bord d'arrondi d'ADR-009) : en clair, l'anticrénelage de
                    //   Chromium laisse un niveau de bleu aux bouts des arcs (213 contre 212).
                    arrondir(item, cx, u(cx, f.radius_sm), u(cx, 0.0))
                        .bg(couleur(c.aplat(Aplat::SurvolActif, c.background_secondary)))
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
                        this.ouverte = None;
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
            .bg(couleur(c.background_secondary))
            .text_color(couleur(c.text_muted))
            .child(
                // ★ `left: 9px` se compte depuis `.search-input-container`, qui n'a pas de bordure ;
                // ici la loupe est dans le champ bordé, et un enfant absolu se pose depuis la boîte de
                // rembourrage : la bordure est retirée (sinon +0,94 px en x).
                div()
                    .absolute()
                    .left(u(cx, LOUPE_GAUCHE - f.border_width))
                    .child(
                        svg()
                            .path(icones::chemin("search"))
                            .size(u(cx, f.icon_xs))
                            .text_color(couleur(c.text_muted)),
                    ),
            )
            .child(
                // Le texte indicatif : couleur `::placeholder` de Chromium (vérifié : aucun
                // `::placeholder` des CSS de Fragment ne vise ce champ).
                div()
                    .text_color(couleur(fragment_theme::theme::INDICATIF_CHROMIUM))
                    .child("Search settings..."),
            );
        // L'arrondi est DÉCLARÉ (catégorie bord d'arrondi d'ADR-009) : ses coins ne s'anticrènelent
        // pas comme ceux de Skia, à un niveau près.
        let champ = arrondir(champ, cx, u(cx, f.radius_sm), u(cx, f.border_width));
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
        // ★ Deux passes. Chromium empile les blocs en positions FRACTIONNAIRES (une description de
        //   12 × 1,4 = 16,8 px fait des cartes de 67,8 px) et n'arrondit qu'à la peinture, arête par
        //   arête ; GPUI arrondissait chaque carte (68 px) et la troisième finissait un pixel trop bas,
        //   tout ce qui suit avec. On mesure donc d'abord les hauteurs exactes, puis on pose chaque bloc
        //   à sa hauteur calée (`echelle::calees`) en lui donnant son arête haute exacte.
        let mut mesures: Vec<Mesure> = Vec::new();
        for (i, d) in defs.iter().enumerate() {
            self.mesurer(d, i == 0, &mut mesures, t, cx);
        }
        // Les marges de blocs voisins se fusionnent (la plus grande gagne) ; la première et la
        // dernière restent dans le rembourrage.
        let origine = TITRE_HAUTEUR + f.border_width + f.size_4_5;
        let mut segments = Vec::with_capacity(2 * mesures.len());
        let mut hauts = Vec::with_capacity(mesures.len());
        let mut exact = origine;
        let mut precedent: Option<f32> = None;
        for m in &mesures {
            let ecart = precedent.map_or(0.0, |bas| bas.max(m.haut));
            exact += ecart;
            hauts.push(exact);
            exact += m.hauteur;
            segments.push(ecart);
            segments.push(m.hauteur);
            precedent = Some(m.bas);
        }
        let calees_ = calees(cx, origine, &segments);
        let mut poses = hauts
            .into_iter()
            .enumerate()
            .map(|(k, haut)| Pose {
                haut,
                ecart: calees_.get(2 * k).copied().unwrap_or_default(),
                hauteur: calees_.get(2 * k + 1).copied().unwrap_or_default(),
            })
            .collect::<Vec<_>>()
            .into_iter();
        let mut colonne = div().flex().flex_col();
        for (i, d) in defs.iter().enumerate() {
            for el in self.definition(d, &i.to_string(), &mut poses, t, cx) {
                colonne = colonne.child(el);
            }
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

    /// Première passe : les marges et la hauteur EXACTE des blocs d'une définition, dans l'ordre.
    fn mesurer(
        &self,
        d: &SettingDefinition,
        premier: bool,
        mesures: &mut Vec<Mesure>,
        t: &Theme,
        cx: &App,
    ) {
        let f = &t.forme;
        match d {
            SettingDefinition::Control(SettingDefinitionControl {
                name,
                description,
                control,
                ..
            }) => {
                let g = self.geometrie(name, description.as_deref(), &[control], t, cx);
                mesures.push(Mesure {
                    haut: 0.0,
                    bas: f.size_2_5,
                    hauteur: g.hauteur,
                });
            }
            SettingDefinition::Item(SettingDefinitionItem {
                name,
                description,
                controls,
                ..
            }) => {
                let refs: Vec<&SettingControl> = controls.iter().collect();
                let g = self.geometrie(name, description.as_deref(), &refs, t, cx);
                mesures.push(Mesure {
                    haut: 0.0,
                    bas: f.size_2_5,
                    hauteur: g.hauteur,
                });
            }
            SettingDefinition::Group(g) => {
                mesures.push(Mesure {
                    haut: if premier { 0.0 } else { f.size_4_2 },
                    bas: f.size_4_1,
                    hauteur: hauteur_entete(t),
                });
                for sous in &g.items {
                    self.mesurer(sous, false, mesures, t, cx);
                }
            }
            // Pages, listes, arbres, actions, rendus libres : avec les onglets qui les emploient.
            _ => {}
        }
    }

    /// Seconde passe : les blocs d'une définition (écart calé puis bloc), dans l'ordre de [`Self::mesurer`].
    fn definition(
        &self,
        d: &SettingDefinition,
        chemin: &str,
        poses: &mut std::vec::IntoIter<Pose>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut sortie = Vec::new();
        match d {
            SettingDefinition::Control(SettingDefinitionControl {
                name,
                description,
                control,
                ..
            }) => {
                let Some(p) = poses.next() else {
                    return sortie;
                };
                let ctl = vec![(format!("{chemin}-0"), control)];
                let carte = self.carte(name, description.as_deref(), &ctl, &p, t, cx);
                poser(&p, carte, &mut sortie);
            }
            SettingDefinition::Item(SettingDefinitionItem {
                name,
                description,
                controls,
                ..
            }) => {
                let Some(p) = poses.next() else {
                    return sortie;
                };
                let ctl: Vec<(String, &SettingControl)> = controls
                    .iter()
                    .enumerate()
                    .map(|(k, c)| (format!("{chemin}-{k}"), c))
                    .collect();
                let carte = self.carte(name, description.as_deref(), &ctl, &p, t, cx);
                poser(&p, carte, &mut sortie);
            }
            SettingDefinition::Group(g) => {
                let Some(p) = poses.next() else {
                    return sortie;
                };
                poser(&p, self.entete(&g.name, t, cx), &mut sortie);
                for (i, sous) in g.items.iter().enumerate() {
                    sortie.extend(self.definition(sous, &format!("{chemin}.{i}"), poses, t, cx));
                }
            }
            _ => {}
        }
        sortie
    }

    /// `.setting-item-heading` : un titre, sans carte (marges dans [`Self::mesurer`]).
    fn entete(&self, nom: &str, t: &Theme, cx: &Context<Self>) -> AnyElement {
        let (f, c) = (&t.forme, &t.palette);
        let el = div()
            .pt(u(cx, f.size_2_5))
            .px(u(cx, f.size_2_1))
            .text_color(couleur(c.text_normal))
            .child(nom.to_owned());
        texte(el, cx, TAILLE_ENTETE, 1.5)
            .font_weight(FontWeight::SEMIBOLD)
            .into_any_element()
    }

    /// La géométrie d'une carte, calculée comme Chromium : largeurs de la ligne (flexbox §9.7),
    /// lignes de texte, rangées de contrôles et hauteurs EXACTES.
    fn geometrie(
        &self,
        nom: &str,
        desc: Option<&str>,
        controles: &[&SettingControl],
        t: &Theme,
        cx: &App,
    ) -> GeoCarte {
        let (f, ty) = (&t.forme, &t.typo);
        // ★ La répartition de la ligne, calculée comme Chromium (flexbox §9.7) au lieu d'être laissée à
        //   taffy : le texte de GPUI n'a pas de largeur min-content (le mot le plus long), la
        //   description refusait donc de se replier et poussait les contrôles.
        let texte_de = |t: &str, taille: f32| largeur_texte(cx, t, FontWeight::NORMAL, taille);
        let mots = |t: &str, taille: f32| {
            t.split_whitespace()
                .map(|m| texte_de(m, taille))
                .fold(0.0_f32, f32::max)
        };
        let desc = desc.filter(|d| !d.is_empty());
        let info_base = desc
            .map_or(0.0, |d| texte_de(d, ty.font_ui_small))
            .max(texte_de(nom, ty.font_ui_normal));
        let info_min = desc
            .map_or(0.0, |d| mots(d, ty.font_ui_small))
            .max(mots(nom, ty.font_ui_normal));
        let mesures: Vec<(f32, f32)> = controles
            .iter()
            .filter_map(|c| Self::largeur_controle(c, t, cx).map(|l| (l, hauteur_controle(c, t))))
            .collect();
        let n = mesures.len();
        let ctl_base =
            mesures.iter().map(|m| m.0).sum::<f32>() + f.size_4_2 * n.saturating_sub(1) as f32;
        let ctl_min = mesures.iter().map(|m| m.0).fold(0.0_f32, f32::max);
        let colonne = (self.largeur - largeur_rail(t)).min(CONTENU_LARGEUR + 2.0 * f.size_4_6)
            - 2.0 * f.size_4_6;
        let interieur = colonne - 2.0 * f.size_4_4;
        let (info_l, ctl_l) = repartir(
            interieur - if n > 0 { f.size_4_4 } else { 0.0 },
            (info_base, info_min, f.size_4_5),
            (ctl_base, ctl_min),
        );
        // L'information : le nom (14 × 1,5) puis la description (marge 2, lignes de 12 × 1,4).
        let ligne_nom = ty.font_ui_normal * 1.5;
        let ligne_desc = unite_chromium(cx, ty.font_ui_small * INTERLIGNE_DESCRIPTION);
        let mut hi = lignes(cx, nom, ty.font_ui_normal, info_l) as f32 * ligne_nom;
        if let Some(d) = desc {
            hi += f.size_2_1 + lignes(cx, d, ty.font_ui_small, info_l) as f32 * ligne_desc;
        }
        // Les contrôles : `flex-wrap`, une rangée tant que la largeur tient.
        let mut rangees: Vec<Vec<(f32, f32)>> = Vec::new();
        let mut largeur_rangee = 0.0;
        for m in mesures {
            match rangees.last_mut() {
                Some(r) if largeur_rangee + f.size_4_2 + m.0 <= ctl_l + 1e-3 => {
                    largeur_rangee += f.size_4_2 + m.0;
                    r.push(m);
                }
                _ => {
                    largeur_rangee = m.0;
                    rangees.push(vec![m]);
                }
            }
        }
        let hauteurs_rangees: Vec<f32> = rangees
            .iter()
            .map(|r| r.iter().map(|m| m.1).fold(0.0_f32, f32::max))
            .collect();
        let hc = hauteurs_rangees.iter().sum::<f32>()
            + f.size_4_2 * hauteurs_rangees.len().saturating_sub(1) as f32;
        let contenu = hi.max(hc);
        GeoCarte {
            info_l,
            ctl_l,
            hi,
            hc,
            rangees: rangees
                .iter()
                .map(|r| r.iter().map(|m| m.1).collect())
                .collect(),
            hauteurs_rangees,
            hauteur: 2.0 * f.size_2_7 + contenu,
        }
    }

    /// `.setting-item` : une carte, infos à gauche, contrôles à droite, posée à son arête exacte.
    fn carte(
        &self,
        nom: &str,
        desc: Option<&str>,
        controles: &[(String, &SettingControl)],
        pose: &Pose,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (f, ty, c) = (&t.forme, &t.typo, &t.palette);
        let rendus: Vec<&(String, &SettingControl)> = controles
            .iter()
            .filter(|(_, c)| Self::largeur_controle(c, t, cx).is_some())
            .collect();
        let refs: Vec<&SettingControl> = rendus.iter().map(|(_, c)| *c).collect();
        let g = self.geometrie(nom, desc, &refs, t, cx);
        // Chromium centre l'information et les contrôles dans la boîte de contenu (`align-items:
        // center`), en fractions ; chacun se peint à l'arrondi de son arête haute.
        let haut = pose.haut;
        let contenu = haut + f.size_2_7;
        let plein = g.hi.max(g.hc);
        let y_info = contenu + (plein - g.hi) / 2.0;
        let y_ctl = contenu + (plein - g.hc) / 2.0;

        let mut info = div()
            .flex_none()
            .w(u(cx, g.info_l))
            .mt(ecart_cale(cx, haut, y_info))
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
            // ★ Ligne de 16,8 px (12 × 1,4). Chromium arrondit l'ascendante et la descendante de DM Sans
            // séparément (11,9 → 12, 3,7 → 4) et tronque le demi-interligne ((16,8 − 16) / 2 → 0) :
            // ligne de base à 12 px du haut de la ligne. GPUI garde les fractions (0,57 + 11,9 = 12,5) et
            // pose les glyphes à 13 : un pixel trop bas (barycentre +0,98 px en y, run 43039933024). La
            // marge haute perd ce pixel ; la carte a sa hauteur calée, la mise en page ne bouge pas.
            // ★ GPUI arrondit aussi l'interligne (16,8 → 17) : une 2e ligne retombe sur celle de Chromium
            // tant que l'arête exacte de la description n'est pas trop près d'un demi-pixel (vrai ici).
            info = info.child(texte(
                div()
                    .mt(u(cx, f.size_2_1) - px(1.0))
                    .text_color(couleur(c.text_muted))
                    .child(d.to_owned()),
                cx,
                ty.font_ui_small,
                INTERLIGNE_DESCRIPTION,
            ));
        }

        // Les rangées de contrôles, calées depuis l'arête exacte du bloc des contrôles.
        let mut segments = Vec::new();
        for (k, h) in g.hauteurs_rangees.iter().enumerate() {
            if k > 0 {
                segments.push(f.size_4_2);
            }
            segments.push(*h);
        }
        let hauteurs = calees(cx, y_ctl, &segments);
        let mut bloc = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(u(cx, g.ctl_l))
            .mt(ecart_cale(cx, haut, y_ctl));
        let mut suivants = rendus.iter();
        let mut y_rangee = y_ctl;
        for (k, (rangee, h)) in g.rangees.iter().zip(&g.hauteurs_rangees).enumerate() {
            if k > 0 {
                bloc = bloc.child(div().h(hauteurs.get(2 * k - 1).copied().unwrap_or_default()));
                y_rangee += f.size_4_2;
            }
            let mut ligne = div()
                .flex()
                .justify_end()
                .items_start()
                .gap(u(cx, f.size_4_2))
                .h(hauteurs.get(2 * k).copied().unwrap_or_default());
            for hauteur_ctl in rangee {
                let Some((cle, controle)) = suivants.next() else {
                    break;
                };
                let y = y_rangee + (h - hauteur_ctl) / 2.0;
                if let Some(el) = self.controle(cle, controle, y, t, cx) {
                    ligne =
                        ligne.child(div().flex_none().mt(ecart_cale(cx, y_rangee, y)).child(el));
                }
            }
            bloc = bloc.child(ligne);
            y_rangee += h;
        }
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap(u(cx, f.size_4_4))
            .h_full()
            .px(u(cx, f.size_4_4))
            .rounded(u(cx, f.radius_ml))
            .bg(couleur(c.workspace_bg))
            .child(info)
            .child(bloc)
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
                Some(etiquette.map_or(0.0, |e| e + f.size_4_2) + LARGEUR_PISTE + 2.0 * MARGE_PISTE)
            }
            SettingControl::Dropdown(d) => Some(largeur_liste(d, t, cx)),
            SettingControl::Color(_) | SettingControl::ExtraButton(_) => Some(f.size_4_7),
            SettingControl::Toggle(_) => Some(LARGEUR_BASCULE),
            _ => None,
        }
    }

    /// Un contrôle, dont l'arête haute exacte (logique) est `y`.
    fn controle(
        &self,
        cle: &str,
        c: &SettingControl,
        y: f32,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (f, p) = (&t.forme, &t.palette);
        match c {
            SettingControl::Slider(s) => Some(self.curseur(cle, s, y, t, cx)),
            SettingControl::Dropdown(d) => Some(self.liste(cle, d, y, t, cx)),
            SettingControl::Color(k) => {
                let ouvert = self.ouverte.as_deref() == Some(cle);
                let cle_s = cle.to_owned();
                let pastille = div()
                    .id(SharedString::from(format!("couleur-{cle}")))
                    .size(u(cx, f.size_4_7))
                    .flex_none()
                    .overflow_hidden()
                    .cursor_pointer()
                    .bg(couleur(couleur_depuis_hex(&k.value)));
                // ★ Ronde = rayon de la demi-taille, DÉCLARÉ pour le sas (fr-p3.1, bord d'arrondi),
                // sous le nom de son exception (anneau de bordure de 1 px, mélange alpha).
                let pastille = arrondir_coins(
                    pastille,
                    cx,
                    u(cx, f.size_4_7 / 2.0),
                    u(cx, f.border_width),
                    [true; 4],
                    ELEMENT_PASTILLE,
                );
                let pastille = bordure(pastille, u(cx, f.border_width))
                    .border_color(couleur(p.tab_outline_color))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.basculer(&cle_s);
                        cx.notify();
                    }));
                if !ouvert {
                    return Some(pastille.into_any_element());
                }
                // ★ Pas de sélecteur natif en GPUI : les huit couleurs nommées du thème en tiennent
                //   lieu (l'accent reste libre dans `appearance.json`, un hex quelconque).
                let hook = self.hook();
                let mut rangee = div().flex().gap(u(cx, f.size_4_2)).p(u(cx, f.size_4_2));
                for (i, c) in [
                    p.color_red,
                    p.color_orange,
                    p.color_yellow,
                    p.color_green,
                    p.color_cyan,
                    p.color_blue,
                    p.color_purple,
                    p.color_pink,
                ]
                .into_iter()
                .enumerate()
                {
                    let (hex, rappel, hook) = (hex_de(c), k.on_change.clone(), hook.clone());
                    let pastille_choix = div()
                        .id(SharedString::from(format!("choix-{cle}-{i}")))
                        .size(u(cx, f.size_4_7))
                        .flex_none()
                        .rounded_full()
                        .cursor_pointer()
                        .bg(couleur(c))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(r) = &rappel {
                                r(hex.clone());
                            }
                            this.ouverte = None;
                            if let Some(h) = &hook {
                                h(cx);
                            }
                            cx.refresh_windows();
                        }));
                    rangee = rangee.child(pastille_choix);
                }
                Some(Self::avec_fenetre(
                    pastille.into_any_element(),
                    rangee,
                    u(cx, f.size_4_7),
                    t,
                    cx,
                ))
            }
            SettingControl::ExtraButton(b) => {
                let action = b.on_click.clone();
                let hook = self.hook();
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
                        if let Some(h) = &hook {
                            h(cx);
                        }
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
                    .h(u(cx, HAUTEUR_BASCULE))
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

    /// `select.dropdown`, fermé, d'arête haute exacte `y` : la largeur est celle de la plus longue option.
    /// Le rappel d'après changement de l'onglet actif.
    fn hook(&self) -> Option<ApresChangement> {
        self.onglets
            .get(self.actif)
            .and_then(|o| o.apres_changement.clone())
    }

    /// Ouvre la fenêtre flottante du contrôle `cle`, ou la ferme si elle l'est déjà.
    fn basculer(&mut self, cle: &str) {
        self.ouverte = if self.ouverte.as_deref() == Some(cle) {
            None
        } else {
            Some(cle.to_owned())
        };
    }

    /// Le contrôle `base`, avec sa fenêtre flottante `contenu` posée sous lui (à `decalage`).
    fn avec_fenetre(
        base: AnyElement,
        contenu: Div,
        decalage: Pixels,
        t: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (f, p) = (&t.forme, &t.palette);
        let boite = contenu
            .occlude()
            .bg(couleur(p.background_secondary))
            .text_color(couleur(p.text_normal));
        let boite = arrondir(boite, cx, u(cx, f.radius_sm), u(cx, f.border_width));
        let boite =
            bordure(boite, u(cx, f.border_width)).border_color(couleur(p.tab_outline_color));
        div()
            .relative()
            .child(base)
            .child(deferred(anchored().child(div().pt(decalage).child(boite))))
            .into_any_element()
    }

    fn liste(
        &self,
        cle: &str,
        d: &SettingDropdownControl,
        y: f32,
        t: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (f, ty, p) = (&t.forme, &t.typo, &t.palette);
        let actuel = d
            .options
            .iter()
            .find(|(v, _)| *v == d.value)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let hauteur = hauteur_liste(t);
        let survol = couleur(p.background_modifier_hover);
        // Le texte (ligne de 13 × 1,5) au haut de la boîte de contenu ; la flèche, image de fond de
        // 16 px centrée dans la boîte de rembourrage (`background-position: right 6px center`). Les
        // enfants absolus se posent depuis la boîte de rembourrage : la bordure est retirée.
        let bord = f.border_width;
        let haut_texte = ecart_cale(cx, y, y + bord + REMBOURRAGE_LISTE) - u(cx, bord);
        let haut_fleche =
            ecart_cale(cx, y, y + bord + (hauteur - 2.0 * bord - f.icon_s) / 2.0) - u(cx, bord);
        let zoom = Echelle::courante(cx).zoom();
        let boite = div()
            .relative()
            .flex_none()
            .w(px((largeur_liste(d, t, cx) * zoom).round()))
            .h(ecart_cale(cx, y, y + hauteur))
            .bg(couleur(p.background_secondary))
            .text_color(couleur(p.text_normal))
            .child(
                div()
                    .absolute()
                    .top(haut_texte)
                    .left(u(cx, f.size_4_2))
                    .whitespace_nowrap()
                    .child(actuel),
            )
            .child(
                div()
                    .absolute()
                    .top(haut_fleche)
                    .right(u(cx, f.size_2_3))
                    .child(
                        svg()
                            .path(icones::chemin("chevrons-up-down"))
                            .size(u(cx, f.icon_s))
                            // ★ Le CSS écrit ici un gris littéral (#8f8e89, dans une URL de données),
                            // sans variable : jeton fixe `FLECHE_LISTE`, le même dans les deux thèmes.
                            .text_color(couleur(fragment_theme::theme::FLECHE_LISTE)),
                    ),
            );
        // Arrondi déclaré, comme celui du champ de recherche.
        let boite = arrondir(boite, cx, u(cx, f.radius_sm), u(cx, bord));
        let boite = bordure(boite, u(cx, bord)).border_color(couleur(p.tab_outline_color));
        let cle_s = cle.to_owned();
        let boite = texte(boite, cx, ty.font_ui_medium, 1.5)
            .id(SharedString::from(format!("liste-{cle}")))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.basculer(&cle_s);
                cx.notify();
            }));
        if self.ouverte.as_deref() != Some(cle) {
            return boite.into_any_element();
        }
        let hook = self.hook();
        let mut options = div()
            .flex()
            .flex_col()
            .min_w(px((largeur_liste(d, t, cx) * zoom).round()));
        for (i, (valeur, libelle)) in d.options.iter().enumerate() {
            let (valeur, rappel, hook) = (valeur.clone(), d.on_change.clone(), hook.clone());
            options = options.child(
                div()
                    .id(SharedString::from(format!("option-{cle}-{i}")))
                    .px(u(cx, f.size_4_2))
                    .py(u(cx, REMBOURRAGE_LISTE))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .hover(move |s| s.bg(survol))
                    .child(libelle.clone())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(r) = &rappel {
                            r(valeur.clone());
                        }
                        this.ouverte = None;
                        if let Some(h) = &hook {
                            h(cx);
                        }
                        cx.refresh_windows();
                    })),
            );
        }
        Self::avec_fenetre(boite.into_any_element(), options, u(cx, hauteur), t, cx)
    }

    /// `input.slider` + l'étiquette de valeur (`.slider-value`), d'arête haute exacte `y`.
    fn curseur(
        &self,
        cle: &str,
        s: &SettingSliderControl,
        y: f32,
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
        // La rangée : l'étiquette (13 × 1,5) et l'`input` (4 px + ses marges de 2), centrés. Le pouce
        // (16 px) est centré sur la piste et déborde ; chacun se peint à l'arrondi de son arête.
        let rangee = hauteur_curseur(s.format.is_some(), t);
        let y_piste = y + (rangee - HAUTEUR_PISTE - 2.0 * MARGE_PISTE) / 2.0 + MARGE_PISTE;
        let y_pouce = y_piste + (HAUTEUR_PISTE - DIAMETRE_POUCE) / 2.0;

        let bornes = self.bornes.clone();
        let (cle_canvas, cle_appui) = (cle.to_owned(), cle.to_owned());
        let on_change = s.on_change.clone();

        let piste = div()
            .id(SharedString::from(format!("curseur-{cle}")))
            .relative()
            .flex_none()
            .w(u(cx, LARGEUR_PISTE))
            .h(u(cx, DIAMETRE_POUCE))
            // ★ `margin: 2px` : la feuille de style de Chromium (agent utilisateur) pour
            // `input[type=range]`, que settings.css ne remet pas à zéro. Sans elle, la piste était 2 px
            // trop à droite, l'étiquette 4 px, et le glisser de la scène tombait sur 145 % au lieu de 150.
            .mx(u(cx, MARGE_PISTE))
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
                    .absolute()
                    .top(ecart_cale(cx, y, y_piste))
                    .left(u(cx, 0.0))
                    .w_full()
                    .h(u(cx, HAUTEUR_PISTE))
                    .rounded(u(cx, f.radius_xxs))
                    .bg(couleur(p.tab_outline_color)),
            )
            // Le pouce : un disque, arrondi DÉCLARÉ (bord d'arrondi d'ADR-009).
            .child(arrondir(
                div()
                    .absolute()
                    .top(ecart_cale(cx, y, y_pouce))
                    .left(u(cx, centre - DIAMETRE_POUCE / 2.0))
                    .size(u(cx, DIAMETRE_POUCE))
                    .bg(couleur(p.color_accent)),
                cx,
                u(cx, DIAMETRE_POUCE / 2.0),
                u(cx, 0.0),
            ))
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
            .items_start()
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
        // La fenêtre de réglages est une surface opaque (ClearType) ; ses listes déroulantes sont
        // des dessins différés, donc des calques gris (ADR-017).
        crate::rendu::mode_texte::surface_opaque(texte(racine, cx, t.typo.font_ui_medium, 1.5))
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
    use super::{hauteur_curseur, hauteur_entete, hauteur_liste, repartir};
    use fragment_theme::Theme;

    #[test]
    fn les_hauteurs_exactes_sont_celles_du_css() {
        let t = Theme::sombre();
        // 5 + 5 + 1 + 1 + 13 × 1,5 : la liste fait 31,5 px (peinte sur 31 ou 32 lignes).
        assert_eq!(hauteur_liste(&t), 31.5);
        // 10 + 16 × 1,5 + 4.
        assert_eq!(hauteur_entete(&t), 38.0);
        // L'étiquette (19,5) l'emporte sur l'`input` (4 + 2 × 2).
        assert_eq!(hauteur_curseur(true, &t), 19.5);
        assert_eq!(hauteur_curseur(false, &t), 8.0);
    }

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
