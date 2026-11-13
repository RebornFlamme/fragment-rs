// Port de app/src/ui/views/agent/trace/traceView.ts (rendu) et trace.css (constantes)
//! La vue GPUI de la trace : peint l'instantané de la [`Poignee`] à chaque image.
//!
//! Les constantes viennent de `trace.css` (voir `docs/constantes-rendu.md`, section
//! `ui/views/agent/trace/trace.css`) ; les couleurs sont des jetons du thème : `--chat-fg` =
//! `text_normal`, `--chat-muted` = `text_muted`, `--chat-border` = `tab_outline_color`,
//! `--chat-bg` = `workspace_bg`.
//!
//! L'ORBE n'est pas ici : c'est un emplacement ([`FournisseurOrbe`]) que remplit `fr-t7-orbe`.
//!
//! ★ Écarts assumés (voir la PR) : pas de flou (opacité seule), pas de fenêtre à masque dégradé
//! sur la pensée (un voile `workspace_bg` en tient lieu), la montée en douceur (FLIP) de la
//! pensée n'est pas rejouée, le reflet colore le libellé entier au lieu de glisser dessus.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use fragment_agent::contrat::ResultatOutil;
use fragment_agent::trace_model::{OrbState, StepKind, Trace, TraceStatus};
use fragment_core::{Command, CommandManager};
use fragment_theme::{Couleur, Theme};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, PathBuilder, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Transformation, Window, canvas, div, point, px,
    radians, relative, svg,
};

use super::animation::{CaseVue, CoucheVue, angle_arc, reflet};
use super::poignee::{
    CibleVue, EtapeVue, EtatEtape, EtatsTrace, Marque, OrbeChoix, Poignee, VueTrace, nom_orbe,
};
use crate::flottant::{ecouter_args, nombre};
use crate::icones;
use crate::polices::{FAMILLE_MONOSPACE, famille_interface};
use crate::theme_global::{CouleurGpui, couleur, theme_ou_defaut};

/// L'emplacement de l'orbe de l'en-tête (20 × 20), rempli par `fr-t7-orbe` : la vue lui donne le
/// geste (`etat`), l'encre (la couleur du texte, `--chat-fg`) et l'heure (ms) ; il rend l'élément
/// peint. Sans fournisseur, l'emplacement reste vide (sa boîte est posée quand même).
pub trait FournisseurOrbe {
    fn orbe(&self, etat: OrbState, cote: f32, encre: CouleurGpui, maintenant: i64) -> AnyElement;
}

/// L'heure en millisecondes depuis l'époque (les traces portent des heures d'horloge murale).
fn maintenant() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Les jetons de couleur de la trace, tirés du thème.
struct Encres {
    fg: CouleurGpui,
    muted: CouleurGpui,
    faint: CouleurGpui,
    bordure: CouleurGpui,
    erreur: CouleurGpui,
    fond: CouleurGpui,
    fg_brut: Couleur,
}

impl Encres {
    fn new(t: &Theme) -> Self {
        let p = &t.palette;
        Self {
            fg: couleur(p.text_normal),
            muted: couleur(p.text_muted),
            // `--trace-faint` : color-mix(in srgb, var(--chat-muted) 55%, transparent).
            faint: couleur(p.text_muted.melange(55.0, Couleur::TRANSPARENT)),
            bordure: couleur(p.tab_outline_color),
            erreur: couleur(p.text_error),
            fond: couleur(p.workspace_bg),
            fg_brut: p.text_normal,
        }
    }

    /// `color-mix(in srgb, var(--chat-fg) N%, transparent)`.
    fn fg_pour_cent(&self, n: f32) -> CouleurGpui {
        couleur(self.fg_brut.melange(n, Couleur::TRANSPARENT))
    }
}

/// Le mélange du gris vers l'encre (le reflet d'un libellé en cours).
fn teinte(a: CouleurGpui, b: CouleurGpui, t: f32) -> CouleurGpui {
    let mut c = a;
    c.r = a.r + (b.r - a.r) * t;
    c.g = a.g + (b.g - a.g) * t;
    c.b = a.b + (b.b - a.b) * t;
    c.a = a.a + (b.a - a.a) * t;
    c
}

