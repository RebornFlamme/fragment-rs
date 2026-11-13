// Port de HtmlBlockWidget et HtmlInlineWidget d'app/src/ui/views/markdown/htmlDecorations.ts et de
// html-markdown.css (`.cm-html-block`, `.edit-block-button`), traduits en éléments GPUI (ADR-012).
//! Le widget d'un bloc HTML : l'arbre assaini ([`super::arbre`]) TRADUIT en éléments GPUI, le
//! bouton « </> » du coin, un `<details>` qui s'ouvre, un SVG (animé) peint par le rastériseur.
//!
//! - le texte en ligne d'un élément (texte, `span`, `b`, `mark`, `u`, `a`, `br`…) devient UN texte
//!   balisé (`StyledText`) : il coule et se replie comme une ligne ;
//! - un élément de bloc devient un `div` en colonne, stylé par le sous-ensemble CSS ([`super::css`]) ;
//! - le rendu est INTERACTIF (`ignoreEvent` de Fragment) : il occulte l'éditeur dessous, un clic n'y
//!   déplace pas le curseur ; on repasse à la source par le bouton « </> » (événement
//!   [`EvenementBloc::Source`]) ;
//! - sa hauteur est mesurée à chaque image ([`EvenementBloc::Hauteur`]) : l'hôte la dicte à la
//!   rangée du bloc.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fragment_theme::{Aplat, Couleur, Theme};
use gpui_kit::{
    AbsoluteLength, AnyElement, Bounds, Context, EdgesRefinement, EventEmitter, FontStyle,
    FontWeight, HighlightStyle, ImageSource, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Render, RenderImage, StatefulInteractiveElement as _,
    StrikethroughStyle, Styled as _, StyledText, TextAlign, UnderlineStyle, Window, canvas, div,
    img, px, svg as icone_svg,
};

use super::arbre::{Element, Noeud, analyser};
use super::css::{self, Alignement, StyleCss};
use super::{smil, svg};
use crate::icones;
use crate::polices::FAMILLE_MONOSPACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// `.cm-html-block > .edit-block-button` : `top` / `right` (html-markdown.css:21-22) et `padding`
/// (:24), en px.
pub const BOUTON_DECALAGE: f32 = 4.0;
pub const BOUTON_PADDING: f32 = 3.0;
/// L'opacité du bouton au survol du bloc (html-markdown.css:31).
const BOUTON_OPACITE_SURVOL: f32 = 0.8;
/// L'icône du bouton (`iconSvg('code-xml')`).
const ICONE_BOUTON: &str = "code-xml";
/// La cadence des animations SMIL (ADR-012 §4 : 30 images/s au plus).
const IMAGE_ANIMATION: Duration = Duration::from_millis(33);
/// Retrait d'une liste et d'une citation (feuille du navigateur : `padding-inline-start: 40px`).
const RETRAIT_LISTE: f32 = 40.0;

/// Ce que le bloc dit à son hôte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvenementBloc {
    /// La hauteur du rendu a changé (px logiques, padding compris).
    Hauteur(f32),
    /// Le bouton « </> » : montrer la source (le curseur va au début du bloc).
    Source,
}

/// La géométrie relevée à la dernière image (repère fenêtre).
#[derive(Debug, Clone, Default)]
pub struct GeoBloc {
    /// La boîte du widget (`.cm-html-block`).
    pub boite: Bounds<Pixels>,
    /// La boîte du bouton « </> » (absent en lecture seule).
    pub bouton: Option<Bounds<Pixels>>,
    /// Les éléments qui portent un `id` (`#x`) ou une classe (`.x`) : leur boîte.
    pub elements: HashMap<String, Bounds<Pixels>>,
}

/// Le widget d'un bloc HTML (une entité GPUI).
pub struct BlocHtml {
    source: String,
    arbre: Vec<Noeud>,
    /// Le début du bloc dans la note (UTF-16) : où le bouton « </> » pose le curseur.
    debut: usize,
    premiere_ligne: usize,
    lecture_seule: bool,
    /// Les `<details>` ouverts, par leur chemin dans l'arbre.
    ouverts: HashSet<Vec<usize>>,
    /// L'horloge des animations.
    origine: Instant,
    /// Les SVG rastérisés : par chemin, (texte rastérisé, densité, image).
    images: HashMap<Vec<usize>, (String, f32, Arc<RenderImage>)>,
    survole: bool,
    hauteur: Rc<RefCell<f32>>,
    geo: Rc<RefCell<GeoBloc>>,
    anime_en_cours: bool,
}

