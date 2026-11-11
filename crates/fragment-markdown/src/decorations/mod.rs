// Port de app/src/ui/views/markdown/decorationPlugin.ts (buildFnMarkdownRenderer, curseurActif,
// revealAfterFirstFocus, revealOnFocusOnly, listLevel, isInsideCode) et des constantes d'editor.css.
//! Le modèle PUR du live preview en ligne : (texte, arbre, curseur, contexte) → décorations.
//!
//! Fragment décore la source avec CodeMirror (`Decoration.replace`, `.mark`, `.line`, `.widget`) ; ici
//! le même calcul rend des DONNÉES, testées sans GPUI, que la surface d'édition (`fr-t3-surface`)
//! traduit pour le fork (ADR-006) :
//! - `rangees.masques` : la syntaxe cachée hors curseur (`#`, `**`, `` ` ``, `==`, `[`…`](url)`…) ;
//! - `rangees.polices` / `rangees.hauteurs` : taille et hauteur de ligne des titres 1 à 6 (et police du
//!   code en bloc) ;
//! - `marques` : le style d'une plage, par NOM de jeton du thème (jamais une couleur) ;
//! - `lignes` : le style d'une ligne entière (titre, citation, code, barres d'indentation) ;
//! - `widgets` : ce qui REMPLACE une plage de source (case, puce, barre d'indentation, maths, lien
//!   interne, image, badge de langue, séparateur), avec sa largeur logique.
//!
//! La règle du curseur (`touches`) : un élément montre sa source quand la sélection principale
//! TOUCHE sa plage, bornes comprises, et seulement si le curseur a le droit de révéler
//! ([`curseur_actif`]) et que l'éditeur n'est pas en lecture seule.
//!
//! Hors de ce module : les blocs `$$…$$`, les embeds `![[…]]`, les tableaux, les Properties, le HTML
//! (des `StateField` chez Fragment : `blocs`), le repli et l'état survolé de la puce (`blocs`), le
//! rendu des widgets (`fr-t3-widgets-inline`).
//!
//! ★ La source d'un widget n'est PAS dans `rangees.masques` : un masque a une largeur nulle, un widget
//!   a la sienne ([`Largeur`]). C'est la surface qui décide comment le fork les dessine (T3 §3.6).

mod calcul;
mod index_lignes;
pub mod styles;

pub use calcul::calculer;
pub use styles::{StyleLigne, StyleMarque};

use crate::syntaxe::modele_rangees::ModeleRangees;
use crate::syntaxe::{Etendue, Plage};

/// Quand le curseur d'un éditeur peut-il révéler la source (`curseurActif` du TS) ?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Revelation {
    /// L'éditeur hôte sans extension : la source se révèle toujours sous le curseur.
    #[default]
    Toujours,
    /// L'éditeur d'une NOTE (`revealAfterFirstFocus`) : rien n'est révélé tant que l'utilisateur n'a
    /// pas mis son curseur dedans une première fois ; ensuite, même sans focus, comme `Toujours`.
    ApresPremierFocus,
    /// Un éditeur IMBRIQUÉ, une cellule de tableau (`revealOnFocusOnly`) : seulement s'il a le focus.
    /// Sans quoi une formule en tête de CHAQUE cellule (sélection à 0 par défaut) resterait en source.
    FocusSeulement,
}

/// L'état du curseur d'un éditeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Curseur<'a> {
    /// Les sélections, en UTF-16. La PREMIÈRE est la principale : comme Fragment
    /// (`state.selection.main`), elle seule révèle de la source.
    pub selections: &'a [Etendue],
    /// L'éditeur a le focus.
    pub focus: bool,
    /// L'éditeur a eu le focus au moins une fois depuis le chargement de CE document.
    pub a_eu_le_focus: bool,
}

