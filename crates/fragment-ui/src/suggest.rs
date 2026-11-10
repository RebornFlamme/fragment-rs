//! Les modales de suggestion : un champ, une liste, un choix.
//!
//! Le squelette est imposé (le champ, le clavier, la sélection cyclique, le défilement) ; celui qui
//! s'en sert remplit trois trous, ceux du trait [`Suggeres`] : d'où viennent les suggestions, comment
//! dessiner une ligne, que faire du choix. [`ElementsFlous`] en fait la version branchée sur la
//! recherche floue du cœur (filtre, tri par score, surlignage fournis) : c'est le cas de la palette.
//!
//! Tout ce qui ne dépend pas de GPUI est pur et testé sans fenêtre : le découpage de surlignage
//! ([`surligner`]), la liste et sa sélection ([`ListeSuggestions`]), le filtrage ([`filtrer_flou`]).
//!
//! ★ Le champ de saisie est volontairement minimal : texte inséré en fin, retour arrière. Pas de
//! curseur déplaçable, de sélection ni de collage (dette suivie dans docs/parite-pixels.md).

use std::ops::Range;

use fragment_core::{
    CommandManager, FuzzyMatch, InputEvent, KeyEvent, Modifiers as ModsCoeur, PointerEvent,
    prepare_fuzzy_search,
};
use gpui_kit::{
    AppContext as _, Context, FocusHandle, FontWeight, HighlightStyle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, Render, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, StyledText, Window, div, relative, svg,
};

use crate::echelle::u;
use crate::flottant::ombres;
use crate::icones;
use crate::modal::{Demande, Fermeture, Ouverture, largeur_boite, ouvrir};
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

// ── Les données d'une ligne ───────────────────────────────────────────────────

/// Un morceau du titre d'une ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Le texte.
    pub texte: String,
    /// Grisé (le préfixe du plugin, `.suggestion-prefix`).
    pub prefixe: bool,
    /// Surligné (une plage trouvée, `.suggestion-highlight`).
    pub surligne: bool,
}

/// Ce qui s'affiche à droite d'une ligne (`.suggestion-aux`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auxiliaire {
    /// Un raccourci, dans sa puce `kbd`.
    Raccourci(String),
    /// L'épingle d'une commande épinglée.
    Epingle,
}

/// Une ligne de la liste, décrite sans GPUI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ligne {
    /// Le titre, morceau par morceau.
    pub segments: Vec<Segment>,
    /// La partie droite.
    pub aux: Vec<Auxiliaire>,
}

/// Une instruction de légende : la touche et ce qu'elle fait (`↑↓` « naviguer »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// La touche, telle qu'affichée.
    pub commande: String,
    /// Ce qu'elle fait.
    pub but: String,
}

impl Instruction {
    /// Une instruction.
    pub fn new(commande: &str, but: &str) -> Self {
        Self {
            commande: commande.to_owned(),
            but: but.to_owned(),
        }
    }
}

// ── Surlignage ────────────────────────────────────────────────────────────────

/// Découpe `texte` selon les plages trouvées : `(morceau, surligné)`.
///
/// Les plages sont des unités de code UTF-16 du texte CHERCHÉ ; `decalage` est la position de `texte`
/// dans ce texte quand on n'en rend qu'une sous-chaîne (la palette cherche sur « Plugin: Commande »
/// mais rend le préfixe et le nom séparément). Une plage hors de la sous-chaîne, ou qui en chevauche
/// une précédente, est ignorée.
pub fn surligner(texte: &str, plages: &[(usize, usize)], decalage: usize) -> Vec<(String, bool)> {
    let unites: Vec<u16> = texte.encode_utf16().collect();
    let n = unites.len();
    let tranche = |a: usize, b: usize| String::from_utf16_lossy(&unites[a..b]);
    let mut sortie = Vec::new();
    let mut curseur = 0;
    for &(de, a) in plages {
        let debut = de.saturating_sub(decalage);
        let fin = a.saturating_sub(decalage).min(n);
        if fin <= debut || debut < curseur {
            continue;
        }
        if debut > curseur {
            sortie.push((tranche(curseur, debut), false));
        }
        sortie.push((tranche(debut, fin), true));
        curseur = fin;
    }
    if curseur < n {
        sortie.push((tranche(curseur, n), false));
    }
    sortie
}

