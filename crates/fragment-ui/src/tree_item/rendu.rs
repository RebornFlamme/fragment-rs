// Port de app/src/ui/views/TreeItem.tsx et treeItem.css
//! Le rendu d'un item d'arbre : une ligne (retrait, chevron, icône, titre tronqué, états) et ses
//! enfants repliables dans un conteneur bordé du guide d'indentation.
//!
//! Aucun modèle de données : l'appelant décrit chaque ligne par une [`Ligne`] et fournit les
//! enfants déjà rendus (rien quand le dossier est replié : le conteneur, et son guide, disparaissent
//! avec eux). La profondeur sert UNIQUEMENT au calcul marge négative / rembourrage.

use std::rc::Rc;

use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement as _, IntoElement as _,
    MouseButton, MouseDownEvent, ParentElement as _, Pixels, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use crate::echelle::{calees, u};
use crate::flottant::{arrondir, arrondir_au_survol, texte};
use crate::icones;
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::theme_global::{couleur, theme_ou_defaut};
use crate::tree_item::couleurs::{Etat, couleur_guide, couleurs_ligne};
use crate::tree_item::geometrie::{
    COTE_CHEVRON, GOUTTIERE, INTERLIGNE, LARGEUR_GUIDE, MARGE_ENFANTS, OPACITE_CHEVRON, PAS,
    RETRAIT_ENFANTS, chevron_gauche, chevron_haut, hauteur_ligne, marge_gauche, retrait_titre,
};

/// L'opacité de l'accent du fond de sélection (`::selection` de `index.css`), en pourcents.
const POURCENT_SELECTION: f32 = 35.0;

/// Le groupe GPUI de la ligne : le survol de la ligne colore son chevron.
const GROUPE_LIGNE: &str = "tree-item-ligne";

/// Le rappel d'un clic.
pub type RappelClic = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
/// Le rappel d'un appui du bouton droit (menu contextuel).
pub type RappelMenu = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

/// Ce que l'appelant dit d'une ligne.
pub struct Ligne {
    id: ElementId,
    profondeur: usize,
    repliable: bool,
    replie: bool,
    etat: Etat,
    icone: Option<String>,
    titre: SharedString,
    panneau: Option<Couleur>,
    hauteur: Option<Pixels>,
    sonde: Option<String>,
    flair: Option<SharedString>,
    titre_selectionne: bool,
    sur_chevron: Option<RappelClic>,
    sur_clic: Option<RappelClic>,
    sur_menu: Option<RappelMenu>,
}

impl Ligne {
    /// Une ligne de titre `titre`, à la racine, sans chevron.
    pub fn new(id: impl Into<ElementId>, titre: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            profondeur: 0,
            repliable: false,
            replie: false,
            etat: Etat::default(),
            icone: None,
            titre: titre.into(),
            panneau: None,
            hauteur: None,
            sonde: None,
            flair: None,
            titre_selectionne: false,
            sur_chevron: None,
            sur_clic: None,
            sur_menu: None,
        }
    }

    /// La profondeur (0 à la racine).
    pub fn profondeur(mut self, profondeur: usize) -> Self {
        self.profondeur = profondeur;
        self
    }

    /// Pose un chevron ; `replie` choisit sa pointe (droite replié, bas déplié).
    pub fn repliable(mut self, replie: bool) -> Self {
        self.repliable = true;
        self.replie = replie;
        self
    }

    /// L'élément actif (fichier ouvert).
    pub fn actif(mut self, actif: bool) -> Self {
        self.etat.actif = actif;
        self
    }

    /// La cible d'un dépôt de fichier en cours.
    pub fn cible_depot(mut self, cible: bool) -> Self {
        self.etat.cible_depot = cible;
        self
    }

    /// Une icône facultative (nom du registre `icones`) avant le titre.
    pub fn icone(mut self, nom: impl Into<String>) -> Self {
        self.icone = Some(nom.into());
        self
    }

    /// Le fond opaque du panneau, sur lequel se composent les fonds translucides (par défaut
    /// `background_secondary`, celui des barres latérales).
    pub fn panneau(mut self, fond: Couleur) -> Self {
        self.panneau = Some(fond);
        self
    }

    /// Impose la hauteur de la ligne (celle que rend [`hauteurs_calees`] dans une pile).
    pub fn hauteur(mut self, hauteur: Pixels) -> Self {
        self.hauteur = Some(hauteur);
        self
    }

    /// Un nom pour les mesures de test : `<nom>/ligne`, `<nom>/chevron`, `<nom>/titre`,
    /// `<nom>/enfants`.
    pub fn sonde(mut self, nom: impl Into<String>) -> Self {
        self.sonde = Some(nom.into());
        self
    }

    /// Le titre entièrement sélectionné (champ de renommage qui vient de s'ouvrir : `select()`) :
    /// le fond `::selection` de Fragment derrière le texte.
    pub fn titre_selectionne(mut self, oui: bool) -> Self {
        self.titre_selectionne = oui;
        self
    }

    /// Le texte de droite (`tree-item-flair`) : le numéro de page d'une entrée de table des
    /// matières. Il ne rétrécit pas, le titre cède la place ; sonde `<nom>/flair`.
    pub fn flair(mut self, texte: impl Into<SharedString>) -> Self {
        self.flair = Some(texte.into());
        self
    }

    /// Le rappel du clic sur la gouttière du chevron (24 px de large, pleine hauteur de ligne) :
    /// il consomme le clic, celui de la ligne n'a pas lieu. Sans effet sans [`Self::repliable`].
    pub fn sur_chevron(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.sur_chevron = Some(Rc::new(f));
        self
    }

    /// Le rappel du clic.
    pub fn sur_clic(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.sur_clic = Some(Rc::new(f));
        self
    }

    /// Le rappel du bouton droit.
    pub fn sur_menu(
        mut self,
        f: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.sur_menu = Some(Rc::new(f));
        self
    }
}