fn avec_opacite(mut c: CouleurGpui, o: f32) -> CouleurGpui {
    c.a *= o.clamp(0.0, 1.0);
    c
}

/// La vue du part `agent-trace`.
pub struct TraceView {
    poignee: Poignee,
    orbe: Option<Rc<dyn FournisseurOrbe>>,
    reduit: bool,
    /// Hauteur naturelle mesurée du corps et des détails (pour animer la hauteur).
    corps_h: Rc<Cell<f32>>,
    details_h: RefCell<HashMap<String, Rc<Cell<f32>>>>,
    minuterie: bool,
}

impl TraceView {
    /// Une vue sur une trace. `etats` garde ce que l'utilisateur a choisi d'un rendu à l'autre
    /// (clé : le message). `reduit` : `prefers-reduced-motion`.
    pub fn new(
        etats: &mut EtatsTrace,
        cle: &str,
        trace: Trace,
        orbe: OrbeChoix,
        reduit: bool,
    ) -> Self {
        Self {
            poignee: Poignee::new(etats.pour(cle), trace, orbe, reduit, maintenant()),
            orbe: None,
            reduit,
            corps_h: Rc::new(Cell::new(0.0)),
            details_h: RefCell::default(),
            minuterie: false,
        }
    }

    /// Branche l'orbe (`fr-t7-orbe`).
    #[must_use]
    pub fn avec_orbe(mut self, f: Rc<dyn FournisseurOrbe>) -> Self {
        self.orbe = Some(f);
        self
    }

    /// Une nouvelle version du part : la vue se met à jour, rien n'est reconstruit.
    pub fn update(&mut self, trace: Trace, cx: &mut Context<Self>) {
        self.poignee.update(trace, maintenant());
        cx.notify();
    }

    /// L'orbe choisie dans les réglages, relayée en cours de route.
    pub fn definir_orbe(&mut self, orbe: OrbeChoix, cx: &mut Context<Self>) {
        self.poignee.definir_orbe(orbe, maintenant());
        cx.notify();
    }

    /// La poignée (tests, hôte).
    #[must_use]
    pub fn poignee(&self) -> &Poignee {
        &self.poignee
    }

    /// La hauteur naturelle mesurée du détail d'une étape (partagée avec le canvas de mesure).
    fn mesure_de(&self, id: &str) -> Rc<Cell<f32>> {
        self.details_h
            .borrow_mut()
            .entry(id.to_string())
            .or_default()
            .clone()
    }

    fn clic_en_tete(&mut self, cx: &mut Context<Self>) {
        self.poignee.clic_en_tete(maintenant());
        cx.notify();
    }

    fn clic_etape(&mut self, id: &str, cx: &mut Context<Self>) {
        self.poignee.clic_etape(id, maintenant());
        cx.notify();
    }
}

// ── les pièces ───────────────────────────────────────────────────────────────

fn icone(nom: &str, cote: f32, c: CouleurGpui) -> impl IntoElement {
    svg().path(icones::chemin(nom)).size(px(cote)).text_color(c)
}

fn chevron(ouvert: bool, c: CouleurGpui, visible: bool) -> AnyElement {
    let mut e = div().flex().flex_none().items_center();
    if !visible {
        e = e.invisible();
    }
    e.child(
        svg()
            .path(icones::chemin("chevron-right"))
            .size(px(13.0))
            .text_color(c)
            .with_transformation(Transformation::rotate(radians(if ouvert {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            }))),
    )
    .into_any_element()
}

/// Un texte en fondu croisé : la couche courante dans le flux, les sortantes superposées.
fn texte_fondu(
    couches: &[CoucheVue],
    encres: &Encres,
    maintenant: i64,
    reduit: bool,
) -> AnyElement {
    let mut boite = div()
        .relative()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap();
    for c in couches {
        let mut couche = div()
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .opacity(c.opacite)
            .text_color(if c.reflet && !reduit {
                teinte(encres.muted, encres.fg, reflet(maintenant))
            } else {
                encres.muted
            })
            .child(c.texte.clone());
        if c.sortante {
            couche = couche.absolute().top_0().left_0();
        }
        boite = boite.child(couche);
    }
    boite.into_any_element()
}

