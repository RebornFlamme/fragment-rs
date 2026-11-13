// Port du branchement d'app/src/ui/views/markdown/htmlDecorations.ts dans l'éditeur (htmlBlockField,
// le ViewPlugin de buildHtmlInline, le `mousedown` du bouton « </> »).
//! L'hôte du HTML d'une note : il ENVELOPPE un [`HoteBlocs`] et lui ajoute le HTML du Markdown.
//!
//! - les blocs : un [`BlocHtml`] sur chaque bloc HTML rendu (curseur dehors) ; sa hauteur mesurée
//!   est rendue par le `Mesureur` de l'hôte des blocs (les lignes de la source sont masquées par
//!   `fragment_markdown::blocs`) ; le bouton « </> » pose le curseur au début du bloc, la source
//!   revient ;
//! - en ligne : un complément de rangées APRÈS celui des blocs masque les balises hors curseur
//!   ([`super::en_ligne`]) ; le texte balisé reçoit ses styles par une collection de décorations à
//!   nous sur l'`EditorState`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_markdown::blocs::{BlocCalcule, ContenuBloc, ModeBloc};
use fragment_markdown::decorations::curseur_actif;
use fragment_markdown::syntaxe::Etendue;
use fragment_markdown::syntaxe::modele_rangees::ModeleRangees;
use fragment_text::utf16::utf16_to_byte;
use gpui_kit::base::input::{TextDecoration, TextDecorationCollection};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div, px,
};

use super::bloc::{BlocHtml, EvenementBloc, style_en_ligne};
use super::en_ligne::{self, EnLigne};
use crate::editeur::constantes::INTERLIGNE_TEXTE;
use crate::editeur::constantes::{MARGE_LIGNE_DROITE, MARGE_LIGNE_GAUCHE, debut_texte, fin_texte};
use crate::editeur::{ComplementRangees, EditeurLive, EntreeComplement, OptionsEditeur};
use crate::md_blocs::{HoteBlocs, Mesureur};
use crate::theme_global::theme_ou_defaut;

/// L'hôte du HTML d'une note (une entité GPUI).
pub struct HoteHtml {
    blocs: Entity<HoteBlocs>,
    editeur: Entity<EditeurLive>,
    widgets: Vec<Entity<BlocHtml>>,
    /// La hauteur mesurée de chaque bloc HTML, par sa première ligne.
    hauteurs: Rc<RefCell<HashMap<usize, f32>>>,
    /// Le HTML en ligne de la dernière projection.
    en_ligne: Rc<RefCell<EnLigne>>,
    marques: TextDecorationCollection,
    marques_posees: Vec<TextDecoration>,
    lecture_seule: bool,
    _abonnements: Vec<Subscription>,
}

/// Une estimation de la hauteur d'un bloc avant sa première mesure : une ligne par ligne de source.
fn estimation(b: &BlocCalcule, taille: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let lignes = (b.derniere_ligne - b.premiere_ligne + 1) as f32;
    lignes * taille * INTERLIGNE_TEXTE
}

