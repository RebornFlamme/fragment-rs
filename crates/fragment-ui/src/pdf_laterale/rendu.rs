// Port de app/src/ui/views/pdf/pdf-view.css (`.pdf-sidebar-*`, `.pdf-thumbnail*`, `.pdf-outline-*`)
// et de app/src/ui/views/pdf/PdfSidebar.ts (construireToc, construireEntree, construirePoignee)
//
// Le dessin de la barre latérale. Les mesures sont celles du CSS (`docs/constantes-rendu.md`),
// lues dans les jetons de forme du thème : rembourrage de la vue des vignettes 5 / 7 px, de la
// vignette 4 px, écart 8 px, vue de la table 8 / 3 / 8 / 8 px. La table des matières est un arbre
// de `tree_item` (mêmes lignes que l'explorateur), sur le fond de la barre.

use std::sync::Arc;

use fragment_pdf::laterale::LigneToc;
use fragment_theme::{Aplat, Couleur};
use gpui_kit::{
    AnyElement, App, Context, Corners, DispatchPhase, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Render,
    ScrollEvent, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, canvas, div,
    point, px, relative,
};

use super::vue::LateralePdf;
use super::{
    COTE_NUMERO, DEBORD_POIGNEE, DECALAGE_NUMERO, LARGEUR_POIGNEE, LIGNE_MOLETTE,
    MESSAGE_PADDING_H, MESSAGE_PADDING_V, PLAFOND_RELATIF,
};
use crate::echelle::{Echelle, u};
use crate::flottant::{ombrer, texte};
use crate::pdf_pages::pixels_peints;
use crate::theme_global::{couleur, theme_ou_defaut};
use crate::tree_item::geometrie::hauteur_ligne;
use crate::tree_item::{Ligne, hauteurs_calees, rendre_item};

/// La largeur logique (px CSS) d'une position de la souris.
fn en_css(x: gpui_kit::Pixels, zoom: f64) -> f64 {
    f64::from(f32::from(x)) / zoom
}

impl Render for LateralePdf {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Barre fermée = `display: none` : rien n'est mesuré, donc rien n'est peint.
        if !self.modele.est_ouverte() {
            return div().hidden().into_any_element();
        }
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);
        let entite = cx.entity().downgrade();
        let zero = u(cx, 0.0);

        // La zone de contenu : mesurée au prépeint, livrée après l'image ; la peinture retire de
        // l'atlas les images jetées.
        let jetees = std::rc::Rc::clone(&self.a_jeter);
        let mesure = {
            let entite = entite.clone();
            canvas(
                move |bounds, _: &mut Window, cx: &mut App| {
                    cx.defer(move |cx| {
                        let _ = entite.update(cx, |v, cx| v.apres_image(bounds, cx));
                    });
                },
                move |_, (), window: &mut Window, _| {
                    for image in jetees.borrow_mut().drain(..) {
                        let _ = window.drop_image(image);
                    }
                },
            )
            .absolute()
            .top(zero)
            .left(zero)
            .size_full()
        };

        let mut contenu = div()
            .id("pdf-sidebar-content")
            .relative()
            .flex_1()
            .min_w(zero)
            .h_full()
            .overflow_hidden()
            .child(mesure);
        contenu = match self.modele.mode() {
            fragment_pdf::laterale::ModeBarre::Vignettes => self
                .rendre_vignettes(cx)
                .into_iter()
                .fold(contenu, |c, v| c.child(v))
                .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| this.molette(ev, cx))),
            fragment_pdf::laterale::ModeBarre::TableDesMatieres => {
                contenu.child(self.rendre_table(cx))
            }
        };

        // La poignée, à cheval sur la bordure droite.
        let accent = couleur(c.color_accent.melange(50.0, Couleur::TRANSPARENT));
        let poignee = div()
            .id("pdf-sidebar-resizer")
            .absolute()
            .top(zero)
            .bottom(zero)
            .right(u(cx, DEBORD_POIGNEE))
            .w(u(cx, LARGEUR_POIGNEE))
            .cursor_col_resize()
            .hover(move |s| s.bg(accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    let zoom = f64::from(Echelle::courante(cx).zoom());
                    this.poignee_saisir(en_css(ev.position.x, zoom), cx);
                    cx.stop_propagation();
                }),
            );

        // Les écouteurs GLOBAUX du pointeur : le glissement continue hors de la poignée.
        let ecouteurs = {
            let (deplace, relache) = (entite.clone(), entite);
            canvas(
                |_, _, _| {},
                move |_, (), window: &mut Window, _| {
                    let deplace = deplace.clone();
                    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        let zoom = f64::from(Echelle::courante(cx).zoom());
                        let _ = deplace.update(cx, |v, cx| {
                            v.poignee_deplacer(en_css(ev.position.x, zoom), cx);
                        });
                    });
                    let relache = relache.clone();
                    window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left {
                            return;
                        }
                        let _ = relache.update(cx, |v, cx| v.poignee_relacher(cx));
                    });
                },
            )
            .absolute()
            .top(zero)
            .left(zero)
            .size_full()
        };

        texte(
            div()
                .id("pdf-sidebar-container")
                .relative()
                .flex()
                .flex_none()
                .h_full()
                .w(u(cx, self.modele.largeur() as f32))
                .max_w(relative(PLAFOND_RELATIF))
                .bg(couleur(c.workspace_bg))
                .border_r(u(cx, f.border_width))
                .border_color(couleur(c.background_modifier_border))
                .debug_selector(|| "pdf-sidebar-container".to_owned())
                .child(contenu)
                .child(poignee)
                .child(ecouteurs),
            cx,
            t.typo.font_ui_small,
            t.typo.line_height_normal,
        )
        .into_any_element()
    }
}