fn chrono(cases: &[CaseVue], encres: &Encres, hauteur_case: f32) -> AnyElement {
    let mut ligne = div()
        .flex()
        .flex_none()
        .font_family(FAMILLE_MONOSPACE)
        .text_color(encres.faint);
    for c in cases {
        let mut boite = div().relative().h(px(hauteur_case)).overflow_hidden();
        let decalage = hauteur_case * 0.7;
        if let Some(a) = c.ancien {
            boite = boite.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(-c.roulement * decalage))
                    .opacity(1.0 - c.roulement)
                    .whitespace_nowrap()
                    .child(a.to_string()),
            );
        }
        boite = boite.child(
            div()
                .relative()
                .top(px((1.0 - c.roulement) * decalage))
                .opacity(c.roulement)
                .whitespace_nowrap()
                .child(c.courant.to_string()),
        );
        ligne = ligne.child(boite);
    }
    ligne.into_any_element()
}

/// L'arc qui tourne : un anneau pâle et son quart supérieur à l'encre (11 px, trait de 1,5 px).
fn arc(encres: &Encres, epaisseur: f32, angle: f32) -> AnyElement {
    let (anneau, plein) = (avec_opacite(encres.muted, 0.30), encres.fg);
    canvas(
        |_, _, _| (),
        move |bornes, (), fenetre, _| {
            let cote = f32::from(bornes.size.width).min(f32::from(bornes.size.height));
            let r = (cote - epaisseur) / 2.0;
            let centre = point(
                bornes.origin.x + bornes.size.width / 2.0,
                bornes.origin.y + bornes.size.height / 2.0,
            );
            let sur = |deg: f32| {
                let a = deg.to_radians();
                point(centre.x + px(r * a.cos()), centre.y + px(r * a.sin()))
            };
            let rayons = point(px(r), px(r));
            let mut cercle = PathBuilder::stroke(px(epaisseur));
            cercle.move_to(sur(0.0));
            cercle.arc_to(rayons, px(0.0), false, true, sur(180.0));
            cercle.arc_to(rayons, px(0.0), false, true, sur(360.0));
            if let Ok(chemin) = cercle.build() {
                fenetre.paint_path(chemin, anneau);
            }
            let mut quart = PathBuilder::stroke(px(epaisseur));
            quart.move_to(sur(angle - 135.0));
            quart.arc_to(rayons, px(0.0), false, true, sur(angle - 45.0));
            if let Ok(chemin) = quart.build() {
                fenetre.paint_path(chemin, plein);
            }
        },
    )
    .size(px(11.0))
    .into_any_element()
}

fn marqueur(e: &EtapeVue, encres: &Encres, epaisseur: f32, angle: f32) -> AnyElement {
    let contenu = match e.marque {
        Marque::Arc => arc(encres, epaisseur, angle),
        Marque::Point => div()
            .size(px(5.0))
            .rounded_full()
            .bg(if e.etat == EtatEtape::Error {
                encres.erreur
            } else {
                encres.faint
            })
            .into_any_element(),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(14.0))
        .child(contenu)
        .into_any_element()
}

fn puce(c: &CibleVue, encres: &Encres, t: &Theme) -> AnyElement {
    let f = &t.forme;
    let mut p = div()
        .flex()
        .items_center()
        .gap(px(f.size_4_1))
        .min_w_0()
        .px(px(f.size_2_3))
        .rounded(px(f.radius_sm))
        .bg(encres.fg_pour_cent(6.0))
        .text_color(encres.fg)
        .line_height(relative(1.6))
        .overflow_hidden()
        .whitespace_nowrap();
    p = if c.mono {
        p.font_family(FAMILLE_MONOSPACE)
            .text_size(px(t.typo.font_ui_small))
    } else {
        p.font_weight(FontWeight::MEDIUM)
    };
    if let Some(i) = &c.icone {
        p = p.child(div().flex().flex_none().child(icone(i, 12.0, encres.muted)));
    }
    p.child(
        div()
            .overflow_hidden()
            .text_ellipsis()
            .child(c.texte.clone()),
    )
    .into_any_element()
}