impl HoteHtml {
    /// Un éditeur de note sur `texte`, avec ses blocs et son HTML. `autres` mesure les blocs qui ne
    /// sont pas du HTML (tableaux, Properties, embeds…).
    pub fn new(
        texte: &str,
        options: OptionsEditeur,
        autres: Option<Mesureur>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let hauteurs: Rc<RefCell<HashMap<usize, f32>>> = Rc::default();
        let h = hauteurs.clone();
        let mesureur: Mesureur = Rc::new(move |b: &BlocCalcule, taille| {
            if matches!(b.contenu, ContenuBloc::Html { .. }) {
                return Some(
                    h.borrow()
                        .get(&b.premiere_ligne)
                        .copied()
                        .unwrap_or_else(|| estimation(b, taille)),
                );
            }
            autres.as_ref().and_then(|f| f(b, taille))
        });
        let blocs = cx.new(|cx| HoteBlocs::new(texte, options, Some(mesureur), window, cx));
        let editeur = blocs.read(cx).editeur().clone();
        let en_ligne: Rc<RefCell<EnLigne>> = Rc::default();
        let el = en_ligne.clone();
        let aval: ComplementRangees =
            Rc::new(move |e: &EntreeComplement<'_>, m: &mut ModeleRangees| {
                let sel = e
                    .curseur
                    .selections
                    .first()
                    .copied()
                    .unwrap_or(Etendue::new(0, 0));
                let actif = !e.ctx.lecture_seule
                    && curseur_actif(e.ctx.revelation, e.curseur.focus, e.curseur.a_eu_le_focus);
                let r = en_ligne::analyser(e.texte, e.arbre, sel, actif);
                m.masques.extend(r.masques.iter().copied());
                *el.borrow_mut() = r;
            });
        blocs.update(cx, |b, cx| b.poser_complement_aval(Some(aval), cx));
        let etat = editeur.read(cx).etat().clone();
        let marques = etat.update(cx, |s, cx| s.create_decorations_collection(Vec::new(), cx));
        let abonnements = vec![
            cx.observe(&editeur, |h, _, cx| h.synchroniser(cx)),
            cx.observe(&blocs, |_, _, cx| cx.notify()),
        ];
        let mut h = HoteHtml {
            blocs,
            editeur,
            widgets: Vec::new(),
            hauteurs,
            en_ligne,
            marques,
            marques_posees: Vec::new(),
            lecture_seule: options.lecture_seule,
            _abonnements: abonnements,
        };
        h.synchroniser(cx);
        h
    }

    #[must_use]
    pub fn blocs(&self) -> &Entity<HoteBlocs> {
        &self.blocs
    }

    #[must_use]
    pub fn editeur(&self) -> &Entity<EditeurLive> {
        &self.editeur
    }

    /// Les blocs HTML RENDUS (curseur dehors), dans l'ordre du document.
    #[must_use]
    pub fn widgets(&self) -> &[Entity<BlocHtml>] {
        &self.widgets
    }

    /// Le HTML en ligne de la dernière projection.
    #[must_use]
    pub fn en_ligne(&self) -> EnLigne {
        self.en_ligne.borrow().clone()
    }

    /// Les styles posés sur le texte balisé en ligne (octets).
    #[must_use]
    pub fn marques(&self) -> &[TextDecoration] {
        &self.marques_posees
    }

    /// Les blocs HTML de la dernière projection qui sont rendus à la place de leur source.
    fn blocs_rendus(&self, cx: &Context<Self>) -> Vec<BlocCalcule> {
        self.blocs
            .read(cx)
            .blocs()
            .into_iter()
            .filter(|b| {
                matches!(b.contenu, ContenuBloc::Html { .. }) && b.mode == ModeBloc::Remplace
            })
            .collect()
    }

    /// Un widget par bloc rendu (mis à jour en place, créés, retirés), puis les styles en ligne.
    fn synchroniser(&mut self, cx: &mut Context<Self>) {
        let rendus = self.blocs_rendus(cx);
        self.widgets.truncate(rendus.len());
        for (i, b) in rendus.iter().enumerate() {
            let ContenuBloc::Html { source } = &b.contenu else {
                continue;
            };
            let (debut, ligne) = (b.source.utf16.from, b.premiere_ligne);
            if let Some(w) = self.widgets.get(i) {
                w.update(cx, |w, cx| {
                    w.poser_source(source);
                    w.poser_place(debut, ligne);
                    cx.notify();
                });
                continue;
            }
            let lecture_seule = self.lecture_seule;
            let w = cx.new(|_| BlocHtml::new(source, debut, ligne, lecture_seule));
            self._abonnements
                .push(cx.subscribe(&w, Self::sur_evenement));
            self.widgets.push(w);
        }
        self.poser_marques(cx);
        cx.notify();
    }

    fn sur_evenement(&mut self, w: Entity<BlocHtml>, ev: &EvenementBloc, cx: &mut Context<Self>) {
        match *ev {
            EvenementBloc::Hauteur(v) => {
                let ligne = w.read(cx).premiere_ligne();
                let avant = self.hauteurs.borrow_mut().insert(ligne, v);
                if avant.is_none_or(|a| (a - v).abs() > 0.01) {
                    self.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
                    cx.notify();
                }
            }
            EvenementBloc::Source => {
                // `view.dispatch({ selection: { anchor: from } })` puis `view.focus()`.
                let debut = w.read(cx).debut();
                let etat = self.editeur.read(cx).etat().clone();
                cx.defer(move |cx| {
                    let Some(fenetre) = cx.active_window() else {
                        return;
                    };
                    let _ = fenetre.update(cx, |_, window, cx| {
                        etat.update(cx, |s, cx| {
                            let t = s.value().to_string();
                            let o = utf16_to_byte(&t, debut).unwrap_or(t.len());
                            s.focus(window, cx);
                            s.set_selected_range(o..o, cx);
                        });
                    });
                });
            }
        }
    }

    /// Pose les styles du texte balisé en ligne (seulement s'ils ont changé).
    fn poser_marques(&mut self, cx: &mut Context<Self>) {
        let theme = theme_ou_defaut(cx);
        let taille = self.editeur.read(cx).taille_texte();
        let liste: Vec<TextDecoration> = self
            .en_ligne
            .borrow()
            .marques
            .iter()
            .map(|m| {
                let style = m
                    .balise
                    .attributes
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("style"))
                    .map(|(_, v)| v.as_str());
                TextDecoration::new(
                    m.plage.octets.range(),
                    style_en_ligne(&theme, &m.balise.tag_name, style, taille),
                )
            })
            .collect();
        if liste != self.marques_posees {
            self.marques.set(liste.clone(), cx);
            self.marques_posees = liste;
        }
    }
}

impl Render for HoteHtml {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.blocs.read(cx).geometrie();
        let gauche = debut_texte(g.largeur) - MARGE_LIGNE_GAUCHE;
        let largeur = (g.largeur - gauche - fin_texte(g.largeur) + MARGE_LIGNE_DROITE).max(0.0);
        let mut couche = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden();
        for w in &self.widgets {
            let ligne = w.read(cx).premiere_ligne();
            let Some(le) = g.ligne(ligne) else { continue };
            if le.hauteur <= 0.0 {
                continue;
            }
            couche = couche.child(
                div()
                    .absolute()
                    .left(px(gauche))
                    .top(px(le.haut - g.haut))
                    .w(px(largeur))
                    .child(w.clone()),
            );
        }
        div()
            .relative()
            .size_full()
            .child(self.blocs.clone())
            .child(couche)
    }
}