impl EventEmitter<EvenementBloc> for BlocHtml {}

impl BlocHtml {
    #[must_use]
    pub fn new(source: &str, debut: usize, premiere_ligne: usize, lecture_seule: bool) -> Self {
        let mut b = BlocHtml {
            source: String::new(),
            arbre: Vec::new(),
            debut,
            premiere_ligne,
            lecture_seule,
            ouverts: HashSet::new(),
            origine: Instant::now(),
            images: HashMap::new(),
            survole: false,
            hauteur: Rc::new(RefCell::new(0.0)),
            geo: Rc::default(),
            anime_en_cours: false,
        };
        b.poser_source(source);
        b
    }

    /// Change la source (une frappe dans le bloc puis sortie du curseur) : l'arbre est refait, les
    /// `<details>` reprennent leur attribut `open`.
    pub fn poser_source(&mut self, source: &str) {
        if self.source == source {
            return;
        }
        source.clone_into(&mut self.source);
        self.arbre = analyser(source);
        self.ouverts.clear();
        self.images.clear();
        let mut chemin = Vec::new();
        ouverts_initiaux(&self.arbre, &mut chemin, &mut self.ouverts);
    }

    /// Repose la place du bloc dans la note.
    pub fn poser_place(&mut self, debut: usize, premiere_ligne: usize) {
        self.debut = debut;
        self.premiere_ligne = premiere_ligne;
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// L'arbre assaini rendu.
    #[must_use]
    pub fn arbre(&self) -> &[Noeud] {
        &self.arbre
    }

    #[must_use]
    pub fn debut(&self) -> usize {
        self.debut
    }

    #[must_use]
    pub fn premiere_ligne(&self) -> usize {
        self.premiere_ligne
    }

    /// La géométrie de la dernière image.
    #[must_use]
    pub fn geometrie(&self) -> GeoBloc {
        self.geo.borrow().clone()
    }

    /// Le `<details>` d'`id` donné est-il ouvert ?
    #[must_use]
    pub fn details_ouvert(&self, id: &str) -> Option<bool> {
        let chemin = chemin_de(&self.arbre, &mut Vec::new(), &|e| {
            e.attribut("id") == Some(id)
        })?;
        Some(self.ouverts.contains(&chemin))
    }

    /// Bascule un `<details>` (le clic sur son `<summary>`).
    pub fn basculer(&mut self, chemin: Vec<usize>, cx: &mut Context<Self>) {
        if !self.ouverts.remove(&chemin) {
            self.ouverts.insert(chemin);
        }
        cx.notify();
    }

    /// Le temps des animations, en secondes.
    #[must_use]
    pub fn temps(&self) -> f32 {
        self.origine.elapsed().as_secs_f32()
    }

    /// Le bloc contient-il un SVG animé ?
    #[must_use]
    pub fn anime(&self) -> bool {
        let mut a = false;
        super::arbre::parcourir(&self.arbre, &mut |e| {
            a |= e.nom.eq_ignore_ascii_case("svg") && smil::anime(e);
        });
        a
    }

    /// Le survol du bloc (le bouton « </> » paraît).
    pub fn survoler(&mut self, dedans: bool, cx: &mut Context<Self>) {
        if self.survole != dedans {
            self.survole = dedans;
            cx.notify();
        }
    }

    /// Relance l'image suivante d'une animation (30 images/s), tant qu'un SVG animé est rendu.
    fn programmer_animation(&mut self, cx: &mut Context<Self>) {
        if self.anime_en_cours {
            return;
        }
        self.anime_en_cours = true;
        cx.spawn(async move |b, cx| {
            cx.background_executor().timer(IMAGE_ANIMATION).await;
            let _ = b.update(cx, |b, cx| {
                b.anime_en_cours = false;
                cx.notify();
            });
        })
        .detach();
    }
}

fn ouverts_initiaux(noeuds: &[Noeud], chemin: &mut Vec<usize>, ouverts: &mut HashSet<Vec<usize>>) {
    for (i, n) in noeuds.iter().enumerate() {
        if let Noeud::Element(e) = n {
            chemin.push(i);
            if e.nom_bas() == "details" && e.attribut("open").is_some() {
                ouverts.insert(chemin.clone());
            }
            ouverts_initiaux(&e.enfants, chemin, ouverts);
            chemin.pop();
        }
    }
}

fn chemin_de(
    noeuds: &[Noeud],
    chemin: &mut Vec<usize>,
    f: &dyn Fn(&Element) -> bool,
) -> Option<Vec<usize>> {
    for (i, n) in noeuds.iter().enumerate() {
        if let Noeud::Element(e) = n {
            chemin.push(i);
            if f(e) {
                return Some(chemin.clone());
            }
            if let Some(c) = chemin_de(&e.enfants, chemin, f) {
                return Some(c);
            }
            chemin.pop();
        }
    }
    None
}

/// Les éléments qui coulent dans une ligne de texte.
const EN_LIGNE: [&str; 24] = [
    "span", "b", "strong", "i", "em", "u", "ins", "s", "del", "strike", "mark", "a", "code", "kbd",
    "sub", "sup", "small", "br", "abbr", "cite", "q", "font", "label", "samp",
];

fn est_en_ligne(n: &Noeud) -> bool {
    match n {
        Noeud::Texte(_) => true,
        Noeud::Element(e) => !e.svg && EN_LIGNE.contains(&e.nom_bas().as_str()),
    }
}

/// Ce qui se transmet aux descendants pendant la traduction.
#[derive(Clone)]
struct Ctx {
    theme: Theme,
    taille: f32,
    densite: f32,
    temps: f32,
}

/// Le style d'un élément en ligne (feuille du navigateur, puis son attribut `style`).
#[must_use]
pub fn style_en_ligne(
    theme: &Theme,
    nom: &str,
    attribut_style: Option<&str>,
    taille: f32,
) -> HighlightStyle {
    let p = &theme.palette;
    let mut h = HighlightStyle::default();
    let trait_ = || UnderlineStyle {
        thickness: px(1.),
        color: None,
        wavy: false,
    };
    match nom {
        "b" | "strong" => h.font_weight = Some(FontWeight::BOLD),
        "i" | "em" | "cite" => h.font_style = Some(FontStyle::Italic),
        "u" | "ins" => h.underline = Some(trait_()),
        "s" | "del" | "strike" => {
            h.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            });
        }
        "mark" => h.background_color = Some(couleur(p.text_highlight_bg).into()),
        "a" => {
            h.color = Some(couleur(p.link_color).into());
            h.underline = Some(trait_());
        }
        _ => {}
    }
    if let Some(s) = attribut_style {
        let c = css::lire(theme, s, taille);
        if let Some(x) = c.couleur {
            h.color = Some(couleur(x).into());
        }
        if let Some(x) = c.fond {
            h.background_color = Some(couleur(x).into());
        }
        if let Some(g) = c.gras {
            h.font_weight = Some(if g {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            });
        }
        if c.italique {
            h.font_style = Some(FontStyle::Italic);
        }
        if c.souligne {
            h.underline = Some(trait_());
        }
        if c.barre {
            h.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            });
        }
    }
    h
}