/// Le détail d'une étape : la pensée (fenêtre de 4 lignes, collée en bas) ou le résultat.
fn detail(e: &EtapeVue, encres: &Encres, t: &Theme) -> AnyElement {
    let f = &t.forme;
    if e.kind == StepKind::Thinking {
        let fenetre = px(t.typo.font_ui_medium * 1.55 * 4.0);
        let mut cadre = div()
            .relative()
            .flex()
            .flex_col()
            .justify_end()
            .mb(px(f.size_2_3))
            .overflow_hidden();
        cadre = if e.etat == EtatEtape::Done {
            // Relu (pensée finie) : tout le texte, sans fenêtre (18 em).
            cadre.max_h(px(t.typo.font_ui_medium * 18.0))
        } else {
            cadre.max_h(fenetre)
        };
        let mut cadre = cadre.child(
            div()
                .line_height(relative(1.55))
                .text_color(encres.faint)
                .child(e.contenu.clone()),
        );
        if e.etat != EtatEtape::Done {
            // Le fondu en haut (masque dégradé de la référence) : un voile qui s'efface vers le bas.
            cadre = cadre.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(t.typo.font_ui_medium * 1.7))
                    .bg(gpui_kit::linear_gradient(
                        180.0,
                        gpui_kit::linear_color_stop(encres.fond, 0.0),
                        gpui_kit::linear_color_stop(avec_opacite(encres.fond, 0.0), 1.0),
                    )),
            );
        }
        return cadre.into_any_element();
    }
    div()
        .mt(px(f.size_2_1))
        .mb(px(f.size_2_3))
        .py(px(7.0))
        .px(px(f.size_2_5))
        .border(px(f.border_width))
        .border_color(encres.bordure)
        .rounded(px(f.radius_m))
        .bg(encres.fg_pour_cent(3.0))
        .font_family(FAMILLE_MONOSPACE)
        .text_size(px(11.5))
        .line_height(relative(1.55))
        .max_h(px(10.5 * 11.5))
        .overflow_hidden()
        .child(e.contenu.clone())
        .into_any_element()
}

