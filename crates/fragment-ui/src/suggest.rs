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
    AppContext as _, Context, FocusHandle, Font, FontFallbacks, FontStyle, FontWeight,
    HighlightStyle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled, StyledText, TextRun, Window, div, relative,
};

use crate::echelle::{calees, u};
use crate::flottant::arrondir;
use crate::icones;
use crate::modal::{Demande, Fermeture, Ouverture, largeur_boite, ouvrir};
use crate::polices::{FAMILLE_INTERFACE, caracteristiques};
use crate::rendu::melange::bord_unique;
use crate::rendu::ombre::ombrer_au_calque;
use crate::rendu::texte::texte_exact;
use crate::theme_global::{couleur, theme_ou_defaut};
use fragment_theme::Aplat;

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
                t.palette.aplat(Aplat::SurvolActif, fond_boite)
            } else {
                fond_boite
            };
            let fond_puce = t.palette.aplat(Aplat::Survol, fond_rangee);
            // ★ La puce mesure 11 × 1,5 + 2 × (1 + 1) = 20,5 px, centrée dans la ligne de 21 px du
            // titre : Chromium la pose à 0,25 px du haut et arrondit chaque arête (7,25 → 7,
            // 27,75 → 28), soit 21 px peints. GPUI arrondit l'interligne de 16,5 à 16 et peint 20 px.
            // On lui donne la hauteur calée (21) moins rembourrage et bordure : 17 px d'interligne ;
            // le texte retombe au même endroit (demi-interligne 8,5 depuis 161 contre 8,25 depuis
            // 161,25).
            let cadre_puce = 2.0 * (1.0 + t.forme.border_width);
            let haut_puce = t.typo.font_ui_smaller * t.typo.line_height_normal + cadre_puce;
            let ligne_titre = t.typo.font_ui_normal * t.typo.line_height_normal;
            let origine_puce = (ligne_titre - haut_puce).max(0.0) / 2.0;
            let interligne_puce = calees(cx, origine_puce, &[haut_puce])
                .first()
                .map_or(u(cx, haut_puce), |h| *h)
                - u(cx, cadre_puce);
            let mut aux = div()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .gap(u(cx, t.forme.size_2_3));
            for a in &ligne.aux {
                aux = match a {
                    Auxiliaire::Raccourci(texte) => aux.child(arrondir(
                        div()
                            .px(u(cx, t.forme.size_2_3))
                            .py(u(cx, 1.0))
                            .line_height(interligne_puce)
                            .bg(couleur(fond_puce))
                            .border(bordure)
                            .border_color(contour)
                            .text_color(attenue)
                            .text_size(u(cx, t.typo.font_ui_smaller))
                            .whitespace_nowrap()
                            .child(texte.clone()),
                        cx,
                        u(cx, t.forme.radius_s),
                        bordure,
                    )),
                    Auxiliaire::Epingle => aux.child(
                        div().flex().text_color(attenue).child(
                            crate::icone::icone_image()
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
                // Le fond sélectionné est arrondi : ses coins sont déclarés (ADR-009, bord d'arrondi).
                rangee = arrondir(
                    rangee.bg(couleur(fond_rangee)),
                    cx,
                    u(cx, t.forme.radius_sm),
                    u(cx, 0.0),
                );
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
        // ★ Chromium pose les morceaux de la légende en positions FRACTIONNAIRES (largeurs exactes du
        // texte, écarts de 9 et 6 px) ; GPUI arrondit chaque feuille de texte au pixel supérieur, d'où
        // +1 px par morceau, +2 à +3 px au bout de la ligne. Les largeurs sont donc mesurées (avec les
        // replis de Chromium pour les touches) et calées arête par arête (`echelle::calees`).
        let taille = t.typo.font_ui_small;
        let mesurer = |texte: &str, graisse: FontWeight| -> f32 {
            let police = Font {
                family: FAMILLE_INTERFACE.into(),
                features: caracteristiques(),
                fallbacks: Some(replis_chromium()),
                weight: graisse,
                style: FontStyle::Normal,
            };
            let run = TextRun {
                len: texte.len(),
                font: police,
                color: Default::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let zoom = u(cx, 1.0);
            let l = window
                .text_system()
                .layout_line(texte, u(cx, taille), &[run], None);
            f32::from(l.width) / f32::from(zoom)
        };
        let mut segments = Vec::new();
        for (k, ins) in self.parametres.instructions.iter().enumerate() {
            if k > 0 {
                segments.push(t.forme.size_2_9);
            }
            segments.push(
                mesurer(&ins.commande, FontWeight::SEMIBOLD)
                    + AVANCE_GRAS_SYNTHETIQUE * glyphes_en_gras_synthetique(&ins.commande) as f32,
            );
            segments.push(t.forme.size_2_3);
            segments.push(mesurer(&ins.but, FontWeight::NORMAL));
        }
        let largeurs = calees(cx, t.forme.size_2_7, &segments);
        // ★ fr-p4.1.2 : chaque morceau part de l'arête CALÉE ; Chromium peint son texte à l'arête
        // exacte. Le résidu (exact − calé, px zoomés) sort du même calcul que `calees`.
        let zoom = f32::from(u(cx, 1.0));
        let residu = |m: usize| -> f32 {
            let exact = (t.forme.size_2_7 + segments[..m].iter().sum::<f32>()) * zoom;
            let cale = (t.forme.size_2_7 * zoom + 0.5 + 1e-3).floor()
                + largeurs[..m].iter().map(|l| f32::from(*l)).sum::<f32>();
            exact - cale
        };
        let mut legende = div()
            .flex_none()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_y(u(cx, t.forme.size_4_1))
            .py(u(cx, t.forme.size_4_2))
            .px(u(cx, t.forme.size_2_7))
            .border_t(bordure)
            .border_color(contour)
            .text_color(attenue)
            .text_size(u(cx, t.typo.font_ui_small));
        for (k, ins) in self.parametres.instructions.iter().enumerate() {
            let base = if k > 0 { 4 * k - 1 } else { 0 };
            let ecart = if k > 0 { largeurs[base] } else { u(cx, 0.0) };
            let i = if k > 0 { base + 1 } else { 0 };
            let mut commande = div()
                .flex_none()
                .w(largeurs[i])
                .whitespace_nowrap()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(normal)
                .child(texte_exact(ins.commande.clone()).decalage(residu(i), 0.0));
            if glyphes_en_gras_synthetique(&ins.commande) > 0 {
                // Le gras synthétique : le même texte repeint décalé de l'épaississement de Skia.
                commande = commande.relative().child(
                    div()
                        .absolute()
                        .top(u(cx, 0.0))
                        .left(u(cx, epaississement_gras_synthetique(taille)))
                        .whitespace_nowrap()
                        .child(texte_exact(ins.commande.clone()).decalage(residu(i), 0.0)),
                );
            }
            legende = legende.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .ml(ecart)
                    .child(repli_chromium(commande))
                    .child(
                        div()
                            .flex_none()
                            .ml(largeurs[i + 1])
                            .w(largeurs[i + 2])
                            .whitespace_nowrap()
                            .child(texte_exact(ins.but.clone()).decalage(residu(i + 2), 0.0)),
                    ),
            );
        }

        let boite = div()
            .id("prompt")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| this.sur_touche(ev, cx)))
            .relative()
            .flex()
            .flex_col()
            .w(largeur_boite(window, cx, 700.0, 2.0 * t.forme.border_width))
            .max_h(hauteur_max + u(cx, 2.0 * t.forme.border_width))
            .text_color(normal)
            .font_family(FAMILLE_INTERFACE)
            .font_features(caracteristiques())
            .text_size(u(cx, t.typo.font_ui_normal))
            .line_height(relative(t.typo.line_height_normal))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        // Un seul anticrénelage au bord (bordure opaque, fond en retrait : `rendu::melange`).
        let mut boite = bord_unique(
            boite,
            cx,
            (t.forme.radius_l, t.forme.border_width),
            t.palette.tab_outline_color,
            t.palette.workspace_bg,
            false,
        )
        .child(entree)
        .child(resultats);
        boite = ombrer_au_calque(
            boite,
            cx,
            &t.palette.shadow_l,
            "--shadow-l",
            u(cx, t.forme.radius_l),
            bordure,
            t.palette.background_modifier_cover,
        );
        if !self.parametres.instructions.is_empty() {
            boite = boite.child(legende);
        }
        boite
    }
}