// ── La liste et sa sélection ──────────────────────────────────────────────────

/// Les suggestions rendues et l'indice sélectionné.
#[derive(Debug, Clone, PartialEq)]
pub struct ListeSuggestions<T> {
    valeurs: Vec<T>,
    selection: Option<usize>,
    limite: usize,
}

impl<T> ListeSuggestions<T> {
    /// Une liste vide, qui garde au plus `limite` suggestions.
    pub fn new(limite: usize) -> Self {
        Self {
            valeurs: Vec::new(),
            selection: None,
            limite,
        }
    }

    /// Remplace les suggestions (tronquées à la limite) ; la première est sélectionnée.
    pub fn remplacer(&mut self, mut valeurs: Vec<T>) {
        valeurs.truncate(self.limite);
        self.selection = if valeurs.is_empty() { None } else { Some(0) };
        self.valeurs = valeurs;
    }

    /// Déplace la sélection de `delta`, de façon CYCLIQUE : ↓ en bas de liste remonte en haut.
    pub fn deplacer(&mut self, delta: isize) {
        let n = self.valeurs.len();
        if n == 0 {
            return;
        }
        let courant = self.selection.map_or(-1, |i| i as isize);
        let n = n as isize;
        self.selection = Some(((courant + delta).rem_euclid(n)) as usize);
    }

    /// Sélectionne l'indice `i` (survol) ; sans effet hors de la liste.
    pub fn selectionner(&mut self, i: usize) {
        if i < self.valeurs.len() {
            self.selection = Some(i);
        }
    }

    /// L'indice sélectionné.
    pub fn indice(&self) -> Option<usize> {
        self.selection
    }

    /// La suggestion sélectionnée.
    pub fn selectionnee(&self) -> Option<&T> {
        self.selection.and_then(|i| self.valeurs.get(i))
    }

    /// Les suggestions.
    pub fn valeurs(&self) -> &[T] {
        &self.valeurs
    }

    /// Aucune suggestion ?
    pub fn est_vide(&self) -> bool {
        self.valeurs.is_empty()
    }
}

// ── Les trous ─────────────────────────────────────────────────────────────────

/// Les trois trous d'une modale de suggestion.
pub trait Suggeres: 'static {
    /// Ce qu'une suggestion porte.
    type Valeur: Clone + 'static;

    /// D'où viennent les suggestions pour cette requête.
    fn suggestions(&mut self, requete: &str) -> Vec<Self::Valeur>;

    /// Comment dessiner une suggestion.
    fn ligne(&self, valeur: &Self::Valeur) -> Ligne;

    /// Que faire du choix. La modale est déjà marquée fermée : on peut en ouvrir une autre.
    fn choisir(&mut self, valeur: Self::Valeur, evenement: Option<InputEvent>);
}