/// Les hauteurs entières de `n` lignes empilées depuis l'arête haute exacte `origine` (logique) :
/// leur cumul retombe sur l'arrondi de chaque arête exacte, comme Chromium.
pub fn hauteurs_calees(cx: &App, origine: f32, n: usize) -> Vec<Pixels> {
    let t = theme_ou_defaut(cx);
    let h = hauteur_ligne(t.forme.size_4_1, t.typo.font_ui_small);
    calees(cx, origine, &vec![h; n])
}

/// Rend l'item : sa ligne puis, s'il y en a, ses `enfants` dans le conteneur d'indentation.
pub fn rendre_item(ligne: Ligne, enfants: Vec<AnyElement>, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let panneau = ligne.panneau.unwrap_or(c.background_secondary);
    let sonde = ligne.sonde.clone();
    let nom = move |suffixe: &str| sonde.as_ref().map(|s| format!("{s}/{suffixe}"));

    let repos = couleurs_ligne(c, ligne.etat, false, panneau);
    let survol = couleurs_ligne(c, ligne.etat, true, panneau);
    let hauteur = ligne
        .hauteur
        .unwrap_or_else(|| u(cx, hauteur_ligne(f.size_4_1, t.typo.font_ui_small)));

    let mut row = div()
        .id(ligne.id)
        .relative()
        .flex()
        .items_center()
        .h(hauteur)
        // Le trick de la marge négative : la boîte part du bord du panneau à toute profondeur.
        .ml(u(cx, marge_gauche(ligne.profondeur)))
        .pl(u(cx, retrait_titre(ligne.profondeur)))
        .pr(u(cx, f.size_2_3))
        .cursor_default()
        .group(GROUPE_LIGNE)
        .text_color(couleur(repos.texte));
    row = texte(row, cx, t.typo.font_ui_small, INTERLIGNE);
    if let Some(fond) = repos.fond {
        // Le fond est connu : le coin arrondi est déclaré (ADR-009, bord d'arrondi).
        row = arrondir(row.bg(couleur(fond)), cx, u(cx, f.radius_s), u(cx, 0.0));
    } else {
        // Le fond de survol, seul peint : son arrondi est déclaré quand le pointeur est dessus.
        row = arrondir_au_survol(row, cx, u(cx, f.radius_s));
    }
    let (fond_survol, texte_survol) = (survol.fond.map(couleur), couleur(survol.texte));
    row = row.hover(move |s| {
        let s = s.text_color(texte_survol);
        match fond_survol {
            Some(fond) => s.bg(fond),
            None => s,
        }
    });
    if let Some(nom) = nom("ligne") {
        row = row.debug_selector(move || nom);
    }
    if let Some(sur_clic) = ligne.sur_clic {
        row = row.on_click(move |e, w, app| sur_clic(e, w, app));
    }
    if let Some(sur_menu) = ligne.sur_menu {
        row = row.on_mouse_down(MouseButton::Right, move |e, w, app| sur_menu(e, w, app));
    }

    if ligne.repliable {
        // Le triangle est posé en absolu à l'arête arrondie de sa position exacte (fractionnaire).
        let gauche = calees(cx, 0.0, &[chevron_gauche(ligne.profondeur)])[0];
        let haut = calees(cx, 0.0, &[chevron_haut(f.size_4_1, t.typo.font_ui_small)])[0];
        let icone = if ligne.replie {
            "right-triangle-replie"
        } else {
            "right-triangle"
        };
        let mut chevron = div()
            .absolute()
            .left(gauche)
            .top(haut)
            .w(u(cx, COTE_CHEVRON))
            .h(u(cx, COTE_CHEVRON))
            .opacity(OPACITE_CHEVRON)
            // Le triangle prend la couleur de la ligne (`currentColor`), survol compris.
            // ★ Un `crate::icone::icone_image()` de GPUI n'hérite PAS du `text_color` de son parent : sans couleur
            // propre, il ne peint rien (chevrons absents, run 43351537908). Le survol passe par
            // le groupe de la ligne.
            .child(
                crate::icone::icone_image()
                    .path(icones::chemin(icone))
                    .size(u(cx, COTE_CHEVRON))
                    .flex_none()
                    .text_color(couleur(repos.texte))
                    .group_hover(GROUPE_LIGNE, move |s| s.text_color(texte_survol)),
            );
        if let Some(nom) = nom("chevron") {
            chevron = chevron.debug_selector(move || nom);
        }
        row = row.child(chevron);
        if let Some(sur_chevron) = ligne.sur_chevron {
            // La zone de clic : toute la gouttière, posée par-dessus le triangle.
            row = row.child(
                div()
                    .id("chevron-clic")
                    .absolute()
                    .left(u(cx, ligne.profondeur as f32 * PAS))
                    .top(u(cx, 0.0))
                    .w(u(cx, GOUTTIERE))
                    .h_full()
                    .on_click(move |e, w, app| {
                        sur_chevron(e, w, app);
                        app.stop_propagation();
                    }),
            );
        }
    }
    if let Some(nom_icone) = &ligne.icone {
        // ★ Non définie par le CSS de Fragment (l'explorateur et la table des matières n'en
        // ont pas) : taille `icon_s` et écart `size_4_1` provisoires, voir parite.d.
        row = row.child(
            div().flex_none().mr(u(cx, f.size_4_1)).child(
                crate::icone::icone_image()
                    .path(icones::chemin(nom_icone))
                    .size(u(cx, f.icon_s))
                    .flex_none(),
            ),
        );
    }
    let mut titre = div()
        .flex_1()
        .min_w(u(cx, 0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis();
    titre = if ligne.titre_selectionne {
        // ★ `::selection` : l'accent à 35 % sur le panneau, composé comme un REMPLISSAGE de
        // Chromium (`sur_chromium` : 184 et 227 en clair, là où le mélange exact donne 183 et 226).
        // Le rectangle couvre la ligne de texte, sur la largeur du libellé arrondie à l'arête.
        let largeur = largeur_texte(cx, &ligne.titre, FontWeight::NORMAL, t.typo.font_ui_small);
        titre.child(
            div()
                .w(largeur_ajustee(cx, largeur, 0.0))
                .bg(couleur(
                    c.color_accent.sur_chromium(POURCENT_SELECTION, panneau),
                ))
                .child(ligne.titre),
        )
    } else {
        titre.child(ligne.titre)
    };
    if let Some(nom) = nom("titre") {
        titre = titre.debug_selector(move || nom);
    }
    row = row.child(titre);
    if let Some(flair) = ligne.flair {
        let mut bloc = texte(
            div().flex_none().ml(u(cx, f.size_4_2)).opacity(0.6),
            cx,
            t.typo.font_ui_smaller,
            INTERLIGNE,
        )
        .child(flair);
        if let Some(nom) = nom("flair") {
            bloc = bloc.debug_selector(move || nom);
        }
        row = row.child(bloc);
    }

    let mut item = div().flex().flex_col().child(row);
    if !enfants.is_empty() {
        let mut conteneur = div()
            .flex()
            .flex_col()
            .ml(u(cx, MARGE_ENFANTS))
            .border_l(u(cx, LARGEUR_GUIDE))
            .border_color(couleur(couleur_guide(c, panneau)))
            .pl(u(cx, RETRAIT_ENFANTS))
            .children(enfants);
        if let Some(nom) = nom("enfants") {
            conteneur = conteneur.debug_selector(move || nom);
        }
        item = item.child(conteneur);
    }
    item.into_any_element()
}
