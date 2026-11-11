// Port de l'éditeur de note d'app/src/ui/views/markdown/MarkdownView.ts (création de l'EditorView,
// extensions du live preview) et des règles de colonne d'editor.css / markdown-view.css.
//! L'élément éditeur du live preview : l'`EditorState` du fork (ADR-006) PILOTÉ par le modèle.
//!
//! À chaque changement de texte, de sélection ou de focus, l'élément recalcule la projection du
//! live preview ([`super::modele::projeter`]) et la pose sur le fork : masques sans trou, hauteur
//! et taille de police des titres, styles de texte des marques et coloration du code.
//!
//! ★ `set_row_model` NOTIFIE TOUJOURS l'éditeur, et l'élément observe ses notifications pour suivre
//!   la sélection (le fork n'émet aucun événement de sélection) : sans la garde de comparaison
//!   (`fork` ci-dessous), chaque pose relancerait une observation, qui reposerait le même modèle, à
//!   l'infini.
//! ★ On rend l'élément de BASE du fork (`gpui_kit::base::input::Editor`), pas celui de
//!   `gpui_kit::component` : le composant réimpose à chaque image ses marges, sa police monospace
//!   et les couleurs du thème de gpui-kit (le curseur y prendrait `cx.theme().caret`, illisible en
//!   thème clair). Ici, police, interligne, marges de la colonne et couleurs viennent de nos jetons.

use std::cell::Cell;
use std::ops::Range;

use fragment_markdown::decorations::{Contexte, Curseur, Revelation};
use fragment_markdown::syntaxe::imbrique::Imbrique;
use fragment_markdown::syntaxe::modele_rangees::{ArgumentsFork, Plage, lignes_de};
use fragment_markdown::syntaxe::{Arbre, Changement, Etendue};
use fragment_text::editor::EditorChange;
use fragment_text::utf16::byte_to_utf16;
use fragment_theme::{Mode, Theme};
use gpui_kit::base::input::{
    DiagnosticColors, Editor as EditeurDeBase, EditorState, InputEditorStyle, InputEvent,
    TextDecoration, TextDecorationCollection,
};
use gpui_kit::{
    App, AppContext as _, Context, Edges, Entity, EventEmitter, HighlightStyle, Hsla, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};

use super::constantes::{INTERLIGNE_TEXTE, MARGE_CONTENU_VERTICALE, debut_texte, fin_texte};
use super::jetons::couleur_selection;
use super::modele::{Projection, projeter};
use super::surface::SurfaceLive;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La configuration d'un éditeur de live preview.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionsEditeur {
    /// Une note : `ApresPremierFocus` ; une cellule de tableau : `FocusSeulement`.
    pub revelation: Revelation,
    pub lecture_seule: bool,
    /// Largeur d'un niveau d'indentation, en colonnes (réglage Tab size).
    pub unite_indentation: usize,
    /// `--font-text-size`, en pixels logiques.
    pub taille_texte: f32,
}

impl Default for OptionsEditeur {
    fn default() -> Self {
        OptionsEditeur {
            revelation: Revelation::ApresPremierFocus,
            lecture_seule: false,
            unite_indentation: 4,
            taille_texte: fragment_markdown::decorations::styles::TAILLE_TEXTE_DEFAUT,
        }
    }
}

/// L'élément éditeur du live preview (une entité GPUI). Émet un [`EditorChange`] à chaque
/// changement de texte ou de sélection.
pub struct EditeurLive {
    etat: Entity<EditorState>,
    options: OptionsEditeur,
    texte: String,
    lignes: Vec<Plage>,
    arbre: Arbre,
    imbrique: Imbrique,
    /// La sélection principale connue, en octets (pour reconnaître un changement de sélection).
    selection: Range<usize>,
    focus: bool,
    a_eu_le_focus: bool,
    /// Garde de comparaison : le dernier modèle posé sur le fork.
    fork: Option<ArgumentsFork>,
    projection: Projection,
    marques: TextDecorationCollection,
    styles_lignes: TextDecorationCollection,
    /// Le mode du thème de la dernière projection (un basculement reprojette les couleurs).
    mode: Mode,
    /// Nombre de poses réelles du modèle sur le fork (la garde se teste par là).
    poses: usize,
    /// Le « poke » de `request_update` : relu à la prochaine image.
    pub(crate) a_rafraichir: Cell<bool>,
    _abonnements: Vec<Subscription>,
}

impl EventEmitter<EditorChange> for EditeurLive {}