/// Le filtre flou : garde les éléments qui correspondent, triés par score décroissant.
///
/// ★ Requête VIDE : pas de tri. L'ordre des éléments a alors le dernier mot, ce qui permet à la
/// palette de montrer les épinglées puis les récentes avant la première frappe. Le tri est stable.
pub fn filtrer_flou<T: Clone>(
    requete: &str,
    elements: Vec<T>,
    texte: impl Fn(&T) -> String,
) -> Vec<FuzzyMatch<T>> {
    let chercher = prepare_fuzzy_search(requete);
    let mut resultats: Vec<FuzzyMatch<T>> = elements
        .into_iter()
        .filter_map(|item| {
            let m = chercher.search(&texte(&item))?;
            Some(FuzzyMatch { item, r#match: m })
        })
        .collect();
    if !requete.trim().is_empty() {
        resultats.sort_by(|a, b| {
            b.r#match
                .score
                .partial_cmp(&a.r#match.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    resultats
}

/// Les trois trous de la version floue : quels éléments, quel texte, que faire du choix.
pub trait ElementsFlous: 'static {
    /// L'élément.
    type Element: Clone + 'static;

    /// Les éléments à filtrer (appelé à chaque frappe : mettre en cache si c'est coûteux).
    fn elements(&mut self) -> Vec<Self::Element>;

    /// Le texte cherché pour un élément.
    fn texte(&self, element: &Self::Element) -> String;

    /// Le choix, avec l'élément nu.
    fn choisir_element(&mut self, element: Self::Element, evenement: Option<InputEvent>);

    /// Défaut : le texte, plages trouvées surlignées.
    fn ligne(&self, valeur: &FuzzyMatch<Self::Element>) -> Ligne {
        let texte = self.texte(&valeur.item);
        Ligne {
            segments: surligner(&texte, &valeur.r#match.matches, 0)
                .into_iter()
                .map(|(texte, surligne)| Segment {
                    texte,
                    prefixe: false,
                    surligne,
                })
                .collect(),
            aux: Vec::new(),
        }
    }
}

/// L'adaptateur qui fait d'un [`ElementsFlous`] un [`Suggeres`].
pub struct Flou<F>(pub F);

impl<F: ElementsFlous> Suggeres for Flou<F> {
    type Valeur = FuzzyMatch<F::Element>;

    fn suggestions(&mut self, requete: &str) -> Vec<Self::Valeur> {
        let elements = self.0.elements();
        let source = &self.0;
        filtrer_flou(requete, elements, |e| source.texte(e))
    }

    fn ligne(&self, valeur: &Self::Valeur) -> Ligne {
        self.0.ligne(valeur)
    }

    fn choisir(&mut self, valeur: Self::Valeur, evenement: Option<InputEvent>) {
        self.0.choisir_element(valeur.item, evenement);
    }
}

// ── La vue ────────────────────────────────────────────────────────────────────

/// Les réglages d'une modale de suggestion.
#[derive(Debug, Clone)]
pub struct ParametresSuggestion {
    /// Le texte du champ vide.
    pub placeholder: String,
    /// La légende du bas.
    pub instructions: Vec<Instruction>,
    /// Nombre maximal de suggestions rendues.
    pub limite: usize,
    /// Le texte d'état vide.
    pub texte_vide: String,
    /// Dessiner le curseur du champ (faux dans les scènes : Fragment le masque pour ses captures).
    pub curseur: bool,
}

impl Default for ParametresSuggestion {
    fn default() -> Self {
        Self {
            placeholder: String::new(),
            instructions: Vec::new(),
            limite: 100,
            texte_vide: "Aucun résultat.".to_owned(),
            curseur: true,
        }
    }
}

/// La modale de suggestion (`.prompt`).
pub struct ModaleSuggestion<S: Suggeres> {
    source: S,
    parametres: ParametresSuggestion,
    requete: String,
    liste: ListeSuggestions<S::Valeur>,
    focus: FocusHandle,
    defilement: ScrollHandle,
    fermeture: Fermeture,
}

/// L'événement du cœur pour une frappe (lu par les commandes via `last_event`).
fn evenement_clavier(touche: &str, mods: &Modifiers) -> InputEvent {
    InputEvent::Key(KeyEvent::new(touche).with_mods(mods_coeur(mods)))
}

fn mods_coeur(m: &Modifiers) -> ModsCoeur {
    ModsCoeur {
        ctrl: m.control,
        meta: m.platform,
        alt: m.alt,
        shift: m.shift,
    }
}

impl<S: Suggeres> ModaleSuggestion<S> {
    /// Demande l'ouverture. Une modale de même `cle` déjà ouverte fait ignorer la demande.
    pub fn ouvrir(cle: &'static str, source: S, parametres: ParametresSuggestion) {
        ouvrir(Demande::new(Some(cle), move |_, cx, fermeture| {
            let vue = cx.new(|cx| {
                let mut modale = Self {
                    liste: ListeSuggestions::new(parametres.limite),
                    source,
                    parametres,
                    requete: String::new(),
                    focus: cx.focus_handle(),
                    defilement: ScrollHandle::new(),
                    fermeture,
                };
                // Chaque ouverture repart d'un champ vide.
                modale.mettre_a_jour();
                modale
            });
            let focus = vue.read(cx).focus.clone();
            Ouverture {
                vue: vue.into(),
                focus,
            }
        }));
    }

    fn mettre_a_jour(&mut self) {
        let valeurs = self.source.suggestions(&self.requete);
        self.liste.remplacer(valeurs);
        self.defilement.scroll_to_item(0);
    }

    fn deplacer(&mut self, delta: isize) {
        self.liste.deplacer(delta);
        if let Some(i) = self.liste.indice() {
            self.defilement.scroll_to_item(i);
        }
    }

    /// Ferme PUIS notifie : la source peut ouvrir autre chose sans conflit de portée.
    fn choisir(&mut self, valeur: S::Valeur, evenement: InputEvent) {
        self.fermeture.demander();
        self.source.choisir(valeur, Some(evenement));
    }

    fn sur_touche(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        match k.key.as_str() {
            "escape" => self.fermeture.demander(),
            "down" => self.deplacer(1),
            "up" => self.deplacer(-1),
            "enter" => {
                if let Some(v) = self.liste.selectionnee().cloned() {
                    self.choisir(v, evenement_clavier("Enter", &k.modifiers));
                }
            }
            "backspace" => {
                self.requete.pop();
                self.mettre_a_jour();
            }
            // Tab ne sort pas de la modale : il n'y a nulle part où aller.
            "tab" => {}
            _ => {
                let raccourci = k.modifiers.control || k.modifiers.alt || k.modifiers.platform;
                match &k.key_char {
                    Some(c) if !raccourci && !c.chars().any(char::is_control) => {
                        self.requete.push_str(c);
                        self.mettre_a_jour();
                    }
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}

/// Les surlignages d'un titre : `(texte entier, plages stylées)` en OCTETS, triées, sans recouvrement.
fn titre_et_plages(
    segments: &[Segment],
    prefixe: Hsla,
    surligne: Hsla,
) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
    let mut texte = String::new();
    let mut plages = Vec::new();
    for s in segments {
        let debut = texte.len();
        texte.push_str(&s.texte);
        let fin = texte.len();
        if s.surligne {
            // Le surlignage l'emporte sur le grisé du préfixe (accent, semi-gras).
            plages.push((
                debut..fin,
                HighlightStyle {
                    color: Some(surligne),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..HighlightStyle::default()
                },
            ));
        } else if s.prefixe {
            plages.push((
                debut..fin,
                HighlightStyle {
                    color: Some(prefixe),
                    ..HighlightStyle::default()
                },
            ));
        }
    }
    (texte, plages)
}

impl<S: Suggeres> Render for ModaleSuggestion<S> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        let normal = couleur(t.palette.text_normal);
        let attenue = couleur(t.palette.text_muted);
        let contour = couleur(t.palette.tab_outline_color);
        let bordure = u(cx, t.forme.border_width);
        let hauteur_max = window.viewport_size().height * 0.70;

        // ── Le champ ──
        let mut champ = div()
            .flex()
            .flex_row()
            .items_center()
            .min_w_0()
            .flex_1()
            .text_size(u(cx, 16.0))
            .line_height(relative(t.typo.line_height_normal));
        if self.requete.is_empty() {
            let mut grise = Hsla::from(attenue);
            grise.a *= 0.7;
            champ = champ.child(
                div()
                    .text_color(grise)
                    .child(self.parametres.placeholder.clone()),
            );
        } else {
            champ = champ.child(div().child(self.requete.clone()));
        }
        if self.parametres.curseur {
            champ = champ.child(div().w(u(cx, 1.0)).h(u(cx, 19.0)).bg(normal));
        }
        let entree = div()
            .flex_none()
            .flex()
            .px(u(cx, t.forme.size_2_7))
            .py(u(cx, t.forme.size_4_3))
            .border_b(bordure)
            .border_color(contour)
            .child(champ);

        // ── Les résultats ──
        let mut resultats = div()
            .id("resultats")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.defilement)
            .p(u(cx, t.forme.size_2_3));
        if self.liste.est_vide() {
            resultats = resultats.child(
                div()
                    .py(u(cx, t.forme.size_2_7))
                    .px(u(cx, t.forme.size_2_5))
                    .text_center()
                    .text_color(attenue)
                    .child(self.parametres.texte_vide.clone()),
            );
        }
        let selection = self.liste.indice();
        for (i, valeur) in self.liste.valeurs().iter().enumerate() {
            let ligne = self.source.ligne(valeur);
            let (texte, plages) = titre_et_plages(
                &ligne.segments,
                Hsla::from(attenue),
                Hsla::from(couleur(t.palette.color_accent)),
            );
            let titre = div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(StyledText::new(texte).with_highlights(plages));
            // ★ Les fonds translucides sont composés à l'octet près (`Couleur::sur`) : GPUI les
            // mélange en flottants et tombe un niveau à côté de Chromium sur les captures.
            let fond_boite = t.palette.workspace_bg;
            let fond_rangee = if selection == Some(i) {
                t.palette.background_modifier_active_hover.sur(fond_boite)
            } else {
                fond_boite
            };
            let fond_puce = t.palette.background_modifier_hover.sur(fond_rangee);
            let mut aux = div()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .gap(u(cx, t.forme.size_2_3));
            for a in &ligne.aux {
                aux = match a {
                    Auxiliaire::Raccourci(texte) => aux.child(
                        div()
                            .px(u(cx, t.forme.size_2_3))
                            .py(u(cx, 1.0))
                            .rounded(u(cx, t.forme.radius_s))
                            .bg(couleur(fond_puce))
                            .border(bordure)
                            .border_color(contour)
                            .text_color(attenue)
                            .text_size(u(cx, t.typo.font_ui_smaller))
                            .whitespace_nowrap()
                            .child(texte.clone()),
                    ),
                    Auxiliaire::Epingle => aux.child(
                        div().flex().text_color(attenue).child(
                            svg()
                                .path(icones::chemin("pin"))
                                .size(u(cx, t.forme.icon_xs))
                                .text_color(attenue),
                        ),
                    ),
                };
            }
            let mut rangee = div()
                .id(("suggestion", i))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(u(cx, t.forme.size_2_5))
                .py(u(cx, 7.0))
                .px(u(cx, t.forme.size_2_5))
                .rounded(u(cx, t.forme.radius_sm))
                .text_color(normal);
            if selection == Some(i) {
                rangee = rangee.bg(couleur(fond_rangee));
            }
            // mousemove et non « entrée » : le survol ne vole pas la sélection quand la liste
            // défile sous une souris immobile.
            let rangee = rangee
                .on_mouse_move(cx.listener(move |this, _: &MouseMoveEvent, _, cx| {
                    if this.liste.indice() != Some(i) {
                        this.liste.selectionner(i);
                        cx.notify();
                    }
                }))
                // L'appui (pas le relâchement) choisit : avant que le champ ne perde le focus.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        if let Some(v) = this.liste.valeurs().get(i).cloned() {
                            let evt = InputEvent::Pointer(PointerEvent {
                                button: 0,
                                mods: mods_coeur(&ev.modifiers),
                            });
                            this.choisir(v, evt);
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }),
                )
                .child(titre)
                .child(aux);
            resultats = resultats.child(rangee);
        }

        // ── La légende ──
        let mut legende = div()
            .flex_none()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_x(u(cx, t.forme.size_2_9))
            .gap_y(u(cx, t.forme.size_4_1))
            .py(u(cx, t.forme.size_4_2))
            .px(u(cx, t.forme.size_2_7))
            .border_t(bordure)
            .border_color(contour)
            .text_color(attenue)
            .text_size(u(cx, t.typo.font_ui_small));
        for ins in &self.parametres.instructions {
            legende = legende.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(u(cx, t.forme.size_2_3))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(normal)
                            .child(ins.commande.clone()),
                    )
                    .child(div().child(ins.but.clone())),
            );
        }

        let mut boite = div()
            .id("prompt")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| this.sur_touche(ev, cx)))
            .relative()
            .flex()
            .flex_col()
            .w(largeur_boite(window, cx, 700.0, 2.0 * t.forme.border_width))
            .max_h(hauteur_max + u(cx, 2.0 * t.forme.border_width))
            .bg(couleur(t.palette.workspace_bg))
            .text_color(normal)
            .border(bordure)
            .border_color(contour)
            .rounded(u(cx, t.forme.radius_l))
            .shadow(ombres(cx, &t.palette.shadow_l))
            .font_family(FAMILLE_INTERFACE)
            .text_size(u(cx, t.typo.font_ui_normal))
            .line_height(relative(t.typo.line_height_normal))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(entree)
            .child(resultats);
        if !self.parametres.instructions.is_empty() {
            boite = boite.child(legende);
        }
        boite
    }
}

