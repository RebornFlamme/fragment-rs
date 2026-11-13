// Port de l'hôte des widgets de bloc d'app/src/ui/views/markdown/decorationPlugin.ts (SeparatorWidget,
// LangBadgeWidget, MathBlockWidget et son `selectInside`, `markdownFolding` : hoverTracker,
// foldChevronGutter, wireFoldControl) et des fonds de `.cm-ob-codeblock*` (editor.css).
//! L'hôte des blocs : il ENVELOPPE un [`EditeurLive`] et lui ajoute ce que le fork ne sait pas faire
//! seul.
//!
//! - les rangées : un complément ([`super::complement`]) pose les blocs, le repli et les lignes de
//!   code dans le modèle que l'éditeur donne au fork ;
//! - sous le texte (l'éditeur est en `fond_transparent`) : le fond de la note, puis le fond des blocs
//!   de code, coins arrondis sur la première et la dernière ligne ;
//! - par-dessus : le trait des séparateurs, le badge de langue, le cadre des blocs rendus (maths,
//!   embed : cadre réservé) et les chevrons de la gouttière de repli ;
//! - la souris : survol de ligne (chevron visible), clic sur un chevron (repli), clic sur un bloc
//!   de maths rendu (sélection du LaTeX, comme `selectInside`).
//!
//! Un bloc est ATOMIQUE par construction du fork : « à la place » = une rangée, un arrêt ;
//! « dessous » = de la hauteur ajoutée, que Haut/Bas traversent d'un coup et qu'un clic renvoie à la
//! ligne suivante (ADR-006).
//!
//! Repère : comme `SurfaceLive`, le repère DOCUMENT a son origine au coin haut-gauche du contenu qui
//! défile ; sa position dans la fenêtre est DÉDUITE d'une ligne visible (`range_to_bounds` du fork
//! contre `haut_de_ligne`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_markdown::blocs::{BlocCalcule, ContenuBloc, ModeBloc};
use fragment_markdown::decorations::GenreWidget;
use fragment_markdown::syntaxe::modele_rangees::ModeleRangees;
use fragment_maths::moteur::Moteur;
use fragment_text::editor::EditorChange;
use fragment_text::utf16::{byte_to_utf16, utf16_to_byte};
use fragment_theme::{Aplat, Couleur, Theme};
use gpui_kit::{
    App, AppContext as _, Bounds, Context, Corners, Entity, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, Styled as _, Subscription, TextAlign, TextRun, Window, canvas, div, fill, font,
    point, px, size,
};

use super::complement::{Entree, completer};
use super::geometrie::{
    ESPACEMENT_BADGE_EM, PADDING_BADGE_EM, PADDING_CLOTURE_CODE_EM, corps_badge, corps_code,
    interligne,
};
use super::maths::{MathsBlocs, Strut};
use crate::editeur::constantes::{MARGE_LIGNE_DROITE, MARGE_LIGNE_GAUCHE, debut_texte, fin_texte};
use crate::editeur::{ComplementRangees, EditeurLive, EntreeComplement, OptionsEditeur};
use crate::icones;
use crate::md_repli::gouttiere::{boite_chevron, dans_la_gouttiere};
use crate::md_repli::{Chevron, Repli, gouttiere};
use crate::polices::{FAMILLE_INTERFACE, FAMILLE_MONOSPACE};
use crate::theme_global::{couleur, theme_ou_defaut};

/// La hauteur du rendu d'un bloc, en pixels logiques, ou `None` s'il n'est pas (encore) mesuré.
///
/// Les maths viennent de `fragment-maths` (ADR-011), un embed de T9, un tableau, les Properties et le
/// HTML de leurs tâches : chacun fournit sa mesure à l'hôte.
pub type Mesureur = Rc<dyn Fn(&BlocCalcule, f32) -> Option<f32>>;

/// L'unité de mise en page de Chromium (`LayoutUnit`, 1/64 px) : en dessous, une rangée est nulle.
const UNITE_MISE_EN_PAGE: f32 = 1.0 / 64.0;

/// L'intervalle de relecture du cache des maths pendant que le moteur travaille.
const RELECTURE_MATHS_MS: u64 = 30;

fn rgb(c: Couleur) -> [u8; 3] {
    [c.r, c.g, c.b]
}