impl EditeurLive {
    /// Un éditeur sur `texte`.
    pub fn new(
        texte: &str,
        options: OptionsEditeur,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let valeur = texte.to_owned();
        let etat = cx.new(|cx| {
            let mut e = EditorState::new(window, cx)
                .line_number(false)
                .auto_close(false)
                .smart_indent(false)
                .soft_wrap(true)
                .default_value(valeur);
            e.set_readonly(options.lecture_seule, cx);
            e
        });
        let (marques, styles_lignes) = etat.update(cx, |s, cx| {
            (
                s.create_decorations_collection(Vec::new(), cx),
                s.create_decorations_collection(Vec::new(), cx),
            )
        });
        let abonnements = vec![
            cx.subscribe(&etat, Self::sur_evenement),
            cx.observe(&etat, Self::sur_notification),
        ];
        let arbre = Arbre::analyser(texte);
        let imbrique = Imbrique::analyser(texte, &arbre);
        let mode = theme_ou_defaut(cx).mode;
        let mut e = EditeurLive {
            etat,
            options,
            texte: texte.to_owned(),
            lignes: lignes_de(texte),
            arbre,
            imbrique,
            selection: 0..0,
            focus: false,
            a_eu_le_focus: false,
            fork: None,
            projection: Projection::default(),
            marques,
            styles_lignes,
            mode,
            poses: 0,
            a_rafraichir: Cell::new(false),
            _abonnements: abonnements,
        };
        e.recalculer(cx);
        e
    }

    /// L'état d'édition du fork (texte, sélection, géométrie).
    #[must_use]
    pub fn etat(&self) -> &Entity<EditorState> {
        &self.etat
    }

    /// Le texte courant.
    #[must_use]
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Les lignes logiques du texte courant.
    #[must_use]
    pub fn lignes(&self) -> &[Plage] {
        &self.lignes
    }

    /// La dernière projection posée.
    #[must_use]
    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    /// Combien de fois le modèle a réellement été posé sur le fork.
    #[must_use]
    pub fn poses(&self) -> usize {
        self.poses
    }

    /// L'éditeur a-t-il le focus ?
    #[must_use]
    pub fn a_le_focus(&self) -> bool {
        self.focus
    }

    /// La taille du texte, en pixels logiques.
    #[must_use]
    pub fn taille_texte(&self) -> f32 {
        self.options.taille_texte
    }

    /// L'interligne de base (une rangée de texte courant), en pixels logiques.
    #[must_use]
    pub fn interligne(&self) -> f32 {
        self.options.taille_texte * INTERLIGNE_TEXTE
    }

    /// La surface de texte (`TextSurface`, UTF-16) de cet éditeur, lue dans `cx`.
    #[must_use]
    pub fn surface<'a>(&'a self, cx: &'a App) -> SurfaceLive<'a> {
        SurfaceLive::new(self, self.etat.read(cx))
    }

    /// Le modèle posé sur le fork (`None` avant la première pose).
    #[must_use]
    pub fn arguments_fork(&self) -> Option<&ArgumentsFork> {
        self.fork.as_ref()
    }

    fn sur_evenement(
        &mut self,
        etat: Entity<EditorState>,
        ev: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match ev {
            InputEvent::Change => {
                let nouveau = etat.read(cx).value().to_string();
                if nouveau == self.texte {
                    return;
                }
                let c = Changement::entre(&self.texte, &nouveau);
                let remplacement =
                    super::changement::Remplacement::depuis(&self.texte, &nouveau, c);
                // ★ Les deux analyses incrémentales CONSOMMENT l'ancien arbre : on le remplace
                //   le temps de l'appel par l'arbre (vide, immédiat) d'un texte vide.
                let ancien = std::mem::replace(&mut self.arbre, Arbre::analyser(""));
                self.arbre = ancien.reanalyser(&nouveau, c);
                let vide = Imbrique::analyser("", &Arbre::analyser(""));
                let ancienne = std::mem::replace(&mut self.imbrique, vide);
                self.imbrique = ancienne.reanalyser(&nouveau, &self.arbre, c);
                self.lignes = lignes_de(&nouveau);
                self.texte = nouveau;
                self.selection = etat.read(cx).selected_range();
                cx.emit(remplacement.en_changement());
                self.recalculer(cx);
            }
            InputEvent::Focus => {
                self.focus = true;
                self.a_eu_le_focus = true;
                self.recalculer(cx);
            }
            InputEvent::Blur => {
                self.focus = false;
                self.recalculer(cx);
            }
            InputEvent::PressEnter { .. } => {}
        }
    }

    fn sur_notification(&mut self, etat: Entity<EditorState>, cx: &mut Context<Self>) {
        let selection = etat.read(cx).selected_range();
        if selection != self.selection {
            self.selection = selection;
            cx.emit(super::changement::changement_de_selection());
            self.recalculer(cx);
        }
    }