/// Enregistre les commandes de scène `pixels:suggest` : aucune, la palette porte les siennes.
pub fn commandes_de_scene(_commandes: &CommandManager) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn nus(v: Vec<(String, bool)>) -> Vec<(String, bool)> {
        v
    }

    fn att(l: &[(&str, bool)]) -> Vec<(String, bool)> {
        l.iter().map(|(t, s)| ((*t).to_owned(), *s)).collect()
    }

    #[test]
    fn surligner_decoupe_selon_les_plages() {
        let r = surligner("Afficher le panneau", &[(12, 19)], 0);
        assert_eq!(nus(r), att(&[("Afficher le ", false), ("panneau", true)]));
    }

    #[test]
    fn surligner_sans_plage_rend_le_texte_entier() {
        assert_eq!(nus(surligner("abc", &[], 0)), att(&[("abc", false)]));
    }

    #[test]
    fn surligner_decale_les_plages_d_une_sous_chaine() {
        // « Plugin: Commande » cherché en entier, le nom (offset 8) rendu seul.
        let r = surligner("Commande", &[(8, 11)], 8);
        assert_eq!(nus(r), att(&[("Com", true), ("mande", false)]));
    }

    #[test]
    fn surligner_ignore_une_plage_hors_de_la_sous_chaine() {
        assert_eq!(nus(surligner("nom", &[(0, 3)], 8)), att(&[("nom", false)]));
    }

    #[test]
    fn surligner_compte_en_utf16() {
        // « 😀 » vaut 2 unités : la plage (2,3) tombe sur « a ».
        let r = surligner("😀ab", &[(2, 3)], 0);
        assert_eq!(nus(r), att(&[("😀", false), ("a", true), ("b", false)]));
    }

    #[test]
    fn la_selection_est_cyclique() {
        let mut l = ListeSuggestions::new(100);
        l.remplacer(vec!["a", "b", "c"]);
        assert_eq!(l.indice(), Some(0));
        l.deplacer(-1);
        assert_eq!(l.selectionnee(), Some(&"c"));
        l.deplacer(1);
        assert_eq!(l.selectionnee(), Some(&"a"));
        l.deplacer(1);
        assert_eq!(l.selectionnee(), Some(&"b"));
    }

    #[test]
    fn la_liste_est_tronquee_a_la_limite() {
        let mut l = ListeSuggestions::new(2);
        l.remplacer(vec![1, 2, 3]);
        assert_eq!(l.valeurs(), &[1, 2]);
    }

    #[test]
    fn une_liste_vide_n_a_pas_de_selection() {
        let mut l: ListeSuggestions<u8> = ListeSuggestions::new(10);
        l.remplacer(vec![]);
        l.deplacer(1);
        assert_eq!(l.indice(), None);
    }

    #[test]
    fn filtrer_flou_trie_par_score_sauf_requete_vide() {
        let elements = vec!["zebra", "alpha", "beta"];
        let vide = filtrer_flou("", elements.clone(), |s| (*s).to_owned());
        let ordre: Vec<_> = vide.iter().map(|m| m.item).collect();
        assert_eq!(ordre, elements, "requête vide : l'ordre d'entrée est gardé");
        let filtre = filtrer_flou("al", elements, |s| (*s).to_owned());
        assert_eq!(filtre.len(), 1);
        assert_eq!(filtre[0].item, "alpha");
    }
}