/// Superpose `dessus` à `dessous` (ce que `dessus` précise l'emporte).
fn fusionner(dessous: HighlightStyle, dessus: HighlightStyle) -> HighlightStyle {
    HighlightStyle {
        color: dessus.color.or(dessous.color),
        font_weight: dessus.font_weight.or(dessous.font_weight),
        font_style: dessus.font_style.or(dessous.font_style),
        background_color: dessus.background_color.or(dessous.background_color),
        underline: dessus.underline.or(dessous.underline),
        strikethrough: dessus.strikethrough.or(dessous.strikethrough),
        fade_out: dessus.fade_out.or(dessous.fade_out),
    }
}

/// Un paragraphe de texte balisé en construction (blancs repliés comme `white-space: normal`).
#[derive(Default)]
struct Paragraphe {
    texte: String,
    styles: Vec<(Range<usize>, HighlightStyle)>,
}

impl Paragraphe {
    fn pousser(&mut self, t: &str, style: HighlightStyle) {
        let debut = self.texte.len();
        for c in t.chars() {
            if c.is_whitespace() {
                if !self.texte.is_empty() && !self.texte.ends_with([' ', '\n']) {
                    self.texte.push(' ');
                }
            } else {
                self.texte.push(c);
            }
        }
        if self.texte.len() > debut && style != HighlightStyle::default() {
            self.styles.push((debut..self.texte.len(), style));
        }
    }