/// Une zone repliable : la hauteur naturelle mesurée × la fraction, le contenu en fondu.
fn repliable(
    id: impl Into<SharedString>,
    fraction: f32,
    opacite: f32,
    mesure: Rc<Cell<f32>>,
    contenu: AnyElement,
) -> AnyElement {
    let id: SharedString = id.into();
    let m = mesure.clone();
    let interieur = div()
        .id(id)
        .relative()
        .flex_none()
        .opacity(opacite)
        .child(contenu)
        .child(
            canvas(
                move |bornes, _, _| m.set(f32::from(bornes.size.height)),
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    let mut boite = div().flex().flex_col().overflow_hidden();
    boite = if fraction >= 0.999 {
        boite
    } else {
        boite.h(px(mesure.get() * fraction.max(0.0)))
    };
    boite.child(interieur).into_any_element()
}

impl TraceView {
    #[allow(clippy::too_many_arguments)] // le rendu d'une ligne lit tout le contexte de l'image
    fn etape(
        &self,
        i: usize,
        e: &EtapeVue,
        derniere: bool,
        v: &VueTrace,
        encres: &Encres,
        t: &Theme,
        angle: f32,
        now: i64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let f = &t.forme;
        let id = e.id.clone();
        let cliquable = e.a_detail;
        let mut ligne = div()
            .id(("agent-trace-ligne", i))
            .flex()
            .items_center()
            .gap(px(f.size_2_3))
            .min_h(px(26.0))
            .min_w_0()
            .child(marqueur(e, encres, f.border_width * 1.5, angle));
        if e.kind == StepKind::Note {
            ligne = ligne.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .italic()
                    .max_h(px(t.typo.font_ui_medium * t.typo.line_height_normal * 2.0))
                    .overflow_hidden()
                    .text_color(encres.muted)
                    .child(
                        e.verbe
                            .iter()
                            .find(|c| !c.sortante)
                            .map_or_else(String::new, |c| c.texte.clone()),
                    ),
            );
        } else {
            ligne = ligne.child(div().flex_none().max_w_full().child(texte_fondu(
                &e.verbe,
                encres,
                now,
                self.reduit,
            )));
        }
        if let Some(c) = &e.cible {
            ligne = ligne.child(div().flex().min_w_0().child(puce(c, encres, t)));
        }
        if !e.temps.is_empty() {
            ligne = ligne.child(
                div()
                    .flex_none()
                    .text_color(encres.faint)
                    .child(e.temps.clone()),
            );
        }
        ligne = ligne.child(chevron(e.ouvert, encres.faint, e.a_detail));
        if cliquable {
            ligne = ligne
                .cursor_pointer()
                .hover(|s| s.text_color(encres.fg))
                .on_click(cx.listener(move |this, _, _, cx| this.clic_etape(&id, cx)));
        }
        let mesure_detail = self.mesure_de(&e.id);
        let mut li = div().relative().opacity(e.entree).child(ligne);
        if e.a_detail {
            let marge = if v.seule { 0.0 } else { f.size_4_5 };
            li = li.child(div().ml(px(marge)).child(repliable(
                format!("agent-trace-detail-{}", e.id),
                e.detail_hauteur,
                e.detail_opacite,
                mesure_detail,
                detail(e, encres, t),
            )));
        }
        if !derniere {
            // Le fil qui relie les marqueurs.
            li = li.child(
                div()
                    .absolute()
                    .left(px(6.5))
                    .top(px(19.0))
                    .bottom(px(-7.0))
                    .w(px(1.0))
                    .bg(encres.bordure),
            );
        }
        li.into_any_element()
    }
}

impl Render for TraceView {
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = maintenant();
        let t = theme_ou_defaut(cx);
        let encres = Encres::new(&t);
        let f = t.forme.clone();
        if self.poignee.trace().status == TraceStatus::Running {
            self.poignee.tic(now);
        }
        let v = self.poignee.vue(now);
        let angle = if self.reduit { 0.0 } else { angle_arc(now) };

        // En-tête : orbe (pendant), titre, chrono, chevron.
        let mut tete = div()
            .id("agent-trace-tete")
            .flex()
            .items_center()
            .gap(px(f.size_2_3))
            .max_w_full()
            .min_h(px(26.0))
            .pr(px(f.size_2_3))
            .rounded(px(f.radius_sm))
            .text_color(encres.muted)
            .cursor_pointer()
            .hover(|s| s.text_color(encres.fg));
        if let Some(etat) = v.orbe {
            let contenu = self
                .orbe
                .as_ref()
                .map(|o| o.orbe(etat, 20.0, encres.fg, now));
            let mut statut = div()
                .id(SharedString::from(format!(
                    "agent-trace-orbe-{}",
                    nom_orbe(etat)
                )))
                .flex()
                .flex_none()
                .size(px(20.0))
                .text_color(encres.fg);
            if let Some(c) = contenu {
                statut = statut.child(c);
            }
            tete = tete.child(statut);
        }
        tete = tete.child(
            div()
                .min_w_0()
                .child(texte_fondu(&v.titre, &encres, now, self.reduit)),
        );
        // Une pensée SEULE finie : le chrono ferait doublon avec « A réfléchi N s ».
        if !(v.seule && v.status == TraceStatus::Done) {
            tete = tete.child(chrono(
                &v.chrono_cases,
                &encres,
                t.typo.font_ui_medium * t.typo.line_height_normal,
            ));
        }
        tete = tete
            .child(chevron(v.ouvert, encres.faint, !v.vide))
            .on_click(cx.listener(|this, _, _, cx| this.clic_en_tete(cx)));
        if v.vide {
            tete = tete.cursor_default();
        }

        // Corps : la timeline.
        let n = v.etapes.len();
        let mut liste = div().flex().flex_col().mt(px(f.size_2_1));
        for (i, e) in v.etapes.iter().enumerate() {
            // Une pensée SEULE n'a pas de ligne : le corps de la trace EST sa pensée.
            if v.seule {
                liste = liste.child(div().child(repliable(
                    format!("agent-trace-detail-{}", e.id),
                    e.detail_hauteur,
                    e.detail_opacite,
                    self.mesure_de(&e.id),
                    detail(e, &encres, &t),
                )));
            } else {
                liste = liste.child(self.etape(i, e, i + 1 == n, &v, &encres, &t, angle, now, cx));
            }
        }
        let corps = repliable(
            "agent-trace-corps",
            v.corps_hauteur,
            v.corps_opacite,
            self.corps_h.clone(),
            liste.into_any_element(),
        );

        // Les animations tournent tant qu'il y en a ; sinon l'image se fige.
        if self.poignee.anime(now) {
            if self.reduit {
                // Mouvement réduit : seul le chrono avance, au rythme de la seconde.
                self.armer_minuterie(cx);
            } else {
                window.request_animation_frame();
            }
        }

        let racine = div()
            .id("agent-trace")
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .mb(px(f.size_2_5))
            .text_size(px(t.typo.font_ui_medium))
            .line_height(relative(t.typo.line_height_normal))
            .text_color(encres.muted)
            .child(tete)
            .child(div().w_full().child(corps));
        famille_interface(racine)
    }
}