/// Ce qui ne dépend pas du curseur : la configuration de l'éditeur et sa partie visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contexte<'a> {
    pub revelation: Revelation,
    /// En lecture seule, rien n'est jamais révélé : tout reste rendu.
    pub lecture_seule: bool,
    /// Les plages visibles (UTF-16, disjointes, triées : `view.visibleRanges`). Vide = tout le texte.
    pub visibles: &'a [Etendue],
    /// Largeur d'un niveau d'indentation, en colonnes (`indentUnit`, le réglage Tab size).
    pub unite_indentation: usize,
    /// Taille du texte de l'éditeur, en pixels logiques (`--font-text-size`, 16 par défaut).
    pub taille_texte: f32,
}

impl Default for Contexte<'_> {
    fn default() -> Self {
        Contexte {
            revelation: Revelation::Toujours,
            lecture_seule: false,
            visibles: &[],
            unite_indentation: 4,
            taille_texte: styles::TAILLE_TEXTE_DEFAUT,
        }
    }
}

/// Le curseur de cet éditeur peut-il révéler de la source (`curseurActif`) ?
#[must_use]
pub fn curseur_actif(revelation: Revelation, focus: bool, a_eu_le_focus: bool) -> bool {
    match revelation {
        Revelation::Toujours => true,
        Revelation::ApresPremierFocus => a_eu_le_focus,
        Revelation::FocusSeulement => focus,
    }
}

/// La règle `touches(from, to)` : la sélection `[sel.from, sel.to]` et `[from, to]` se chevauchent ou
/// se touchent (un ET des deux comparaisons ; un OU serait presque toujours vrai).
#[must_use]
pub fn touches(selection: Etendue, from: usize, to: usize) -> bool {
    selection.from <= to && from <= selection.to
}

/// Le style d'une plage en ligne (`Decoration.mark`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GenreMarque {
    /// `**…**` (`cm-ob-strong`).
    Gras,
    /// `*…*` (`cm-ob-em`).
    Italique,
    /// `~~…~~` (`cm-ob-strike`).
    Barre,
    /// `==…==` (`cm-hl`).
    Surligne,
    /// `` `…` `` (`cm-ob-code`).
    Code,
    /// `[texte](url)` en entier (`cm-ob-link`).
    Lien,
}

impl GenreMarque {
    /// Le style de la marque, en noms de jetons (`editor.css`).
    #[must_use]
    pub fn style(self) -> StyleMarque {
        styles::style_marque(self)
    }
}

/// Une plage stylée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Marque {
    pub plage: Plage,
    pub genre: GenreMarque,
}

/// Le style d'une ligne entière (`Decoration.line`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GenreLigne {
    /// Titre ATX de niveau 1 à 6 (`cm-ob-h1` … `cm-ob-h6`).
    Titre(u8),
    /// Ligne d'une citation (`cm-ob-quote`).
    Citation,
    /// Ligne d'un bloc de code délimité (`cm-ob-codeblock`).
    Code,
    /// Première ligne du bloc de code (coins hauts arrondis, `cm-ob-codeblock-first`).
    CodePremiere,
    /// Dernière ligne du bloc de code (`cm-ob-codeblock-last`).
    CodeDerniere,
    /// Ligne indentée : un trait vertical par niveau (`cm-indent-line`, `--indent-levels`).
    Indentation { niveaux: usize },
}

impl GenreLigne {
    /// Le style de la ligne, en noms de jetons (`editor.css`).
    #[must_use]
    pub fn style(self) -> StyleLigne {
        styles::style_ligne(self)
    }
}

/// Une ligne logique stylée (numéro à partir de 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecoLigne {
    pub ligne: usize,
    pub genre: GenreLigne,
}