/// Ce que l'hôte partage avec le complément de rangées (appelé par l'éditeur, pendant sa projection).
struct Partage {
    repli: Repli,
    /// Le texte que connaît `repli` (pour remapper ses plis au changement suivant).
    texte: String,
    /// Les blocs de la dernière projection.
    blocs: Vec<BlocCalcule>,
    mesureur: Option<Mesureur>,
    taille_texte: f32,
    /// Les maths en bloc, une fois le moteur posé ([`HoteBlocs::poser_moteur_maths`]) : elles
    /// l'emportent sur le `mesureur` pour les blocs `$$`.
    maths: Option<Rc<MathsBlocs>>,
    /// `--border-width` : la bordure transparente de `.cm-math-block`.
    bordure: f32,
    /// Le chevron sous la souris : l'hôte ne libère que le contrôle qu'il a lui-même posé (une puce
    /// liée pose le sien par `Repli::survoler_controle`).
    chevron_survole: Option<usize>,
    /// Un complément de plus, appliqué après celui des blocs (les balises HTML en ligne masquées,
    /// `fr-t3-html-rendu`) : [`HoteBlocs::poser_complement_aval`].
    aval: Option<ComplementRangees>,
}

/// Une ligne logique telle qu'elle est posée à l'écran (repère FENÊTRE).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LigneEcran {
    pub ligne: usize,
    /// Haut de la première rangée.
    pub haut: f32,
    /// Hauteur des rangées de texte (sans le bloc posé dessous).
    pub hauteur: f32,
    /// Hauteur du bloc posé sous la ligne (`Placement::Dessous`), 0 sinon.
    pub bloc_dessous: f32,
}

/// La géométrie de la dernière image, relue par les écouteurs de souris et les tests.
#[derive(Debug, Clone, Default)]
pub struct Geometrie {
    /// Bord gauche de l'éditeur (fenêtre) et sa largeur.
    pub gauche: f32,
    pub largeur: f32,
    pub haut: f32,
    pub hauteur: f32,
    /// Les lignes visibles, par ligne croissante (les lignes repliées n'y sont pas).
    pub lignes: Vec<LigneEcran>,
    /// Les chevrons dessinés et leur boîte (gauche, haut, côté).
    pub chevrons: Vec<(Chevron, (f32, f32, f32))>,
    /// Les blocs de maths rendus (à la place), leur boîte et le LaTeX à sélectionner (UTF-16).
    pub maths: Vec<(Bounds<Pixels>, usize, usize)>,
}

impl Geometrie {
    /// La ligne sous l'ordonnée FENÊTRE `y` (`lineBlockAtHeight`) : un bloc dessous appartient à sa
    /// ligne.
    #[must_use]
    pub fn ligne_a(&self, y: f32) -> Option<usize> {
        let i = self.lignes.partition_point(|l| l.haut <= y);
        let l = self.lignes.get(i.checked_sub(1)?)?;
        (y < l.haut + l.hauteur + l.bloc_dessous || i == self.lignes.len()).then_some(l.ligne)
    }

    #[must_use]
    pub fn ligne(&self, n: usize) -> Option<&LigneEcran> {
        self.lignes
            .binary_search_by_key(&n, |l| l.ligne)
            .ok()
            .map(|i| &self.lignes[i])
    }
}

/// L'hôte des blocs (une entité GPUI) : l'éditeur et ses widgets de bloc.
pub struct HoteBlocs {
    editeur: Entity<EditeurLive>,
    partage: Rc<RefCell<Partage>>,
    geometrie: Rc<RefCell<Geometrie>>,
    /// Une relecture du cache des maths est en cours.
    relit_maths: Rc<Cell<bool>>,
    _abonnements: Vec<Subscription>,
}