impl LateralePdf {
    /// Molette sur les vignettes : la barre défile (bornée par la hauteur du contenu).
    fn molette(&mut self, ev: &ScrollEvent, cx: &mut Context<Self>) {
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let dy = en_css(ev.delta.pixel_delta(u(cx, LIGNE_MOLETTE)).y, zoom);
        let t = theme_ou_defaut(cx);
        let total = self.hauteur_totale(&t.forme);
        let max = (total - self.contenu.1).max(0.0);
        let haut = (self.modele.scroll_top() - dy).clamp(0.0, max);
        self.agir(cx, |m| m.defiler(haut));
        cx.stop_propagation();
    }

    /// La largeur de la boîte d'une vignette et de son image, en px CSS.
    fn largeurs(&self, f: &fragment_theme::Forme) -> (f32, f32) {
        let contenu = if self.contenu.0 > 0.0 {
            self.contenu.0 as f32
        } else {
            self.modele.largeur() as f32 - f.border_width
        };
        let boite = (contenu - 2.0 * f.size_2_7).max(1.0);
        (boite, (boite - 2.0 * f.size_4_1).max(1.0))
    }

    /// La hauteur de la boîte de la vignette d'indice `i` (0-based).
    fn hauteur_boite(&self, i: usize, largeur_image: f32, f: &fragment_theme::Forme) -> f32 {
        largeur_image * self.ratios.get(i).copied().unwrap_or(1.0) as f32 + 2.0 * f.size_4_1
    }

    /// La hauteur du contenu défilant des vignettes : rembourrage, boîtes et écarts.
    fn hauteur_totale(&self, f: &fragment_theme::Forme) -> f64 {
        let (_, image) = self.largeurs(f);
        let n = self.ratios.len();
        let boites: f32 = (0..n).map(|i| self.hauteur_boite(i, image, f)).sum();
        let ecarts = n.saturating_sub(1) as f32 * f.size_4_2;
        f64::from(2.0 * f.size_2_5 + boites + ecarts)
    }

    /// Les vignettes dans la zone d'observation, posées à leur place dans le contenu défilant.
    fn rendre_vignettes(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);
        let (largeur_boite, largeur_image) = self.largeurs(f);
        let visibles = self.modele.vignettes_visibles();
        let Some(&premiere) = visibles.first() else {
            return Vec::new();
        };
        let defilement = self.modele.scroll_top() as f32;
        let actif = self.modele.page_active();
        let zero = u(cx, 0.0);