impl TraceView {
    /// Un battement de 250 ms pendant que le tour dure (le chrono avance même sans animation).
    fn armer_minuterie(&mut self, cx: &mut Context<Self>) {
        if self.minuterie {
            return;
        }
        self.minuterie = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            this.update(cx, |t, cx| {
                t.minuterie = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ── la commande de scène ─────────────────────────────────────────────────────

thread_local! {
    static SCENE_DEPLIE: Cell<bool> = const { Cell::new(false) };
}

/// La trace de la scène `pixels:trace` : un tour fini (pensée de 2 s, recherche, narration,
/// lecture) sous des heures fixes, pour que la capture ne dépende pas de l'horloge.
#[must_use]
pub fn trace_de_scene() -> Trace {
    use fragment_agent::trace_model::{OutilEntrant, TraceBuilder};
    let t0 = 1_700_000_000_000_i64;
    let entree = |v: serde_json::Value| v.as_object().cloned();
    let mut b = TraceBuilder::new(t0);
    b.thinking("", t0, true);
    b.thinking(
        "Je cherche où la note parle du sujet, puis je la relis.",
        t0,
        false,
    );
    b.tool(
        OutilEntrant {
            id: "g1".into(),
            name: "Grep".into(),
            input: entree(serde_json::json!({ "pattern": "photon" })),
        },
        t0 + 2000,
    );
    b.result(
        "g1",
        ResultatOutil {
            is_error: false,
            text: "cours.md:4: le photon est un quantum.".into(),
        },
        t0 + 2200,
    );
    b.note("Je lis la note pour vérifier.", t0 + 2500);
    b.tool(
        OutilEntrant {
            id: "r1".into(),
            name: "Read".into(),
            input: entree(serde_json::json!({ "file_path": "C:/coffre/cours.md" })),
        },
        t0 + 2600,
    );
    b.result(
        "r1",
        ResultatOutil {
            is_error: false,
            text: "# Cours".into(),
        },
        t0 + 3000,
    );
    b.finish(t0 + 3000, false);
    b.snapshot()
}

/// La vue de la scène : la trace fixe, mouvement réduit (les références sont prises sous
/// `prefers-reduced-motion`), repliée ou dépliée selon l'argument `deplie` de `pixels:trace`.
pub fn vue_de_scene(cx: &mut App) -> Entity<TraceView> {
    let mut etats = EtatsTrace::new();
    let deplie = SCENE_DEPLIE.with(Cell::get);
    let mut vue = TraceView::new(&mut etats, "scene", trace_de_scene(), OrbeChoix::Auto, true);
    if deplie {
        vue.poignee.clic_en_tete(maintenant());
    }
    cx.new(|_| vue)
}

/// Enregistre la commande de scène `pixels:trace` (`{ "deplie": 0 | 1 }`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    SCENE_DEPLIE.with(|d| d.set(false));
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:trace", "Pixels : poser la trace d'un tour").with_callback(
        move || {
            let a = args.borrow();
            let deplie = a
                .get("deplie")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or_else(|| nombre(&a, "deplie").is_some_and(|n| n > 0.0));
            SCENE_DEPLIE.with(|d| d.set(deplie));
        },
    );
    let _retrait = commandes.add_command(commande, "pixels");
}