/// ★ Les polices de repli de Chromium sous Windows pour les touches de la légende (`↑↓`, `↵`), absentes
/// de DM Sans. La pile de Fragment (`'DM Sans', system-ui, …, 'Segoe UI'`) donne `↑↓` à Segoe UI
/// (`system-ui`) ; `↵` (U+21B5), absent de Segoe UI, tombe sur le repli système de Chromium, Cambria
/// Math (vérifié sur la référence : flèche de 7 lignes posée 1 px au-dessus de la ligne de base, la
/// forme de Cambria, et non celle de Segoe UI Symbol que choisit le repli de DirectWrite appelé par
/// GPUI). Segoe UI vient en premier pour que `↑↓` y reste.
fn repli_chromium<E: Styled>(mut e: E) -> E {
    e.text_style().font_fallbacks = Some(replis_chromium());
    e
}

/// ★ Les caractères que Chromium dessine en GRAS SYNTHÉTIQUE dans la légende : `↵` tombe sur Cambria
/// Math, qui n'a pas de graisse 600 ; Chromium (Blink) l'épaissit (`SkFont::setEmbolden`) et lui
/// ajoute 1 px d'avance. Mesuré dans Electron (celui de Fragment) : `↵` à 12 px fait 7 px en 400, 8 px
/// en 600, dix `↵` 70 contre 80 px. GPUI, lui, rend le glyphe de repli maigre et sans l'avance : la
/// suite de la légende glissait de −1 à −1,6 px (barycentres, run 43014977166). `↑↓` (Segoe UI, qui a
/// sa graisse 600) n'est pas concerné : 11,34 px dans les deux graisses.
const GRAS_SYNTHETIQUE: &[char] = &['↵'];

