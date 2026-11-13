// Port de app/src/ui/views/chat/scroll/rail.ts (`ChatRail` : tirets, aperçu au survol, clic, clavier)
// et de scroll.css (`.chat-rail*`, `.chat-rail-preview*`, variables `--chat-rail-*`)
//! Le RAIL de la conversation : une colonne de tirets dans la marge GAUCHE du fil, un par TOUR
//! (une question), à partir de [`RAIL_MIN_TURNS`] tours. Le tiret du tour au centre de l'écran est
//! le plus long et le plus clair (la proximité de `fragment_chat::rail`), le survolé s'allonge et
//! blanchit ; au survol une carte montre la question, le début de la réponse et le premier fichier ;
//! un clic place le tour en haut ([`AuClic`]).
//!
//! ★ Un seul élément racine, absolu, qui couvre le fil et ne capte rien : seuls les tirets ont une
//!   zone de clic. Le rail est centré verticalement ; la carte est posée sur un point d'ancrage de
//!   hauteur nulle, à l'ordonnée du tiret, et se centre dessus (GPUI n'a pas de `translateY`).
//! ★ Les tirets ne sont pas des boutons tabulables un à un : le rail entier prend le focus et ↑ / ↓
//!   y déplacent le tiret courant, Entrée le déclenche. Alt+↑ / Alt+↓ (tour voisin) sont au
//!   défileur.

use std::collections::HashMap;
use std::rc::Rc;

use fragment_chat::rail::{RAIL_MIN_TURNS, RailQuestion, RailTurn, rail_proximity};
use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, Context, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, relative, svg,
};

use crate::chat_defilement::Mesures;
use crate::echelle::u;
use crate::flottant::{ombrer, texte};
use crate::icones;
use crate::theme_global::{couleur, theme_ou_defaut};

/// `--chat-rail-tick-w` : la longueur d'un tiret au repos (px).
pub const TIRET_LARGEUR: f32 = 12.0;
/// `--chat-rail-tick-w-hover` : la longueur du tiret survolé.
pub const TIRET_LARGEUR_SURVOL: f32 = 20.0;
/// `--chat-rail-wave` : ce que la proximité ajoute à la longueur.
pub const VAGUE: f32 = 4.0;
/// `--chat-rail-preview-w` : la largeur de la carte d'aperçu.
pub const APERCU_LARGEUR: f32 = 300.0;
/// L'écart entre le rail et la carte (`right + 8`).
pub const APERCU_ECART: f32 = 8.0;

/// Le clic sur un tiret : l'id de la question du tour.
pub type AuClic = Rc<dyn Fn(&str, &mut App)>;

/// La longueur d'un tiret : `base + p × vague` ; survolé ou focalisé, la longueur de survol.
#[must_use]
pub fn largeur_du_tiret(proximite: f64, survole: bool) -> f32 {
    if survole {
        TIRET_LARGEUR_SURVOL
    } else {
        #[allow(clippy::cast_possible_truncation)]
        let p = proximite as f32;
        TIRET_LARGEUR + p * VAGUE
    }
}

