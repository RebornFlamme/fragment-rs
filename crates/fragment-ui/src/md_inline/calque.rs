// Port de la peinture des widgets de app/src/ui/views/markdown/decorationPlugin.ts (CheckboxWidget,
// BulletWidget, IndentBarWidget, MathInlineWidget) et d'editor.css (`.cm-ob-bullet`,
// `.cm-task-checkbox`, `.cm-indent-line`, `.cm-math-inline`).
//! Le calque des widgets en ligne : peint par-dessus l'éditeur, à la place de la source des
//! plages que le fork a réservées (largeur dictée, ADR-006), et capte leurs clics.
//!
//! ★ Le calque se peint dans le `canvas` d'un enfant de l'éditeur, APRÈS le texte : la boîte d'une
//!   plage (`range_to_bounds`) y est celle de l'image EN COURS, pas de la précédente (un calque
//!   `div` construit au `render` suivait le texte avec une image de retard au défilement).
//! ★ Un clic sur un widget est capté en phase de CAPTURE et arrêté : l'éditeur ne déplace pas son
//!   curseur (Fragment fait `preventDefault` sur le `mousedown`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use fragment_markdown::decorations::{GenreLigne, GenreWidget};
use fragment_maths::moteur::{Cle, Moteur};
use fragment_maths::raster::vers_bitmap;
use fragment_theme::{Couleur, Theme};
use gpui_kit::base::input::EditorState;
use gpui_kit::{
    App, Bounds, Corners, DispatchPhase, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, RenderImage, Styled as _, Window, canvas, fill, point,
    px, size,
};

use super::geometrie::{
    ANNEAU_PUCE_EM, COTE_CASE_EM, CouleurPuce, CtlPuce, POINT_PUCE_EM, TRAIT_INDENTATION,
    aspect_puce, bascule_case, largeurs_des_widgets, selection_maths,
};
use crate::editeur::EditeurLive;
use crate::editeur::element::Calque;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Les cartes d'images de formules gardées au plus (une image par formule, corps et échelle).
const IMAGES_MAX: usize = 256;
/// La capacité du cache de rendus du moteur.
pub const CAPACITE_MOTEUR: usize = 512;
/// Délai entre deux relectures du moteur pendant qu'il travaille (ms) : son fil ne peut pas réveiller
/// l'interface.
const RELECTURE_MS: u64 = 30;

/// Bascule le repli d'une ligne (début UTF-16).
pub type FnBasculer = dyn Fn(usize, &mut Window, &mut App);

/// Ce que l'hôte sait du repli : la puce s'y relie (survol partagé, couleur, clic).
#[derive(Clone)]
pub struct LiaisonRepli {
    /// La ligne (début, UTF-16) porte-t-elle un repli ?
    pub repliable: Rc<dyn Fn(usize) -> bool>,
    /// La section sous cette ligne est-elle repliée ?
    pub replie: Rc<dyn Fn(usize) -> bool>,
    /// Bascule le repli de cette ligne.
    pub basculer: Rc<FnBasculer>,
}

/// L'état des widgets en ligne d'un éditeur.
pub struct MdInline {
    moteur: Rc<Moteur>,
    repli: Option<LiaisonRepli>,
    /// La puce survolée ou pressée (début de ligne UTF-16, état) : partagée avec le chevron du repli.
    puce: Cell<Option<(usize, CtlPuce)>>,
    images: RefCell<HashMap<(Cle, u32), Arc<RenderImage>>>,
    /// Une relecture du moteur est déjà en route.
    relit: Cell<bool>,
}

impl MdInline {
    /// Un état sans calque installé (tests du fork : les largeurs se posent à la main).
    #[must_use]
    pub fn inerte(moteur: Rc<Moteur>) -> Self {
        MdInline {
            moteur,
            repli: None,
            puce: Cell::new(None),
            images: RefCell::new(HashMap::new()),
            relit: Cell::new(false),
        }
    }

    /// Le survol de la puce posé par le chevron du repli (état partagé, `ctl`).
    pub fn poser_puce(&self, ligne: usize, ctl: CtlPuce) {
        self.puce.set(if ctl == CtlPuce::Aucun {
            None
        } else {
            Some((ligne, ctl))
        });
    }