/// L'avance ajoutée par glyphe en gras synthétique (Blink, `SimpleFontData` : 1 px).
const AVANCE_GRAS_SYNTHETIQUE: f32 = 1.0;

/// Le nombre de glyphes de `texte` que Chromium épaissit synthétiquement.
fn glyphes_en_gras_synthetique(texte: &str) -> usize {
    texte
        .chars()
        .filter(|c| GRAS_SYNTHETIQUE.contains(c))
        .count()
}

/// L'épaississement (px) du gras synthétique de Skia à la taille `taille` : `taille × k`, k
/// interpolé de 1/24 (9 px) à 1/32 (36 px) (`kStdFakeBoldInterpKeys` / `Values`), soit 0,49 px à
/// 12 px. Le glyphe est repeint décalé d'autant : GPUI n'expose pas l'épaississement de contour.
fn epaississement_gras_synthetique(taille: f32) -> f32 {
    let t = ((taille - 9.0) / (36.0 - 9.0)).clamp(0.0, 1.0);
    taille * (1.0 / 24.0 + t * (1.0 / 32.0 - 1.0 / 24.0))
}

fn replis_chromium() -> FontFallbacks {
    FontFallbacks::from_fonts(vec!["Segoe UI".to_owned(), "Cambria Math".to_owned()])
}

/// Enregistre les commandes de scène `pixels:suggest` : aucune, la palette porte les siennes.
pub fn commandes_de_scene(_commandes: &CommandManager) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_retour_chariot_est_en_gras_synthetique_et_pas_les_fleches() {
        assert_eq!(glyphes_en_gras_synthetique("↵"), 1);
        assert_eq!(glyphes_en_gras_synthetique("↑↓"), 0);
        assert_eq!(glyphes_en_gras_synthetique("esc"), 0);
        let e = epaississement_gras_synthetique(12.0);
        assert!((e - 0.486).abs() < 0.001, "{e}");
    }

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