/// Ce qu'un widget en ligne dessine à la place de sa source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GenreWidget {
    /// Case à cocher d'une tâche (`CheckboxWidget`). `marqueur` = début du `[ ]` / `[x]` (UTF-16) :
    /// cocher remplace le caractère qui suit par `x`, décocher par une espace.
    Case { cochee: bool, marqueur: usize },
    /// Puce ronde d'une liste à puces (`BulletWidget`). `debut_ligne` (UTF-16) relie la puce au repli
    /// de sa ligne ; l'état replié et le survol viennent du modèle du repli (`blocs`).
    Puce { debut_ligne: usize },
    /// Un niveau d'indentation (`IndentBarWidget`) : cellule transparente, le trait est sur la ligne.
    Indentation,
    /// Formule en ligne (`MathInlineWidget`) : `$…$`, ou `$$…$$` au milieu d'une ligne
    /// (`affichage`). Un clic sélectionne le LaTeX (`tex`), délimiteurs exclus.
    Maths { tex: String, affichage: bool },
    /// `[[cible|alias]]` (`WikiLinkWidget`) : `interieur` = entre les crochets ; `texte` = ce qui
    /// s'affiche (l'alias, sinon le chemin et le sous-chemin). La résolution (lien mort) est à l'hôte.
    LienInterne { interieur: String, texte: String },
    /// `![alt](chemin)` vers un fichier du coffre (`EmbedWidget` en ligne). `chemin` est décodé
    /// (`%20` → espace) ; un chemin externe (`http://`, `data:`) n'en fait jamais un. ★ Fragment ne le
    /// rend que si le chemin se RÉSOUT vers un fichier qu'un embed sait afficher : l'hôte filtre.
    Image { chemin: String },
    /// Badge de langue d'un bloc de code (`LangBadgeWidget`), sur la ligne d'ouverture vidée.
    BadgeLangue { langue: String },
    /// Règle horizontale (`SeparatorWidget`) : un trait sur toute la ligne.
    Separateur,
}

/// La largeur logique d'un widget en ligne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Largeur {
    /// Une largeur fixe, en `em` du texte (`--indent-width` pour case, puce et indentation).
    Em(f32),
    /// Mesurée au rendu (texte d'un lien, formule, image).
    Mesuree,
    /// Toute la largeur de la ligne (séparateur).
    Ligne,
    /// Aucune largeur dans le flux (le badge flotte à droite).
    Nulle,
}

/// Un widget en ligne et la source qu'il remplace.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetEnLigne {
    pub source: Plage,
    pub genre: GenreWidget,
    pub largeur: Largeur,
}

/// Les décorations d'un document pour un curseur donné.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decorations {
    /// Masques (normalisés : triés, disjoints), polices et hauteurs des lignes. `blocs` reste vide :
    /// les widgets de bloc sont le travail de `blocs`.
    pub rangees: ModeleRangees,
    /// Triées par position.
    pub marques: Vec<Marque>,
    /// Triées par ligne, sans doublon.
    pub lignes: Vec<DecoLigne>,
    /// Triés par position de leur source, sans doublon.
    pub widgets: Vec<WidgetEnLigne>,
}

impl Decorations {
    /// Le texte que l'utilisateur VOIT sur la ligne `[de, a)` (octets) : la source moins les masques,
    /// chaque widget remplacé par `rendu(widget)`. Sert aux tests (le `innerText` des specs e2e).
    #[must_use]
    pub fn texte_visible(
        &self,
        texte: &str,
        de: usize,
        a: usize,
        rendu: impl Fn(&WidgetEnLigne) -> String,
    ) -> String {
        let mut out = String::new();
        let mut i = de;
        while i < a {
            if let Some(w) = self
                .widgets
                .iter()
                .find(|w| w.source.octets.from == i && !w.source.octets.is_empty())
            {
                out.push_str(&rendu(w));
                i = w.source.octets.to.max(i + 1);
                continue;
            }
            if let Some(m) = self
                .rangees
                .masques
                .iter()
                .find(|m| m.octets.from <= i && i < m.octets.to)
            {
                i = m.octets.to;
                continue;
            }
            let Some(c) = texte.get(i..).and_then(|r| r.chars().next()) else {
                break;
            };
            out.push(c);
            i += c.len_utf8();
        }
        out
    }
}