impl HoteBlocs {
    /// Un éditeur de note sur `texte`, avec ses blocs. `options.fond_transparent` est forcé : c'est
    /// l'hôte qui peint le fond, sous les fonds de code.
    pub fn new(
        texte: &str,
        options: OptionsEditeur,
        mesureur: Option<Mesureur>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = OptionsEditeur {
            fond_transparent: true,
            ..options
        };
        let editeur = cx.new(|cx| EditeurLive::new(texte, options, window, cx));
        let arbre = fragment_markdown::syntaxe::Arbre::analyser(texte);
        let partage = Rc::new(RefCell::new(Partage {
            repli: Repli::new(texte, &arbre),
            texte: texte.to_owned(),
            blocs: Vec::new(),
            mesureur,
            taille_texte: options.taille_texte,
            maths: None,
            bordure: 0.0,
            chevron_survole: None,
            aval: None,
        }));
        let p = partage.clone();
        let complement: ComplementRangees =
            Rc::new(move |e: &EntreeComplement<'_>, m: &mut ModeleRangees| {
                let mut p = p.borrow_mut();
                if p.texte != e.texte {
                    let ancien = std::mem::take(&mut p.texte);
                    p.repli.apres_changement(&ancien, e.texte, e.arbre);
                    e.texte.clone_into(&mut p.texte);
                }
                let mesureur = p.mesureur.clone();
                let taille = p.taille_texte;
                let (maths, bordure) = (p.maths.clone(), p.bordure);
                let mesure = |b: &BlocCalcule| {
                    if let (ContenuBloc::Maths { tex }, Some(m)) = (&b.contenu, &maths) {
                        return m.hauteur(tex, taille, bordure).unwrap_or(0.0);
                    }
                    mesureur.as_ref().and_then(|f| f(b, taille)).unwrap_or(0.0)
                };
                let entree = Entree {
                    texte: e.texte,
                    arbre: e.arbre,
                    lignes: e.lignes,
                    curseur: e.curseur,
                    ctx: e.ctx,
                    repli: p.repli.etat(),
                    analyse: p.repli.analyse(),
                };
                let blocs = completer(&entree, &mesure, m);
                p.blocs = blocs;
                let aval = p.aval.clone();
                drop(p);
                if let Some(f) = aval {
                    f(e, m);
                }
            });
        editeur.update(cx, |e, cx| e.poser_complement(Some(complement), cx));
        let abonnements = vec![cx.subscribe(&editeur, Self::sur_changement)];
        HoteBlocs {
            editeur,
            partage,
            geometrie: Rc::new(RefCell::new(Geometrie::default())),
            relit_maths: Rc::new(Cell::new(false)),
            _abonnements: abonnements,
        }
    }

    /// Pose le moteur de maths partagé (ADR-011) : les blocs `$$` prennent leur vraie hauteur et
    /// leur image. Le strut est lu sur DM Sans au corps du texte.
    pub fn poser_moteur_maths(&mut self, moteur: Rc<Moteur>, cx: &mut Context<Self>) {
        let theme = theme_ou_defaut(cx);
        let taille = self.partage.borrow().taille_texte;
        let ts = cx.text_system().clone();
        let police = ts.resolve_font(&font(FAMILLE_INTERFACE));
        let strut = Strut {
            ascent: f32::from(ts.ascent(police, px(taille))).round(),
            descent: f32::from(ts.descent(police, px(taille))).round(),
        };
        let maths = Rc::new(MathsBlocs::new(
            moteur,
            strut,
            rgb(theme.palette.text_normal),
        ));
        {
            let mut p = self.partage.borrow_mut();
            p.maths = Some(maths);
            p.bordure = theme.forme.border_width;
        }
        self.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
        self.relire_maths(cx);
        cx.notify();
    }

    /// Pose (ou retire) un complément de rangées APPLIQUÉ APRÈS celui des blocs, puis reprojette.
    /// Ajout de `fr-t3-html-rendu` : les balises HTML en ligne y sont masquées hors curseur.
    pub fn poser_complement_aval(&mut self, c: Option<ComplementRangees>, cx: &mut Context<Self>) {
        self.partage.borrow_mut().aval = c;
        self.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
        cx.notify();
    }

    /// Les maths en bloc posées, s'il y en a.
    #[must_use]
    pub fn maths(&self) -> Option<Rc<MathsBlocs>> {
        self.partage.borrow().maths.clone()
    }

    /// Une formule manquait : relit le cache du moteur jusqu'à ce qu'il ait tout rendu, puis
    /// reprojette (les hauteurs changent) et redessine.
    fn relire_maths(&mut self, cx: &mut Context<Self>) {
        let Some(maths) = self.maths() else { return };
        if !maths.prendre_manque() || self.relit_maths.replace(true) {
            return;
        }
        let relit = self.relit_maths.clone();
        cx.spawn(async move |hote, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(RELECTURE_MATHS_MS))
                    .await;
                if maths.moteur().en_attente() > 0 {
                    continue;
                }
                relit.set(false);
                let _ = hote.update(cx, |h, cx| {
                    h.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
                    // La reprojection a pu demander d'autres formules.
                    h.relire_maths(cx);
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    /// L'éditeur enveloppé.
    #[must_use]
    pub fn editeur(&self) -> &Entity<EditeurLive> {
        &self.editeur
    }

    /// La géométrie de la dernière image (repère fenêtre).
    #[must_use]
    pub fn geometrie(&self) -> Geometrie {
        self.geometrie.borrow().clone()
    }

    /// Les blocs de la dernière projection.
    #[must_use]
    pub fn blocs(&self) -> Vec<BlocCalcule> {
        self.partage.borrow().blocs.clone()
    }

    /// La ligne `ligne` est-elle repliée ?
    #[must_use]
    pub fn ligne_repliee(&self, ligne: usize) -> bool {
        let p = self.partage.borrow();
        p.repli.etat().ligne_repliee(p.repli.analyse(), ligne)
    }

    /// L'interaction partagée de `ligne` (`foldCtl`) : la puce liée d'une liste la reflète.
    #[must_use]
    pub fn interaction(&self, ligne: usize) -> crate::md_repli::Interaction {
        self.partage.borrow().repli.interaction(ligne)
    }

    /// La souris entre sur la puce liée de `ligne` (`None` : elle en sort), comme sur le chevron.
    pub fn survoler_controle(&mut self, ligne: Option<usize>, cx: &mut Context<Self>) {
        if self.partage.borrow_mut().repli.survoler_controle(ligne) {
            cx.notify();
        }
    }

    /// Bascule le repli de `ligne` (le clic sur son chevron, `toggleFoldAt`), puis chasse le curseur
    /// hors des plis.
    pub fn basculer_repli(&mut self, ligne: usize, cx: &mut Context<Self>) {
        let change = self.partage.borrow_mut().repli.basculer(ligne);
        if change {
            self.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
            self.chasser_sans_fenetre(cx);
            cx.notify();
        }
    }

    fn sur_changement(
        &mut self,
        _e: Entity<EditeurLive>,
        ch: &EditorChange,
        cx: &mut Context<Self>,
    ) {
        if ch.selection_changed || ch.doc_changed {
            self.chasser_sans_fenetre(cx);
        }
        cx.notify();
    }

    /// `transactionFilter` : une sélection VIDE dans un pli ou dans la source des Properties remonte
    /// au bout de la ligne parent (ou passe sous le bloc).
    fn chasser_sans_fenetre(&mut self, cx: &mut Context<Self>) {
        let etat = self.editeur.read(cx).etat().clone();
        let (texte, sel) = {
            let s = etat.read(cx);
            (s.value().to_string(), s.selected_range())
        };
        if sel.start != sel.end {
            return;
        }
        let pos = byte_to_utf16(&texte, sel.start);
        let cible = self.partage.borrow().repli.chasser(&texte, pos);
        if cible != pos {
            let o = utf16_to_byte(&texte, cible).unwrap_or(texte.len());
            etat.update(cx, |s, cx| s.set_selected_range(o..o, cx));
        }
    }

    /// Relit, pour l'image, la position de chaque ligne visible et des chevrons.
    fn mesurer(&self, bornes: Bounds<Pixels>, theme: &Theme, cx: &App) -> Geometrie {
        let editeur = self.editeur.read(cx);
        let etat = editeur.etat().read(cx);
        let surface = editeur.surface(cx);
        let largeur = f32::from(bornes.size.width);
        let mut g = Geometrie {
            gauche: f32::from(bornes.origin.x),
            largeur,
            haut: f32::from(bornes.origin.y),
            hauteur: f32::from(bornes.size.height),
            ..Geometrie::default()
        };
        let lignes = editeur.lignes();
        let Some(visibles) = etat.visible_row_range() else {
            return g;
        };
        // ★ `visible_row_range` du fork rend des LIGNES logiques, pas des rangées d'affichage.
        if lignes.is_empty() {
            return g;
        }
        let premiere = visibles.start.min(lignes.len() - 1);
        let derniere = visibles
            .end
            .saturating_sub(1)
            .clamp(premiere, lignes.len() - 1);
        // L'origine du repère document dans la fenêtre : déduite d'une ligne posée par le fork.
        let Some(origine_y) = (premiere..=derniere).find_map(|l| {
            let b = etat.range_to_bounds(&(lignes[l].octets.from..lignes[l].octets.from))?;
            Some(f32::from(b.origin.y) - surface.haut_de_ligne(l))
        }) else {
            return g;
        };
        let fork = editeur.arguments_fork();
        let bloc_de = |l: usize| {
            fork.and_then(|f| {
                f.blocs
                    .binary_search_by_key(&l, |&(n, _)| n)
                    .ok()
                    .map(|i| f.blocs[i].1)
            })
            .unwrap_or(0.0)
        };
        let fin = (derniere + 1).min(lignes.len());
        let mut haut = surface.haut_de_ligne(premiere);
        for l in premiere..fin {
            let suivant = if l + 1 < lignes.len() {
                surface.haut_de_ligne(l + 1)
            } else {
                haut + editeur.interligne() + bloc_de(l)
            };
            let bloc = bloc_de(l);
            // ★ Depuis l'interligne exact (22,4 px, ADR-015), `haut_de_ligne` cumule des flottants
            //   non représentables : une rangée de hauteur 0 ressort à ~1,5e-5 px. Sous l'unité de
            //   mise en page de Chromium (1/64 px), une rangée est nulle.
            let hauteur = (suivant - haut - bloc).max(0.0);
            let hauteur = if hauteur < UNITE_MISE_EN_PAGE {
                0.0
            } else {
                hauteur
            };
            if hauteur > 0.0 || bloc > 0.0 {
                g.lignes.push(LigneEcran {
                    ligne: l,
                    haut: origine_y + haut,
                    hauteur,
                    bloc_dessous: bloc,
                });
            }
            haut = suivant;
        }
        let p = self.partage.borrow();
        let icone = theme.forme.icon_s;
        for le in &g.lignes {
            if let Some(c) = p.repli.chevron(le.ligne) {
                let (x, y, cote) = boite_chevron(largeur, le.haut, le.hauteur, icone);
                g.chevrons.push((c, (g.gauche + x, y, cote)));
            }
        }
        let gauche_texte = g.gauche + debut_texte(largeur) - MARGE_LIGNE_GAUCHE;
        let droite_texte = g.gauche + largeur - fin_texte(largeur) + MARGE_LIGNE_DROITE;
        let texte = editeur.texte();
        for b in &p.blocs {
            if let ContenuBloc::Maths { tex } = &b.contenu
                && b.mode == ModeBloc::Remplace
                && let Some(le) = g.ligne(b.premiere_ligne)
            {
                let source = b.source.texte(texte);
                let debut = source.find("$$").map_or(0, |i| i + 2);
                let de = b.source.utf16.from + byte_to_utf16(source, debut);
                let a = de + fragment_text::utf16::utf16_len(tex);
                let boite = Bounds {
                    origin: point(px(gauche_texte), px(le.haut)),
                    size: size(px(droite_texte - gauche_texte), px(le.hauteur)),
                };
                g.maths.push((boite, de, a));
            }
        }
        g
    }

    /// Peint SOUS le texte : le fond de la note, puis les fonds de code.
    fn peindre_dessous(
        &self,
        bornes: Bounds<Pixels>,
        g: &Geometrie,
        theme: &Theme,
        window: &mut Window,
    ) {
        let p = &theme.palette;
        window.paint_quad(fill(bornes, couleur(p.background_primary)));
        let fond = couleur(p.aplat(Aplat::Survol, p.background_primary));
        let rayon = px(theme.forme.radius_sm);
        let gauche = g.gauche + debut_texte(g.largeur) - MARGE_LIGNE_GAUCHE;
        let droite = g.gauche + g.largeur - fin_texte(g.largeur) + MARGE_LIGNE_DROITE;
        let partage = self.partage.borrow();
        for b in &partage.blocs {
            if !matches!(b.contenu, ContenuBloc::Code { .. }) {
                continue;
            }
            let visibles: Vec<_> = (b.premiere_ligne..=b.derniere_ligne)
                .filter_map(|l| g.ligne(l))
                .collect();
            let (Some(a), Some(z)) = (visibles.first(), visibles.last()) else {
                continue;
            };
            let boite = Bounds {
                origin: point(px(gauche), px(a.haut)),
                size: size(px(droite - gauche), px(z.haut + z.hauteur - a.haut)),
            };
            let coins = Corners {
                top_left: if a.ligne == b.premiere_ligne {
                    rayon
                } else {
                    px(0.)
                },
                top_right: if a.ligne == b.premiere_ligne {
                    rayon
                } else {
                    px(0.)
                },
                bottom_left: if z.ligne == b.derniere_ligne {
                    rayon
                } else {
                    px(0.)
                },
                bottom_right: if z.ligne == b.derniere_ligne {
                    rayon
                } else {
                    px(0.)
                },
            };
            window.paint_quad(fill(boite, fond).corner_radii(coins));
        }
    }

    /// Peint PAR-DESSUS le texte : séparateurs, badges de langue.
    /// Ce que la peinture du dessus lit de l'hôte : les blocs, le corps du texte, les lignes
    /// d'ouverture qui portent un badge de langue (hors curseur seulement : la ligne est alors
    /// vidée de ses accents graves).
    fn donnees_dessus(&self, cx: &App) -> Dessus {
        let partage = self.partage.borrow();
        let editeur = self.editeur.read(cx);
        let lignes = editeur.lignes();
        let badges = partage
            .blocs
            .iter()
            .filter(|b| {
                editeur.projection().decorations.widgets.iter().any(|w| {
                    matches!(w.genre, GenreWidget::BadgeLangue { .. })
                        && lignes
                            .get(b.premiere_ligne)
                            .is_some_and(|l| l.octets.from == w.source.octets.from)
                })
            })
            .map(|b| b.premiere_ligne)
            .collect();
        Dessus {
            blocs: partage.blocs.clone(),
            taille: partage.taille_texte,
            badges,
            maths: partage.maths.clone(),
            bordure: partage.bordure,
        }
    }
}

/// Ce que lit la peinture du dessus (voir [`HoteBlocs::donnees_dessus`]).
struct Dessus {
    blocs: Vec<BlocCalcule>,
    taille: f32,
    badges: Vec<usize>,
    maths: Option<Rc<MathsBlocs>>,
    bordure: f32,
}

/// Peint PAR-DESSUS le texte : séparateurs, badges de langue.
fn peindre_dessus(
    Dessus {
        blocs,
        taille,
        badges,
        maths,
        bordure,
    }: &Dessus,
    g: &Geometrie,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    {
        let p = &theme.palette;
        let taille = *taille;
        let gauche = g.gauche + debut_texte(g.largeur);
        let droite = g.gauche + g.largeur - fin_texte(g.largeur);
        let ts = window.text_system().clone();
        let interface = ts.resolve_font(&font(FAMILLE_INTERFACE));
        let trait_couleur = couleur(p.aplat(Aplat::BordureSurvol, p.background_primary));
        for b in blocs {
            match &b.contenu {
                ContenuBloc::Separateur if b.mode == ModeBloc::Remplace => {
                    let Some(le) = g.ligne(b.premiere_ligne) else {
                        continue;
                    };
                    // `.cm-hr` : boîte en ligne de 0,8 em + 1 px, `vertical-align: middle` → son
                    // milieu est à x-height / 2 au-dessus de la ligne de base ; le trait (bordure
                    // haute) est à 0,4 em sous le haut de la boîte, soit 0,5 px au-dessus du milieu.
                    let (asc, desc) = (
                        f32::from(ts.ascent(interface, px(taille))),
                        f32::from(ts.descent(interface, px(taille))),
                    );
                    let xh = f32::from(ts.x_height(interface, px(taille)));
                    let base = le.haut + (interligne(taille) - (asc + desc)) / 2.0 + asc;
                    let epaisseur = theme.forme.border_width;
                    let y = (base - xh / 2.0 - epaisseur / 2.0).round();
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(px(gauche), px(y)),
                            size: size(px(droite - gauche), px(epaisseur)),
                        },
                        trait_couleur,
                    ));
                }
                ContenuBloc::Code { langue } if !langue.is_empty() => {
                    let Some(le) = g.ligne(b.premiere_ligne) else {
                        continue;
                    };
                    if !badges.contains(&b.premiere_ligne) {
                        continue;
                    }
                    let corps = corps_badge(taille);
                    let texte: SharedString = langue.to_uppercase().into();
                    let run = TextRun {
                        len: texte.len(),
                        font: font(FAMILLE_MONOSPACE),
                        color: couleur(p.text_faint).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    // ★ GPUI n'applique pas `letter-spacing` à la mise en forme : l'écart (0,06 em
                    //   par lettre) est porté dans la largeur, une dette de pixels suivie.
                    let ligne = ts.shape_line(texte.clone(), px(corps), &[run], None);
                    #[allow(clippy::cast_precision_loss)]
                    let espacement = ESPACEMENT_BADGE_EM * corps * texte.chars().count() as f32;
                    let largeur = f32::from(ligne.width) + espacement;
                    let x = droite - PADDING_BADGE_EM[1] * corps - largeur;
                    let y = le.haut
                        + PADDING_CLOTURE_CODE_EM * corps_code(taille)
                        + PADDING_BADGE_EM[0] * corps;
                    let _ = ligne.paint(
                        point(px(x), px(y)),
                        px(interligne(corps)),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
                ContenuBloc::Maths { tex } => {
                    let Some(m) = maths else { continue };
                    // À la place : la rangée de la première ligne ; curseur dedans : le bloc posé
                    // sous la dernière ligne de la source.
                    let boite = match b.mode {
                        ModeBloc::Remplace => {
                            g.ligne(b.premiere_ligne).map(|le| (le.haut, le.hauteur))
                        }
                        ModeBloc::SourceEtRendu => g
                            .ligne(b.derniere_ligne)
                            .map(|le| (le.haut + le.hauteur, le.bloc_dessous)),
                        ModeBloc::SourceSeule => None,
                    };
                    let Some((haut, hauteur)) = boite.filter(|&(_, h)| h > 0.0) else {
                        continue;
                    };
                    let gauche = gauche - MARGE_LIGNE_GAUCHE;
                    let boite = Bounds {
                        origin: point(px(gauche), px(haut)),
                        size: size(px(droite + MARGE_LIGNE_DROITE - gauche), px(hauteur)),
                    };
                    m.peindre(tex, taille, *bordure, boite, window, cx);
                }
                _ => {}
            }
        }
    }
}

impl Render for HoteBlocs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        if let Some(m) = self.maths() {
            // La couleur du texte fait partie de la clé du rendu : un changement de thème redemande
            // les formules (à la prochaine projection, après cette image).
            if m.poser_couleur(rgb(theme.palette.text_normal)) {
                let (e, h) = (self.editeur.clone(), cx.entity().downgrade());
                cx.defer(move |cx| {
                    e.update(cx, |e, cx| e.reprojeter_maintenant(cx));
                    let _ = h.update(cx, |_, cx| cx.notify());
                });
            }
            // Une frappe a pu faire demander une formule : relire le cache jusqu'à son arrivée.
            self.relire_maths(cx);
        }
        let entite = cx.entity();
        let geometrie = self.geometrie.clone();
        let chevrons: Vec<_> = geometrie.borrow().chevrons.clone();
        let id = cx.entity_id();
        let (e1, e2, t1, t2) = (entite.clone(), entite.clone(), theme.clone(), theme.clone());
        let g1 = geometrie.clone();
        div()
            .relative()
            .size_full()
            // Sous le texte : mesure de l'image, fond, fonds de code.
            .child(
                canvas(
                    move |bornes, _, cx| {
                        let g = e1.read(cx).mesurer(bornes, &t1, cx);
                        let change = g.chevrons != g1.borrow().chevrons;
                        *g1.borrow_mut() = g;
                        if change {
                            // Les chevrons sont des éléments : une image de plus pour les poser.
                            cx.notify(id);
                        }
                        bornes
                    },
                    move |_, bornes, window, cx| {
                        let g = geometrie.borrow().clone();
                        e2.read(cx).peindre_dessous(bornes, &g, &t2, window);
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(self.editeur.clone())
            // Par-dessus : séparateurs, badges ; et les écouteurs de souris de l'hôte.
            .child({
                let (e3, t3, g3) = (entite.clone(), theme.clone(), self.geometrie.clone());
                canvas(
                    |_, _, _| {},
                    move |_, (), window, cx| {
                        let g = g3.borrow().clone();
                        let d = e3.read(cx).donnees_dessus(cx);
                        peindre_dessus(&d, &g, &t3, window, cx);
                        ecouter_souris(&e3, &g3, window);
                    },
                )
                .absolute()
                .size_full()
            })
            .children(chevrons.into_iter().map(|(c, (x, y, cote))| {
                let g = self.geometrie.borrow();
                let (gx, gy) = (g.gauche, g.haut);
                div()
                    .absolute()
                    .left(px(x - gx))
                    .top(px(y - gy))
                    .size(px(cote))
                    .child(
                        crate::icone::icone_image()
                            .path(icones::chemin(gouttiere::icone(&c)))
                            .size(px(cote))
                            .text_color(couleur(gouttiere::couleur(&theme, &c))),
                    )
            }))
    }
}

/// Les écouteurs de la souris (posés à la peinture, rejoués à chaque image) : survol des lignes,
/// chevrons, clic sur un bloc de maths rendu.
fn ecouter_souris(
    hote: &Entity<HoteBlocs>,
    geometrie: &Rc<RefCell<Geometrie>>,
    window: &mut Window,
) {
    let (h, g) = (hote.clone(), geometrie.clone());
    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _window, cx| {
        if !phase.capture() {
            return;
        }
        let geo = g.borrow().clone();
        let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
        let dedans = x >= geo.gauche
            && x < geo.gauche + geo.largeur
            && y >= geo.haut
            && y < geo.haut + geo.hauteur;
        let ligne = if dedans { geo.ligne_a(y) } else { None };
        let sur_chevron = geo
            .chevrons
            .iter()
            .find(|(_, b)| dans_boite(*b, ev.position))
            .map(|(c, _)| c.ligne);
        h.update(cx, |hote, cx| {
            let mut p = hote.partage.borrow_mut();
            let mut change = p.repli.survoler_ligne(ligne);
            if p.chevron_survole != sur_chevron {
                p.chevron_survole = sur_chevron;
                change |= p.repli.survoler_controle(sur_chevron);
            }
            drop(p);
            if change {
                cx.notify();
            }
        });
    });
    let (h, g) = (hote.clone(), geometrie.clone());
    window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
        if !phase.capture() || ev.button != MouseButton::Left {
            return;
        }
        let geo = g.borrow().clone();
        let x = f32::from(ev.position.x) - geo.gauche;
        let y = f32::from(ev.position.y);
        if y < geo.haut || y >= geo.haut + geo.hauteur {
            return;
        }
        if dans_la_gouttiere(geo.largeur, x) {
            // La gouttière n'est pas du texte : un clic n'y déplace jamais le curseur.
            cx.stop_propagation();
            if let Some((c, _)) = geo
                .chevrons
                .iter()
                .find(|(_, b)| dans_boite(*b, ev.position))
            {
                let ligne = c.ligne;
                h.update(cx, |hote, cx| hote.basculer_repli(ligne, cx));
            }
            return;
        }
        if let Some(&(_, de, a)) = geo.maths.iter().find(|(b, _, _)| b.contains(&ev.position)) {
            // `selectInside` : le LaTeX entre les `$$`, et le focus.
            cx.stop_propagation();
            h.update(cx, |hote, cx| {
                let etat = hote.editeur.read(cx).etat().clone();
                etat.update(cx, |s, cx| {
                    let t = s.value().to_string();
                    let (o1, o2) = (
                        utf16_to_byte(&t, de).unwrap_or(t.len()),
                        utf16_to_byte(&t, a).unwrap_or(t.len()),
                    );
                    s.focus(window, cx);
                    s.set_selected_range(o1..o2, cx);
                });
            });
        }
    });
    let h = hote.clone();
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
        if phase.bubble() {
            h.update(cx, |hote, cx| {
                if hote.partage.borrow_mut().repli.relacher() {
                    cx.notify();
                }
            });
        }
    });
}

fn dans_boite((x, y, c): (f32, f32, f32), p: Point<Pixels>) -> bool {
    let (px_, py) = (f32::from(p.x), f32::from(p.y));
    px_ >= x && px_ < x + c && py >= y && py < y + c
}