        // Le haut de la première vignette visible : on cumule les précédentes.
        let mut haut = f.size_2_5
            + (0..premiere - 1)
                .map(|i| self.hauteur_boite(i, largeur_image, f) + f.size_4_2)
                .sum::<f32>();
        let fond_actif = couleur(c.aplat(Aplat::SurvolActif, c.workspace_bg));
        let fond_survol = couleur(c.aplat(Aplat::Survol, c.workspace_bg));

        let mut sortie = Vec::new();
        for page in visibles {
            let i = page - 1;
            let hauteur = self.hauteur_boite(i, largeur_image, f);
            let hauteur_image = hauteur - 2.0 * f.size_4_1;
            let y = haut - defilement;
            haut += hauteur + f.size_4_2;

            let image = self
                .vignettes
                .get(&page)
                .and_then(|v| v.image.as_ref())
                .map(|(image, _)| Arc::clone(image));
            let mesures = std::rc::Rc::clone(&self.mesures_image);
            let peinture = canvas(
                move |bounds, window: &mut Window, _: &mut App| {
                    if let Some(p) = pixels_peints(bounds, window.scale_factor()) {
                        mesures.borrow_mut().push((page, p));
                    }
                },
                move |bounds, (), window: &mut Window, _| {
                    if let Some(image) = image {
                        let _ =
                            window.paint_image(bounds, bounds, Corners::default(), image, 0, false);
                    }
                },
            )
            .absolute()
            .top(zero)
            .left(zero)
            .size_full();

            let page_image = ombrer(
                div()
                    .absolute()
                    .left(u(cx, f.size_4_1))
                    .top(u(cx, f.size_4_1))
                    .w(u(cx, largeur_image))
                    .h(u(cx, hauteur_image))
                    .bg(couleur(c.pdf_page_bg))
                    .child(peinture)
                    .debug_selector(move || format!("vignette-image-{page}")),
                cx,
                &c.shadow_s,
                "shadow-s",
                zero,
                zero,
            );

            let numero = texte(
                div()
                    .absolute()
                    .right(u(cx, DECALAGE_NUMERO))
                    .bottom(u(cx, DECALAGE_NUMERO))
                    .min_w(u(cx, COTE_NUMERO))
                    .px(u(cx, f.size_4_1))
                    .py(u(cx, 1.0))
                    .rounded(u(cx, f.radius_xs))
                    .bg(couleur(c.background_overlay))
                    .text_color(couleur(c.text_on_overlay))
                    .text_center(),
                cx,
                t.typo.font_ui_smallest,
                1.0,
            )
            .line_height(u(cx, COTE_NUMERO))
            .child(page.to_string());

            let mut boite = div()
                .id(("vignette", page))
                .absolute()
                .left(u(cx, f.size_2_7))
                .top(u(cx, y))
                .w(u(cx, largeur_boite))
                .h(u(cx, hauteur))
                .rounded(u(cx, f.radius_s))
                .cursor_default()
                .debug_selector(move || format!("vignette-{page}"))
                .on_click(cx.listener(move |this, _, _, cx| this.cliquer_vignette(page, cx)))
                .child(page_image)
                .child(numero);
            // `.is-active` vient après `:hover` dans le CSS : l'actif garde son fond sous le pointeur.
            boite = if page == actif {
                boite.bg(fond_actif)
            } else {
                boite.hover(move |s| s.bg(fond_survol))
            };
            sortie.push(boite.into_any_element());
        }
        sortie
    }

    /// L'arbre de la table des matières (ou le message quand il n'y en a pas).
    fn rendre_table(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme_ou_defaut(cx);
        let (c, f) = (&t.palette, &t.forme);
        if let Some(message) = self.modele.message_vide() {
            return texte(
                div()
                    .id("pdf-outline-empty")
                    .px(u(cx, MESSAGE_PADDING_H))
                    .py(u(cx, MESSAGE_PADDING_V))
                    .text_color(couleur(c.text_muted))
                    .text_center()
                    .debug_selector(|| "pdf-outline-empty".to_owned())
                    .child(message),
                cx,
                t.typo.font_ui_small,
                t.typo.line_height_normal,
            )
            .into_any_element();
        }

        let lignes = self.modele.toc().lignes();
        self.centrer_entree(&lignes, hauteur_ligne(f.size_4_1, t.typo.font_ui_small), cx);
        let hauteurs = hauteurs_calees(cx, f.size_4_2, lignes.len());
        let entite = cx.entity().downgrade();
        let mut i = 0;
        let items = construire(&lignes, &hauteurs, &mut i, 0, &entite, cx);
        div()
            .id("pdf-outline-view")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.defilement_toc)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .pt(u(cx, f.size_4_2))
                    .pr(u(cx, f.size_2_3))
                    .pb(u(cx, f.size_4_2))
                    .pl(u(cx, f.size_4_2))
                    .children(items),
            )
            .into_any_element()
    }

    /// « Reveal » : fait défiler l'arbre pour centrer l'entrée dans la barre (`scrollIntoView
    /// ({ block: 'center' })`). Attend que la hauteur visible soit connue.
    fn centrer_entree(&mut self, lignes: &[LigneToc], hauteur: f32, cx: &mut Context<Self>) {
        if self.contenu.1 <= 0.0 {
            return;
        }
        let Some(chemin) = self.a_centrer.take() else {
            return;
        };
        let Some(rang) = lignes.iter().position(|l| l.chemin == chemin) else {
            return;
        };
        let t = theme_ou_defaut(cx);
        let marge = t.forme.size_4_2;
        let total = 2.0 * marge + lignes.len() as f32 * hauteur;
        let milieu = marge + (rang as f32 + 0.5) * hauteur;
        let max = (total - self.contenu.1 as f32).max(0.0);
        let haut = (milieu - self.contenu.1 as f32 / 2.0).clamp(0.0, max);
        self.defilement_toc.set_offset(point(px(0.0), -u(cx, haut)));
    }
}