    /// L'état de la puce de cette ligne.
    #[must_use]
    pub fn ctl_puce(&self, ligne: usize) -> CtlPuce {
        match self.puce.get() {
            Some((l, c)) if l == ligne => c,
            _ => CtlPuce::Aucun,
        }
    }

    /// Les images de formules en mémoire (pour les tests).
    #[must_use]
    pub fn images(&self) -> usize {
        self.images.borrow().len()
    }
}

/// La clé de rendu d'une formule : corps du texte, couleur `--text-normal`.
fn cle_maths(tex: &str, affichage: bool, taille: f32, theme: &Theme) -> Cle {
    let c: Couleur = theme.palette.text_normal;
    Cle::new(tex, affichage, f64::from(taille), [c.r, c.g, c.b])
}

/// Installe les widgets en ligne sur `editeur` : le calque, et le suivi des largeurs que les
/// widgets dictent au fork. Rend l'état partagé.
pub fn installer(
    editeur: &Entity<EditeurLive>,
    moteur: Rc<Moteur>,
    repli: Option<LiaisonRepli>,
    cx: &mut App,
) -> Rc<MdInline> {
    let etat = Rc::new(MdInline {
        moteur,
        repli,
        puce: Cell::new(None),
        images: RefCell::new(HashMap::new()),
        relit: Cell::new(false),
    });
    let e = etat.clone();
    let calque: Calque = Rc::new(move |ed, _window, _cx| peindre(&e, ed));
    editeur.update(cx, |ed, _| ed.set_calque(Some(calque)));
    // Les largeurs suivent la projection : à chaque notification de l'éditeur (texte, sélection,
    // thème), on les recalcule ; `poser_largeurs_widgets` ne fait rien si elles n'ont pas changé.
    let e = etat.clone();
    let faible = editeur.downgrade();
    cx.observe(editeur, move |ed, cx| {
        mettre_a_jour_largeurs(&e, &ed, faible.clone(), cx);
    })
    .detach();
    mettre_a_jour_largeurs(&etat, editeur, editeur.downgrade(), cx);
    etat
}

/// Recalcule les largeurs des widgets (maths : demande le rendu au moteur s'il manque).
fn mettre_a_jour_largeurs(
    etat: &Rc<MdInline>,
    editeur: &Entity<EditeurLive>,
    faible: gpui_kit::WeakEntity<EditeurLive>,
    cx: &mut App,
) {
    let theme = theme_ou_defaut(cx);
    let (widgets, taille) = {
        let ed = editeur.read(cx);
        (
            ed.projection().decorations.widgets.clone(),
            ed.taille_texte(),
        )
    };
    let moteur = etat.moteur.clone();
    let mut manque = false;
    let largeurs = largeurs_des_widgets(&widgets, taille, &mut |tex, affichage| {
        let cle = cle_maths(tex, affichage, taille, &theme);
        match moteur.en_cache(&cle) {
            Some(Ok(r)) => Some(r.largeur as f32),
            Some(Err(_)) => None,
            None => {
                moteur.demander(&cle);
                manque = true;
                None
            }
        }
    });
    editeur.update(cx, |ed, cx| ed.poser_largeurs_widgets(largeurs, cx));
    if manque && !etat.relit.replace(true) {
        // Le moteur travaille sur son fil : on relit son cache jusqu'à ce qu'il ait fini.
        let e = etat.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(RELECTURE_MS))
                    .await;
                let fini = e.moteur.en_attente() == 0;
                let Some(ed) = faible.upgrade() else { break };
                cx.update(|cx| ed.update(cx, |_, cx| cx.notify()));
                if fini {
                    break;
                }
            }
            e.relit.set(false);
        })
        .detach();
    }
}

/// Un geste que le calque capte.
#[derive(Clone)]
enum Geste {
    /// Une case : `marqueur` (UTF-16) du `[ ]`.
    Case { marqueur: usize },
    /// Une puce repliable : début de ligne (UTF-16).
    Puce { ligne: usize },
    /// Une formule : le LaTeX à sélectionner (octets).
    Maths { selection: Range<usize> },
}

fn teinte(c: Couleur) -> Hsla {
    couleur(c).into()
}