    fn saut(&mut self) {
        while self.texte.ends_with(' ') {
            self.texte.pop();
        }
        self.texte.push('\n');
    }

    fn vide(&self) -> bool {
        self.texte.trim().is_empty()
    }

    fn element(mut self) -> AnyElement {
        while self.texte.ends_with(' ') {
            self.texte.pop();
        }
        let n = self.texte.len();
        let styles: Vec<_> = self
            .styles
            .into_iter()
            .map(|(r, s)| (r.start.min(n)..r.end.min(n), s))
            .filter(|(r, _)| !r.is_empty())
            .collect();
        StyledText::new(self.texte)
            .with_highlights(styles)
            .into_any_element()
    }
}

fn paragraphe_en_ligne(noeuds: &[Noeud], ctx: &Ctx, herite: HighlightStyle, p: &mut Paragraphe) {
    for n in noeuds {
        match n {
            Noeud::Texte(t) => p.pousser(t, herite),
            Noeud::Element(e) => {
                let nom = e.nom_bas();
                if nom == "br" {
                    p.saut();
                    continue;
                }
                let s = fusionner(
                    herite,
                    style_en_ligne(&ctx.theme, &nom, e.attribut("style"), ctx.taille),
                );
                paragraphe_en_ligne(&e.enfants, ctx, s, p);
            }
        }
    }
}

impl BlocHtml {
    /// Traduit une suite de nœuds : les nœuds en ligne consécutifs forment un paragraphe, chaque
    /// élément de bloc son `div`.
    fn traduire(
        &mut self,
        noeuds: &[Noeud],
        chemin: &mut Vec<usize>,
        ctx: &Ctx,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut sortie = Vec::new();
        let mut p = Paragraphe::default();
        for (i, n) in noeuds.iter().enumerate() {
            if est_en_ligne(n) {
                match n {
                    Noeud::Texte(t) => p.pousser(t, HighlightStyle::default()),
                    Noeud::Element(_) => {
                        paragraphe_en_ligne(
                            std::slice::from_ref(n),
                            ctx,
                            HighlightStyle::default(),
                            &mut p,
                        );
                    }
                }
                continue;
            }
            if !p.vide() {
                sortie.push(std::mem::take(&mut p).element());
            } else {
                p = Paragraphe::default();
            }
            let Noeud::Element(e) = n else { continue };
            chemin.push(i);
            if let Some(el) = self.element_de_bloc(e, chemin, ctx, cx) {
                sortie.push(el);
            }
            chemin.pop();
        }
        if !p.vide() {
            sortie.push(p.element());
        }
        sortie
    }