    /// Recalcule la projection et la pose sur le fork (seulement ce qui a changé).
    fn recalculer(&mut self, cx: &mut Context<Self>) {
        self.reprojeter(cx);
        cx.notify();
    }

    fn reprojeter(&mut self, cx: &mut App) {
        let theme = theme_ou_defaut(cx);
        self.mode = theme.mode;
        let (a, b) = (
            self.selection.start.min(self.selection.end),
            self.selection.start.max(self.selection.end),
        );
        let selections = [Etendue::new(
            byte_to_utf16(&self.texte, a),
            byte_to_utf16(&self.texte, b),
        )];
        let curseur = Curseur {
            selections: &selections,
            focus: self.focus,
            a_eu_le_focus: self.a_eu_le_focus,
        };
        let ctx = Contexte {
            revelation: self.options.revelation,
            lecture_seule: self.options.lecture_seule,
            visibles: &[],
            unite_indentation: self.options.unite_indentation,
            taille_texte: self.options.taille_texte,
        };
        let p = projeter(
            &self.texte,
            &self.arbre,
            &self.imbrique,
            &curseur,
            &ctx,
            &theme,
        );
        if self.fork.as_ref() != Some(&p.fork) {
            let f = p.fork.clone();
            self.etat.update(cx, |s, cx| {
                s.set_line_font_sizes(f.polices.iter().map(|&(l, t)| (l, px(t))).collect(), cx);
                s.set_row_model(
                    f.masques,
                    f.hauteurs.iter().map(|&(l, h)| (l, px(h))).collect(),
                    f.blocs.iter().map(|&(l, h)| (l, px(h))).collect(),
                    cx,
                );
            });
            self.fork = Some(p.fork.clone());
            self.poses += 1;
        }
        if p.marques != self.projection.marques {
            self.marques.set(en_decorations(&p.marques), cx);
        }
        if p.lignes != self.projection.lignes {
            self.styles_lignes.set(en_decorations(&p.lignes), cx);
        }
        self.projection = p;
    }
}

fn en_decorations(liste: &[(Range<usize>, HighlightStyle)]) -> Vec<TextDecoration> {
    liste
        .iter()
        .map(|(r, s)| TextDecoration::new(r.clone(), *s))
        .collect()
}

/// Le style que l'élément projette sur le fork : texte, curseur, sélection, fond, en jetons.
///
/// ★ Le curseur prend `--text-normal` (`editor.css:46`, `caret-color: var(--text-normal)`) : clair
///   en sombre, sombre en clair. Une couleur fixe le rendait invisible dans l'un des deux thèmes.
#[must_use]
pub fn style_projete(theme: &Theme) -> InputEditorStyle {
    let p = &theme.palette;
    let h = |c| -> Hsla { couleur(c).into() };
    InputEditorStyle {
        foreground: h(p.text_normal),
        muted_foreground: h(p.text_muted),
        background: h(p.background_primary),
        border: h(p.aplat(fragment_theme::Aplat::Bordure, p.background_primary)),
        selection: couleur_selection(theme),
        caret: h(p.text_normal),
        diagnostics: DiagnosticColors {
            error: h(p.text_error),
            warning: h(p.text_warning),
            info: h(p.text_accent),
            hint: h(p.text_muted),
        },
        ..InputEditorStyle::default()
    }
}

impl Render for EditeurLive {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        if theme.mode != self.mode || self.a_rafraichir.replace(false) {
            self.reprojeter(cx);
        }
        // La colonne se centre dans la largeur de l'éditeur (connue après la première image ; avant,
        // celle de la fenêtre).
        let bornes = self.etat.read(cx).input_bounds();
        let largeur = if bornes.size.width > px(0.) {
            f32::from(bornes.size.width)
        } else {
            f32::from(window.viewport_size().width)
        };
        let marges = Edges {
            top: px(MARGE_CONTENU_VERTICALE),
            bottom: px(MARGE_CONTENU_VERTICALE),
            left: px(debut_texte(largeur)),
            right: px(fin_texte(largeur)),
        };
        let style = style_projete(&theme);
        self.etat.update(cx, |s, _| {
            s.set_editor_paddings(marges);
            s.set_editor_style(style);
        });
        let taille = self.options.taille_texte;
        // ★ L'élément du fork est un enfant `flex_1` + `h_full` : sans parent flex, il ne prend ni la
        //   largeur ni la hauteur et la colonne se calcule sur 0 px.
        div()
            .size_full()
            .flex()
            .bg(couleur(theme.palette.background_primary))
            .text_color(couleur(theme.palette.text_normal))
            .font_family(FAMILLE_INTERFACE)
            .text_size(px(taille))
            .line_height(px(taille * INTERLIGNE_TEXTE))
            .child(EditeurDeBase::new(&self.etat))
    }
}