/// Le calque : un `canvas` qui peint les widgets et capte leurs clics.
fn peindre(etat: &Rc<MdInline>, ed: &EditeurLive) -> gpui_kit::AnyElement {
    let etat = etat.clone();
    let editeur_etat = ed.etat().clone();
    let widgets = ed.projection().decorations.widgets.clone();
    let lignes_deco = ed.projection().decorations.lignes.clone();
    let lignes = ed.lignes().to_vec();
    let texte: Rc<str> = Rc::from(ed.texte());
    let taille = ed.taille_texte();
    let interligne = ed.interligne();
    let lecture_seule = ed.lecture_seule();
    canvas(
        |_, _, _| (),
        move |_, (), window, cx| {
            let theme = theme_ou_defaut(cx);
            let mut gestes: Vec<(Bounds<Pixels>, Geste)> = Vec::new();
            peindre_indentation(&editeur_etat, &lignes_deco, &lignes, &theme, window, cx);
            for w in &widgets {
                let plage = w.source.octets.from..w.source.octets.to;
                let Some(b) = editeur_etat.read(cx).range_to_bounds(&plage) else {
                    continue;
                };
                match &w.genre {
                    GenreWidget::Case { cochee, marqueur } => {
                        peindre_case(*cochee, b, taille, &theme, window, cx);
                        if !lecture_seule {
                            gestes.push((
                                b,
                                Geste::Case {
                                    marqueur: *marqueur,
                                },
                            ));
                        }
                    }
                    GenreWidget::Puce { debut_ligne } => {
                        let (repliable, replie) = match &etat.repli {
                            Some(r) => ((r.repliable)(*debut_ligne), (r.replie)(*debut_ligne)),
                            None => (false, false),
                        };
                        let ctl = etat.ctl_puce(*debut_ligne);
                        peindre_puce(
                            aspect_puce(repliable, replie, ctl),
                            b,
                            taille,
                            &theme,
                            window,
                        );
                        if repliable {
                            gestes.push((
                                b,
                                Geste::Puce {
                                    ligne: *debut_ligne,
                                },
                            ));
                        }
                    }
                    GenreWidget::Maths { tex, affichage } => {
                        peindre_maths(
                            &etat, tex, *affichage, b, taille, interligne, &theme, window, cx,
                        );
                        gestes.push((
                            b,
                            Geste::Maths {
                                selection: selection_maths(plage, *affichage),
                            },
                        ));
                    }
                    _ => {}
                }
            }
            brancher(&etat, &editeur_etat, texte.clone(), gestes, window);
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// Un trait vertical par niveau, au centre de chaque cellule, sur toute la hauteur de la ligne
/// (`.cm-indent-line`) : continu d'une ligne à l'autre, widget remplacé par sa source ou non.
fn peindre_indentation(
    etat: &Entity<EditorState>,
    deco: &[fragment_markdown::decorations::DecoLigne],
    lignes: &[fragment_markdown::syntaxe::modele_rangees::Plage],
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let trait_ = teinte(
        theme
            .palette
            .text_normal
            .melange(22.0, Couleur::TRANSPARENT),
    );
    for d in deco {
        let GenreLigne::Indentation { niveaux } = d.genre else {
            continue;
        };
        let Some(p) = lignes.get(d.ligne) else {
            continue;
        };
        let Some(b) = etat
            .read(cx)
            .range_to_bounds(&(p.octets.from..p.octets.to.max(p.octets.from + 1)))
        else {
            continue;
        };
        let em = f32::from(b.size.height) / crate::editeur::constantes::INTERLIGNE_TEXTE;
        let cellule = fragment_markdown::decorations::styles::LARGEUR_INDENTATION_EM * em;
        for k in 0..niveaux {
            let x = f32::from(b.origin.x) + (k as f32) * cellule + cellule / 2.0
                - TRAIT_INDENTATION / 2.0;
            window.paint_quad(fill(
                Bounds::new(
                    point(px(x), b.origin.y),
                    size(px(TRAIT_INDENTATION), b.size.height),
                ),
                trait_,
            ));
        }
    }
}

/// La case : un carré de 1,05 em centré dans sa cellule de 1,6 em ; cochée, remplie d'accent et
/// marquée d'une coche.
fn peindre_case(
    cochee: bool,
    cellule: Bounds<Pixels>,
    taille: f32,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let cote = COTE_CASE_EM * taille;
    let x = f32::from(cellule.origin.x) + (f32::from(cellule.size.width) - cote) / 2.0;
    let y = f32::from(cellule.origin.y) + (f32::from(cellule.size.height) - cote) / 2.0;
    let boite = Bounds::new(point(px(x), px(y)), size(px(cote), px(cote)));
    let rayon = px(theme.forme.checkbox_radius);
    if cochee {
        window.paint_quad(
            fill(boite, teinte(theme.palette.interactive_accent)).corner_radii(Corners::all(rayon)),
        );
        if let Some(svg) = crate::icones::svg("check") {
            let marge = cote * 0.12;
            let coche = Bounds::new(
                point(px(x + marge), px(y + marge)),
                size(px(cote - 2.0 * marge), px(cote - 2.0 * marge)),
            );
            let _ = window.paint_svg(
                coche,
                "icons/check.svg".into(),
                Some(svg.as_bytes()),
                gpui_kit::TransformationMatrix::unit(),
                teinte(theme.palette.text_on_accent),
                cx,
            );
        }
    } else {
        window.paint_quad(
            gpui_kit::outline(
                boite,
                teinte(theme.palette.text_muted),
                gpui_kit::BorderStyle::Solid,
            )
            .corner_radii(Corners::all(rayon)),
        );
    }
}

/// La puce : un disque de 0,42 em au centre de sa cellule, et son anneau (spread de 0,28 em).
fn peindre_puce(
    aspect: super::geometrie::AspectPuce,
    cellule: Bounds<Pixels>,
    taille: f32,
    theme: &Theme,
    window: &mut Window,
) {
    let p = &theme.palette;
    let base = match aspect.couleur {
        CouleurPuce::Attenuee => p.text_muted,
        CouleurPuce::Normale => p.text_normal,
        CouleurPuce::Lien => p.link_color,
    };
    let d = POINT_PUCE_EM * taille;
    let cx_ = f32::from(cellule.origin.x) + f32::from(cellule.size.width) / 2.0;
    let cy = f32::from(cellule.origin.y) + f32::from(cellule.size.height) / 2.0;
    if let Some(pourcent) = aspect.anneau {
        let r = d / 2.0 + ANNEAU_PUCE_EM * taille;
        window.paint_quad(
            fill(
                Bounds::new(
                    point(px(cx_ - r), px(cy - r)),
                    size(px(2.0 * r), px(2.0 * r)),
                ),
                teinte(base.melange(pourcent, Couleur::TRANSPARENT)),
            )
            .corner_radii(Corners::all(px(r))),
        );
    }
    window.paint_quad(
        fill(
            Bounds::new(
                point(px(cx_ - d / 2.0), px(cy - d / 2.0)),
                size(px(d), px(d)),
            ),
            teinte(base),
        )
        .corner_radii(Corners::all(px(d / 2.0))),
    );
}

/// La formule : l'image du SVG de MathJax, à l'origine ARRONDIE au pixel et à sa taille de mise en
/// page (ADR-011), la ligne de base du SVG posée sur celle du texte.
#[allow(clippy::too_many_arguments)]
fn peindre_maths(
    etat: &MdInline,
    tex: &str,
    affichage: bool,
    cellule: Bounds<Pixels>,
    taille: f32,
    interligne: f32,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let cle = cle_maths(tex, affichage, taille, theme);
    let Some(Ok(rendu)) = etat.moteur.en_cache(&cle) else {
        return;
    };
    let echelle = window.scale_factor();
    let cle_image = (cle, (echelle * 100.0) as u32);
    let image = {
        let mut m = etat.images.borrow_mut();
        if let Some(i) = m.get(&cle_image) {
            i.clone()
        } else {
            let Ok(bitmap) = vers_bitmap(&rendu, echelle) else {
                return;
            };
            let Some(tampon) =
                image::RgbaImage::from_raw(bitmap.largeur, bitmap.hauteur, bitmap.bgra)
            else {
                return;
            };
            if m.len() >= IMAGES_MAX {
                for (_, i) in m.drain() {
                    cx.drop_image(i, Some(window));
                }
            }
            let i = Arc::new(RenderImage::new(vec![image::Frame::new(tampon)]));
            m.insert(cle_image, i.clone());
            i
        }
    };
    // La ligne de base du texte dans la rangée : l'interligne est réparti de part et d'autre de
    // (ascent + descent) arrondis, comme la boîte de ligne de Chromium.
    let ts = window.text_system().clone();
    let police = ts.resolve_font(&gpui_kit::font(FAMILLE_INTERFACE));
    let corps = px(taille);
    let ascent = f32::from(ts.ascent(police, corps)).round();
    let descent = f32::from(ts.descent(police, corps)).round();
    let haut_ligne =
        f32::from(cellule.origin.y) + (f32::from(cellule.size.height) - interligne) / 2.0;
    let base = haut_ligne + (interligne - (ascent + descent)) / 2.0 + ascent;
    let x = f32::from(cellule.origin.x).round();
    let y = (base - rendu.ligne_de_base as f32).round();
    let boite = Bounds::new(
        point(px(x), px(y)),
        size(px(rendu.largeur as f32), px(rendu.hauteur as f32)),
    );
    let _ = window.paint_image(boite, boite, Corners::default(), image, 0, false);
}

/// Branche la souris : le survol des puces et le clic sur un widget (capturé, arrêté).
fn brancher(
    etat: &Rc<MdInline>,
    editeur: &Entity<EditorState>,
    texte: Rc<str>,
    gestes: Vec<(Bounds<Pixels>, Geste)>,
    window: &mut Window,
) {
    let gestes = Rc::new(gestes);
    {
        let (etat, gestes) = (etat.clone(), gestes.clone());
        window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, _cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let sous = gestes.iter().find_map(|(b, g)| match g {
                Geste::Puce { ligne } if b.contains(&ev.position) => Some(*ligne),
                _ => None,
            });
            let avant = etat.puce.get();
            let apres = sous.map(|l| {
                let presse = ev.pressed_button == Some(MouseButton::Left);
                (
                    l,
                    if presse {
                        CtlPuce::Presse
                    } else {
                        CtlPuce::Survol
                    },
                )
            });
            if avant != apres {
                etat.puce.set(apres);
                window.refresh();
            }
        });
    }
    {
        let (etat, gestes, editeur) = (etat.clone(), gestes.clone(), editeur.clone());
        window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                return;
            }
            let Some((_, g)) = gestes.iter().find(|(b, _)| b.contains(&ev.position)) else {
                return;
            };
            cx.stop_propagation();
            match g {
                Geste::Case { marqueur } => {
                    if let Some((plage, remplace)) = bascule_case(&texte, *marqueur) {
                        basculer(&editeur, plage, remplace, window, cx);
                    }
                }
                Geste::Puce { ligne } => {
                    etat.puce.set(Some((*ligne, CtlPuce::Presse)));
                    window.refresh();
                }
                Geste::Maths { selection } => {
                    let s = selection.clone();
                    editeur.update(cx, |e, cx| {
                        e.focus(window, cx);
                        e.set_selected_range(s, cx);
                    });
                }
            }
        });
    }
    {
        let (etat, gestes) = (etat.clone(), gestes);
        window.on_mouse_event(move |ev: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                return;
            }
            let Some((l, CtlPuce::Presse)) = etat.puce.get() else {
                return;
            };
            let dessus = gestes.iter().any(|(b, g)| {
                matches!(g, Geste::Puce { ligne } if *ligne == l) && b.contains(&ev.position)
            });
            etat.puce.set(if dessus {
                Some((l, CtlPuce::Survol))
            } else {
                None
            });
            window.refresh();
            if dessus && let Some(r) = &etat.repli {
                (r.basculer)(l, window, cx);
            }
        });
    }
}

/// Une transaction : remplace `plage` par `texte` en gardant la sélection (cocher ne déplace rien),
/// annulable d'un seul Ctrl+Z.
fn basculer(
    editeur: &Entity<EditorState>,
    plage: Range<usize>,
    texte: &str,
    window: &mut Window,
    cx: &mut App,
) {
    editeur.update(cx, |e, cx| {
        let avant = e.selected_range();
        e.set_selected_range(plage, cx);
        e.replace(texte.to_owned(), window, cx);
        e.set_selected_range(avant, cx);
    });
}