/// Un mélange en OKLab (`color-mix(in oklab, a p%, b)`) de deux couleurs opaques : ce que fait
/// Chromium pour le tiret (gris au bord, blanc au centre). Les canaux sont arrondis à l'octet.
#[must_use]
pub fn melange_oklab(a: Couleur, pourcentage: f64, b: Couleur) -> Couleur {
    fn lineaire(c: u8) -> f64 {
        let x = f64::from(c) / 255.0;
        if x <= 0.040_45 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    }
    fn gamma(x: f64) -> u8 {
        let x = x.clamp(0.0, 1.0);
        let v = if x <= 0.003_130_8 {
            x * 12.92
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let o = (v * 255.0).round() as u8;
        o
    }
    fn vers_lab(c: Couleur) -> [f64; 3] {
        let (r, g, bl) = (lineaire(c.r), lineaire(c.g), lineaire(c.b));
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * bl).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * bl).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * bl).cbrt();
        [
            0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
            1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
            0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
        ]
    }
    let t = (pourcentage / 100.0).clamp(0.0, 1.0);
    let (la, lb) = (vers_lab(a), vers_lab(b));
    let [l0, a0, b0] = [0, 1, 2].map(|i| la[i] * t + lb[i] * (1.0 - t));
    let l = (l0 + 0.396_337_777_4 * a0 + 0.215_803_757_3 * b0).powi(3);
    let m = (l0 - 0.105_561_345_8 * a0 - 0.063_854_172_8 * b0).powi(3);
    let s = (l0 - 0.089_484_177_5 * a0 - 1.291_485_548_0 * b0).powi(3);
    Couleur::octets(
        gamma(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        gamma(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        gamma(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
        255,
    )
}

/// Le rail.
pub struct RailChat {
    tours: Vec<RailTurn>,
    proximite: HashMap<String, f64>,
    /// Le tiret survolé ou courant au clavier.
    survol: Option<usize>,
    au_clic: AuClic,
    focus: FocusHandle,
}

impl RailChat {
    #[must_use]
    pub fn new(au_clic: AuClic, cx: &mut Context<Self>) -> Self {
        Self {
            tours: Vec::new(),
            proximite: HashMap::new(),
            survol: None,
            au_clic,
            focus: cx.focus_handle(),
        }
    }

    /// Les tours (après chaque mise à jour du fil).
    pub fn poser_tours(&mut self, tours: Vec<RailTurn>, cx: &mut Context<Self>) {
        if self.tours != tours {
            self.survol = self.survol.filter(|&i| i < tours.len());
            self.tours = tours;
            cx.notify();
        }
    }

    /// La proximité de chaque tour au centre de l'écran, relevée sur les mesures du défileur.
    pub fn poser_proximite(&mut self, m: &Mesures, cx: &mut Context<Self>) {
        let questions: Vec<RailQuestion> = m
            .boites
            .iter()
            .filter(|b| b.question)
            .map(|b| RailQuestion {
                id: b.id.clone(),
                top: b.top,
            })
            .collect();
        let p = rail_proximity(
            &questions,
            m.scroll_top,
            m.scroll_top + m.visible,
            m.bas_contenu,
        );
        if p != self.proximite {
            self.proximite = p;
            cx.notify();
        }
    }

    /// Le focus du rail (↑ / ↓ entre les tirets).
    #[must_use]
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Le rail est-il affiché (`hidden = tours < 3`) ?
    #[must_use]
    pub fn visible(&self) -> bool {
        self.tours.len() >= RAIL_MIN_TURNS
    }

    #[must_use]
    pub fn tours(&self) -> &[RailTurn] {
        &self.tours
    }

    /// La proximité (0 à 1) du tour `id`.
    #[must_use]
    pub fn proximite(&self, id: &str) -> f64 {
        self.proximite.get(id).copied().unwrap_or(0.0)
    }

    /// `is-visible` : le tour est (en partie) à l'écran.
    #[must_use]
    pub fn a_l_ecran(&self, id: &str) -> bool {
        self.proximite(id) > 0.0
    }

    /// Le tour dont la carte est montrée.
    #[must_use]
    pub fn apercu(&self) -> Option<&RailTurn> {
        self.survol.and_then(|i| self.tours.get(i))
    }

    /// Le survol d'un tiret (la souris, ou le clavier).
    pub fn survoler(&mut self, i: Option<usize>, cx: &mut Context<Self>) {
        if self.survol != i {
            self.survol = i;
            cx.notify();
        }
    }

    /// Le clic sur le tiret `i`.
    pub fn aller(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.tours.get(i) {
            let id = t.id.clone();
            (self.au_clic)(&id, cx);
        }
    }

    /// ↑ / ↓ entre les tiret, Entrée le déclenche.
    fn sur_touche(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.tours.is_empty() {
            return;
        }
        let dernier = self.tours.len() - 1;
        match ev.keystroke.key.as_str() {
            "down" | "up" if !ev.keystroke.modifiers.alt => {
                let i = self.survol.unwrap_or(0);
                let j = if ev.keystroke.key == "down" {
                    (i + 1).min(dernier)
                } else {
                    i.saturating_sub(1)
                };
                self.survol = Some(j);
            }
            "enter" => {
                if let Some(i) = self.survol {
                    self.aller(i, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn carte(&self, i: usize, cx: &App) -> AnyElement {
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);
        let Some(tour) = self.tours.get(i) else {
            return div().into_any_element();
        };
        let largeur_rail = self
            .tours
            .iter()
            .enumerate()
            .map(|(k, x)| largeur_du_tiret(self.proximite(&x.id), self.survol == Some(k)))
            .fold(TIRET_LARGEUR, f32::max);
        let pas = f.size_2_1 + f.size_2_3;
        #[allow(clippy::cast_precision_loss)]
        let dy = (i as f32 - (self.tours.len() as f32 - 1.0) / 2.0) * pas;

        let question = if tour.question.is_empty() {
            "(sans texte)".to_owned()
        } else {
            tour.question.clone()
        };
        let mut carte = div()
            .w(u(cx, APERCU_LARGEUR))
            .flex_none()
            .flex()
            .flex_col()
            .gap(u(cx, f.size_2_3))
            .py(u(cx, f.size_4_2))
            .px(u(cx, f.size_4_3))
            .bg(couleur(c.menu_bg))
            .border(u(cx, f.border_width))
            .border_color(couleur(c.background_modifier_border))
            .rounded(u(cx, f.menu_radius))
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(couleur(c.text_normal))
                    .line_clamp(2)
                    .child(question),
            );
        if !tour.answer.is_empty() {
            carte = carte.child(
                div()
                    .text_color(couleur(c.text_muted))
                    .line_clamp(3)
                    .child(tour.answer.clone()),
            );
        }
        if let Some(fichier) = &tour.file {
            carte = carte.child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(cx, f.size_2_2))
                    .text_color(couleur(c.text_muted))
                    .child(
                        svg()
                            .path(icones::chemin("paperclip"))
                            .size(u(cx, f.icon_xs))
                            .flex_none(),
                    )
                    .child(fichier.clone()),
            );
        }
        let carte = ombrer(
            carte,
            cx,
            &c.menu_shadow,
            "--menu-shadow",
            u(cx, f.menu_radius),
            u(cx, f.border_width),
        );
        // Un point d'ancrage de hauteur nulle, au milieu du fil, décalé à l'ordonnée du tiret : la
        // carte s'y centre.
        div()
            .id("chat-rail-preview")
            .absolute()
            .top(relative(0.5))
            .mt(u(cx, dy))
            .left(u(cx, f.size_4_2 + largeur_rail + APERCU_ECART))
            .h(u(cx, 0.0))
            .flex()
            .items_center()
            .child(texte(
                carte,
                cx,
                t.typo.font_ui_small,
                t.typo.line_height_normal,
            ))
            .into_any_element()
    }
}

impl Render for RailChat {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let racine = div()
            .id("chat-rail-racine")
            .absolute()
            .top_0()
            .left_0()
            .size_full();
        if !self.visible() {
            return racine;
        }
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);

        let mut colonne = div()
            .id("chat-rail")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| this.sur_touche(ev, cx)))
            .flex()
            .flex_col()
            .items_start()
            .gap(u(cx, f.size_2_3))
            .max_h(relative(0.7))
            .overflow_hidden()
            .py(u(cx, f.size_2_2));
        for (i, tour) in self.tours.iter().enumerate() {
            let p = self.proximite(&tour.id);
            let survole = self.survol == Some(i);
            let fond = if survole {
                c.text_normal
            } else {
                melange_oklab(c.text_normal, p * 100.0, c.background_modifier_border_hover)
            };
            let id: ElementId = ("chat-rail-tick", i).into();
            colonne = colonne.child(
                div()
                    .id(id)
                    .flex_none()
                    .w(u(cx, largeur_du_tiret(p, survole)))
                    .h(u(cx, f.size_2_1))
                    .rounded(u(cx, 999.0))
                    .bg(couleur(fond))
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, sur: &bool, _, cx| {
                        let courant = this.survol;
                        if *sur {
                            this.survoler(Some(i), cx);
                        } else if courant == Some(i) {
                            this.survoler(None, cx);
                        }
                    }))
                    // Sans voler le focus du champ (`pointerdown` → `preventDefault`).
                    .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, window, _| {
                        window.prevent_default();
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.aller(i, cx))),
            );
        }

        let mut racine = racine.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(u(cx, f.size_4_2))
                .flex()
                .flex_col()
                .justify_center()
                .child(colonne),
        );
        if let Some(i) = self.survol {
            racine = racine.child(self.carte(i, cx));
        }
        racine
    }
}