/// Les lignes de profondeur `profondeur` à partir de `*i`, chacune suivie de ses enfants quand
/// elle est dépliée (l'ordre de `lignes()` est celui de l'affichage, en pré-ordre).
fn construire(
    lignes: &[LigneToc],
    hauteurs: &[gpui_kit::Pixels],
    i: &mut usize,
    profondeur: usize,
    entite: &WeakEntity<LateralePdf>,
    cx: &App,
) -> Vec<AnyElement> {
    let t = theme_ou_defaut(cx);
    let mut sortie = Vec::new();
    while let Some(l) = lignes.get(*i).filter(|l| l.profondeur == profondeur) {
        let rang = *i;
        *i += 1;
        let enfants = if l.repliable && !l.replie {
            construire(lignes, hauteurs, i, profondeur + 1, entite, cx)
        } else {
            Vec::new()
        };
        let sonde = format!(
            "toc-{}",
            l.chemin
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("-")
        );
        let mut ligne = Ligne::new(("toc", rang), l.titre.clone())
            .profondeur(profondeur)
            .actif(l.active)
            .panneau(t.palette.workspace_bg)
            .hauteur(hauteurs.get(rang).copied().unwrap_or_else(|| u(cx, 26.0)))
            .sonde(sonde);
        if let Some(page) = l.page {
            ligne = ligne.flair(page.to_string());
        }
        let chemin = l.chemin.clone();
        if l.repliable {
            let (cible, chemin) = (entite.clone(), chemin.clone());
            ligne = ligne.repliable(l.replie).sur_chevron(move |_, _, app| {
                let _ = cible.update(app, |v, cx| v.basculer_entree(&chemin, cx));
            });
        }
        let cible = entite.clone();
        ligne = ligne.sur_clic(move |_, _, app| {
            let _ = cible.update(app, |v, cx| v.cliquer_entree(&chemin, cx));
        });
        sortie.push(rendre_item(ligne, enfants, cx));
    }
    sortie
}