    #[allow(clippy::too_many_lines)]
    fn element_de_bloc(
        &mut self,
        e: &Element,
        chemin: &mut Vec<usize>,
        ctx: &Ctx,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let nom = e.nom_bas();
        let th = &ctx.theme;
        let st = e
            .attribut("style")
            .map(|s| css::lire(th, s, ctx.taille))
            .unwrap_or_default();
        if st.cache {
            return None;
        }
        let corps: AnyElement = match nom.as_str() {
            "svg" => self.svg(e, chemin, ctx, ctx.densite),
            "hr" => div()
                .my(px(ctx.taille / 2.0))
                .h(px(th.forme.border_width))
                .bg(couleur(
                    th.palette
                        .aplat(Aplat::Bordure, th.palette.background_primary),
                ))
                .into_any_element(),
            "iframe" => {
                // ADR-012 §6 : une carte de repli (lien et titre), pas de lecteur vivant.
                let titre = e
                    .attribut("title")
                    .or_else(|| e.attribut("src"))
                    .unwrap_or("iframe")
                    .to_owned();
                div()
                    .p(px(th.forme.size_4_1))
                    .border_1()
                    .border_color(couleur(
                        th.palette
                            .aplat(Aplat::Bordure, th.palette.background_primary),
                    ))
                    .rounded(px(th.forme.radius_s))
                    .text_color(couleur(th.palette.text_muted))
                    .child(titre)
                    .into_any_element()
            }
            "img" => {
                // Dette : l'image n'est pas décodée ici (pièces jointes : T9) ; le texte de repli.
                let alt = e.attribut("alt").unwrap_or("").to_owned();
                div()
                    .text_color(couleur(th.palette.text_faint))
                    .child(alt)
                    .into_any_element()
            }
            "details" => self.details(e, chemin, ctx, cx),
            "table" | "thead" | "tbody" | "tfoot" => {
                let enfants = self.traduire(&e.enfants, chemin, ctx, cx);
                let mut d = div().flex().flex_col();
                if nom == "table" {
                    d = d.border_1().border_color(couleur(
                        th.palette
                            .aplat(Aplat::Bordure, th.palette.background_primary),
                    ));
                }
                d.children(enfants).into_any_element()
            }
            "tr" => {
                let enfants = self.traduire(&e.enfants, chemin, ctx, cx);
                div().flex().flex_row().children(enfants).into_any_element()
            }
            "td" | "th" => {
                let enfants = self.traduire(&e.enfants, chemin, ctx, cx);
                let mut d = div().flex_1().flex().flex_col().p(px(1.0));
                if nom == "th" {
                    d = d.font_weight(FontWeight::BOLD);
                }
                d.children(enfants).into_any_element()
            }
            "ul" | "ol" => {
                let mut items = Vec::new();
                let mut n = 0;
                for (i, x) in e.enfants.iter().enumerate() {
                    let Noeud::Element(li) = x else { continue };
                    n += 1;
                    chemin.push(i);
                    let contenu = self.traduire(&li.enfants, chemin, ctx, cx);
                    chemin.pop();
                    let puce = if nom == "ol" {
                        format!("{n}.")
                    } else {
                        "•".to_owned()
                    };
                    items.push(
                        div()
                            .flex()
                            .flex_row()
                            .child(div().w(px(RETRAIT_LISTE)).flex_none().child(puce))
                            .child(div().flex_1().flex().flex_col().children(contenu))
                            .into_any_element(),
                    );
                }
                div().flex().flex_col().children(items).into_any_element()
            }
            _ => {
                let enfants = self.traduire(&e.enfants, chemin, ctx, cx);
                let mut d = div().flex().flex_col();
                match nom.as_str() {
                    "p" => d = d.my(px(ctx.taille)),
                    "blockquote" => d = d.my(px(ctx.taille)).ml(px(RETRAIT_LISTE)),
                    "pre" => d = d.font_family(FAMILLE_MONOSPACE),
                    h if h.len() == 2 && h.starts_with('h') => {
                        let em = match h {
                            "h1" => 2.0,
                            "h2" => 1.5,
                            "h3" => 1.17,
                            "h5" => 0.83,
                            "h6" => 0.67,
                            _ => 1.0,
                        };
                        d = d
                            .text_size(px(ctx.taille * em))
                            .font_weight(FontWeight::BOLD)
                            .my(px(ctx.taille * em * 0.67));
                    }
                    _ => {}
                }
                d.children(enfants).into_any_element()
            }
        };
        let mut boite = div().child(corps);
        boite = appliquer(boite, &st);
        if e.attribut("id").is_some() || e.attribut("class").is_some() {
            let cles: Vec<String> = e
                .attribut("id")
                .map(|i| format!("#{i}"))
                .into_iter()
                .chain(
                    e.attribut("class")
                        .unwrap_or("")
                        .split_whitespace()
                        .map(|c| format!(".{c}")),
                )
                .collect();
            let geo = self.geo.clone();
            boite = boite.relative().child(
                canvas(
                    move |bornes, _, _| {
                        let mut g = geo.borrow_mut();
                        for c in &cles {
                            g.elements.insert(c.clone(), bornes);
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        }
        Some(boite.into_any_element())
    }

    fn details(
        &mut self,
        e: &Element,
        chemin: &mut Vec<usize>,
        ctx: &Ctx,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ouvert = self.ouverts.contains(chemin.as_slice());
        let mut resume = Paragraphe::default();
        resume.pousser(if ouvert { "▾ " } else { "▸ " }, HighlightStyle::default());
        let mut reste = Vec::new();
        let mut a_un_resume = false;
        for (i, n) in e.enfants.iter().enumerate() {
            match n {
                Noeud::Element(s) if !a_un_resume && s.nom_bas() == "summary" => {
                    a_un_resume = true;
                    paragraphe_en_ligne(&s.enfants, ctx, HighlightStyle::default(), &mut resume);
                }
                _ => reste.push((i, n.clone())),
            }
        }
        if !a_un_resume {
            // Comme le navigateur : un `<details>` sans résumé dit « Détails ».
            resume.pousser("Détails", HighlightStyle::default());
        }
        let cle = chemin.clone();
        let mut d = div().flex().flex_col().child(
            div()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |b, _, _, cx| {
                        cx.stop_propagation();
                        b.basculer(cle.clone(), cx);
                    }),
                )
                .child(resume.element()),
        );
        if ouvert {
            let noeuds: Vec<Noeud> = reste.iter().map(|(_, n)| n.clone()).collect();
            // Le chemin d'un enfant de `details` reste celui de l'arbre d'origine.
            let mut contenu = Vec::new();
            let mut p = Paragraphe::default();
            for ((i, _), n) in reste.iter().zip(&noeuds) {
                if est_en_ligne(n) {
                    paragraphe_en_ligne(
                        std::slice::from_ref(n),
                        ctx,
                        HighlightStyle::default(),
                        &mut p,
                    );
                    continue;
                }
                if !p.vide() {
                    contenu.push(std::mem::take(&mut p).element());
                }
                if let Noeud::Element(x) = n {
                    chemin.push(*i);
                    if let Some(el) = self.element_de_bloc(x, chemin, ctx, cx) {
                        contenu.push(el);
                    }
                    chemin.pop();
                }
            }
            if !p.vide() {
                contenu.push(p.element());
            }
            d = d.children(contenu);
        }
        d.into_any_element()
    }

    fn svg(&mut self, e: &Element, chemin: &[usize], ctx: &Ctx, densite: f32) -> AnyElement {
        let (l, h) = svg::taille(e);
        let anime = smil::anime(e);
        let source = svg::source_a(&ctx.theme, e, if anime { ctx.temps } else { 0.0 });
        let image = match self.images.get(chemin) {
            Some((s, d, i)) if *s == source && (*d - densite).abs() < f32::EPSILON => {
                Some(i.clone())
            }
            _ => svg::rasteriser(&source, densite).inspect(|i| {
                self.images
                    .insert(chemin.to_vec(), (source.clone(), densite, i.clone()));
            }),
        };
        let mut d = div().w(px(l)).h(px(h)).flex_none();
        if let Some(i) = image {
            d = d.child(img(ImageSource::Render(i)).w(px(l)).h(px(h)));
        }
        d.into_any_element()
    }
}

/// Pose le sous-ensemble CSS lu sur un `div`.
fn appliquer(mut d: gpui_kit::Div, s: &StyleCss) -> gpui_kit::Div {
    if let Some(c) = s.couleur {
        d = d.text_color(couleur(c));
    }
    if let Some(c) = s.fond {
        d = d.bg(couleur(c));
    }
    if let Some((l, c)) = s.bordure {
        let w: AbsoluteLength = px(l).into();
        d.style().border_widths = EdgesRefinement {
            top: Some(w),
            right: Some(w),
            bottom: Some(w),
            left: Some(w),
        };
        if let Some(c) = c.or(s.couleur) {
            d = d.border_color(couleur(c));
        }
    }
    if let Some([h, dr, b, g]) = s.padding {
        d = d.pt(px(h)).pr(px(dr)).pb(px(b)).pl(px(g));
    }
    if let Some(r) = s.rayon {
        d = d.rounded(px(r));
    }
    if let Some(g) = s.gras {
        d = d.font_weight(if g {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        });
    }
    if s.italique {
        d = d.italic();
    }
    if let Some(t) = s.taille {
        d = d.text_size(px(t));
    }
    if let Some(a) = s.alignement {
        d = d.text_align(match a {
            Alignement::Gauche => TextAlign::Left,
            Alignement::Centre => TextAlign::Center,
            Alignement::Droite => TextAlign::Right,
        });
    }
    if s.flex {
        d = d.flex();
        d = if s.colonne {
            d.flex_col()
        } else {
            d.flex_row()
        };
        if let Some(g) = s.gap {
            d = d.gap(px(g));
        }
    }
    if let Some(g) = s.flex_grow {
        d = d.flex_grow(g).flex_basis(px(0.));
    }
    d
}

/// La couleur d'un élément nommé du bloc, telle que le rendu l'applique (tests).
#[must_use]
pub fn couleur_appliquee(theme: &Theme, e: &Element, taille: f32) -> Option<Couleur> {
    e.attribut("style")
        .and_then(|s| css::lire(theme, s, taille).couleur)
}

impl Render for BlocHtml {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let f = theme.forme.clone();
        let ctx = Ctx {
            taille: crate::editeur::OptionsEditeur::default().taille_texte,
            densite: window.scale_factor(),
            temps: self.temps(),
            theme: theme.clone(),
        };
        self.geo.borrow_mut().elements.clear();
        let arbre = std::mem::take(&mut self.arbre);
        let contenu = self.traduire(&arbre, &mut Vec::new(), &ctx, cx);
        self.arbre = arbre;
        if self.anime() {
            self.programmer_animation(cx);
        }
        let (geo, hauteur) = (self.geo.clone(), self.hauteur.clone());
        let entite = cx.entity().downgrade();
        let mut boite = div()
            .id("cm-html-block")
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .occlude()
            .pt(px(f.size_2_1))
            .pr(px(f.size_2_1))
            .pb(px(f.size_2_1))
            .pl(px(f.size_2_3))
            .overflow_x_hidden()
            .text_color(couleur(theme.palette.text_normal))
            .on_hover(cx.listener(|b, dedans: &bool, _, cx| b.survoler(*dedans, cx)))
            .children(contenu)
            .child(
                canvas(
                    move |bornes, _, cx| {
                        geo.borrow_mut().boite = bornes;
                        let h = f32::from(bornes.size.height);
                        let avant = *hauteur.borrow();
                        if (avant - h).abs() > 0.01 {
                            *hauteur.borrow_mut() = h;
                            if let Some(e) = entite.upgrade() {
                                cx.defer(move |cx| {
                                    e.update(cx, |_, cx| cx.emit(EvenementBloc::Hauteur(h)));
                                });
                            }
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        if !self.lecture_seule {
            let cote = f.icon_s;
            let geo = self.geo.clone();
            boite = boite.child(
                div()
                    .absolute()
                    .top(px(BOUTON_DECALAGE))
                    .right(px(BOUTON_DECALAGE))
                    .flex()
                    .p(px(BOUTON_PADDING))
                    .rounded(px(f.radius_s))
                    .cursor_pointer()
                    .opacity(if self.survole {
                        BOUTON_OPACITE_SURVOL
                    } else {
                        0.0
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(EvenementBloc::Source);
                        }),
                    )
                    .child(
                        icone_svg()
                            .path(icones::chemin(ICONE_BOUTON))
                            .size(px(cote))
                            .text_color(couleur(theme.palette.text_muted)),
                    )
                    .child(
                        canvas(
                            move |bornes, _, _| geo.borrow_mut().bouton = Some(bornes),
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            );
        }
        boite
    }
}
