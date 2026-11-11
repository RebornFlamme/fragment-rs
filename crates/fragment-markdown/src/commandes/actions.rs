// Port de app/src/ui/views/markdown/EditorCommandsPlugin.ts
//! La table des commandes `editor:*` : un identifiant, un nom, une transformation PURE de
//! `editor_transforms`, un raccourci éventuel.
//!
//! ★ Les identifiants sont ceux d'Obsidian, à la lettre : ils forment une API (raccourcis
//!   personnalisés, CLI, plugins tiers). Le préfixe `editor:` s'ajoute à l'enregistrement, mais
//!   la table et [`appliquer_commande_editeur`] parlent en identifiants SANS préfixe, parce que
//!   le menu contextuel les applique aussi — y compris à un éditeur qui n'est pas l'éditeur
//!   actif (une cellule de tableau). Une implémentation, deux portes.

use std::rc::Rc;

use fragment_core::command::{Command, EditorContext};
use fragment_core::keymap::types::{Hotkey, Modifier};
use fragment_text::{
    Editor, EditorChangeSpec, EditorRange, EditorSelectionSpec, EditorTransactionSpec, utf16_len,
};

use crate::editor_transforms::{self as t, Transformation};

/// Ce que `EditorContext::editor` contient pour une commande d'éditeur : un `Rc<dyn Editor>`
/// (type dimensionné, donc `downcast_ref::<EditeurPartage>()` fonctionne).
pub type EditeurPartage = Rc<dyn Editor>;

/// Le préfixe que l'identifiant du plugin « Éditeur » donne à ses commandes.
pub const PREFIXE: &str = "editor:";

/// Une commande-transformation.
#[derive(Clone, Copy)]
pub struct ActionEditeur {
    /// Sans le préfixe `editor:`.
    pub id: &'static str,
    pub nom: &'static str,
    pub transformation: Transformation,
    /// Un raccourci `Mod+<touche>`, quand la commande en a un par défaut.
    pub touche_mod: Option<&'static str>,
    /// Maintenir le raccourci re-déclenche la commande.
    pub repetable: bool,
}

impl ActionEditeur {
    /// L'identifiant complet : `editor:toggle-bold`.
    #[must_use]
    pub fn id_complet(&self) -> String {
        format!("{PREFIXE}{}", self.id)
    }

    /// Les raccourcis par défaut de la commande.
    #[must_use]
    pub fn raccourcis(&self) -> Vec<Hotkey> {
        self.touche_mod
            .map(|k| Hotkey::new(&[Modifier::Mod], k))
            .into_iter()
            .collect()
    }
}

// ── Les transformations à paramètre, en pointeurs de fonction sans capture ──

type Sel = EditorRange;
type Spec = Option<EditorTransactionSpec>;

fn gras(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "**"))
}
fn italique(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "*"))
}
fn barre(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "~~"))
}
fn surligne(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "=="))
}
fn code(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "`"))
}
fn math_en_ligne(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "$"))
}
fn commentaire(d: &str, s: Sel) -> Spec {
    Some(t::basculer_marqueur(d, s, "%%"))
}
fn titre<const N: usize>(d: &str, s: Sel) -> Spec {
    t::definir_titre(d, s, N)
}
fn ligne_vers_le_haut(d: &str, s: Sel) -> Spec {
    t::permuter_lignes(d, s, true)
}
fn ligne_vers_le_bas(d: &str, s: Sel) -> Spec {
    t::permuter_lignes(d, s, false)
}
fn lien(d: &str, s: Sel) -> Spec {
    Some(t::inserer_lien(d, s))
}
fn lien_interne(d: &str, s: Sel) -> Spec {
    Some(t::inserer_lien_interne(d, s))
}
fn encadre(d: &str, s: Sel) -> Spec {
    Some(t::inserer_encadre(d, s))
}
fn tableau(d: &str, s: Sel) -> Spec {
    Some(t::inserer_tableau(d, s))
}
fn note_de_bas_de_page(d: &str, s: Sel) -> Spec {
    Some(t::inserer_note_de_bas_de_page(d, s))
}
fn ligne_horizontale(d: &str, s: Sel) -> Spec {
    Some(t::inserer_ligne_horizontale(d, s))
}
fn bloc_de_code(d: &str, s: Sel) -> Spec {
    Some(t::inserer_bloc_de_code(d, s))
}
fn bloc_math(d: &str, s: Sel) -> Spec {
    Some(t::inserer_bloc_math(d, s))
}

const fn action(
    id: &'static str,
    nom: &'static str,
    transformation: Transformation,
) -> ActionEditeur {
    ActionEditeur {
        id,
        nom,
        transformation,
        touche_mod: None,
        repetable: false,
    }
}

const fn avec_mod(mut a: ActionEditeur, touche: &'static str) -> ActionEditeur {
    a.touche_mod = Some(touche);
    a
}

const fn repetable(mut a: ActionEditeur) -> ActionEditeur {
    a.repetable = true;
    a
}

/// Les commandes-transformations, dans l'ordre de la table d'origine.
pub static ACTIONS: &[ActionEditeur] = &[
    // ── Marqueurs en ligne ──
    avec_mod(action("toggle-bold", "Gras", gras), "b"),
    avec_mod(action("toggle-italics", "Italique", italique), "i"),
    action("toggle-strikethrough", "Barré", barre),
    action("toggle-highlight", "Surligner", surligne),
    action("toggle-code", "Code en ligne", code),
    action("toggle-inline-math", "Math en ligne", math_en_ligne),
    action("toggle-comment", "Commentaire", commentaire),
    action(
        "clear-formatting",
        "Effacer la mise en forme",
        t::effacer_mise_en_forme,
    ),
    // ── Liens ──
    avec_mod(action("insert-link", "Insérer un lien", lien), "k"),
    action("insert-wikilink", "Insérer un lien interne", lien_interne),
    // ── Blocs ──
    action("insert-callout", "Insérer un encadré", encadre),
    action("insert-table", "Insérer un tableau", tableau),
    action(
        "insert-footnote",
        "Insérer une note de bas de page",
        note_de_bas_de_page,
    ),
    action(
        "insert-horizontal-rule",
        "Insérer une ligne horizontale",
        ligne_horizontale,
    ),
    action("insert-codeblock", "Insérer un bloc de code", bloc_de_code),
    action("insert-mathblock", "Insérer un bloc math", bloc_math),
    // ── Titres ──
    action("set-heading-0", "Retirer le titre", titre::<0>),
    action("set-heading-1", "Titre 1", titre::<1>),
    action("set-heading-2", "Titre 2", titre::<2>),
    action("set-heading-3", "Titre 3", titre::<3>),
    action("set-heading-4", "Titre 4", titre::<4>),
    action("set-heading-5", "Titre 5", titre::<5>),
    action("set-heading-6", "Titre 6", titre::<6>),
    // ── Lignes ──
    action("toggle-bullet-list", "Liste à puces", t::basculer_liste),
    action(
        "toggle-numbered-list",
        "Liste numérotée",
        t::basculer_liste_numerotee,
    ),
    action(
        "toggle-checklist-status",
        "Liste de tâches",
        t::basculer_liste_taches,
    ),
    action("toggle-blockquote", "Citation", t::basculer_citation),
    repetable(action(
        "swap-line-up",
        "Déplacer la ligne vers le haut",
        ligne_vers_le_haut,
    )),
    repetable(action(
        "swap-line-down",
        "Déplacer la ligne vers le bas",
        ligne_vers_le_bas,
    )),
];

/// La transformation de la commande `id` (SANS préfixe), si c'en est une.
#[must_use]
pub fn transformation_de(id: &str) -> Option<Transformation> {
    ACTIONS
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.transformation)
}

/// Le mariage d'une transformation pure et d'un éditeur : lire, calculer, appliquer en UNE
/// transaction (donc une seule entrée d'annulation), et ramener la sélection à l'écran.
pub fn appliquer_transformation(editor: &dyn Editor, transformation: Transformation) {
    let spec = transformation(&editor.get_value(), editor.get_selection());
    if let Some(mut spec) = spec {
        spec.scroll_into_view = true;
        editor.transaction(spec);
    }
}

/// Applique la commande `editor:<id>` à un éditeur DONNÉ (pas forcément l'actif). Faux si `id`
/// (sans préfixe) n'est pas une commande-transformation.
pub fn appliquer_commande_editeur(editor: &dyn Editor, id: &str) -> bool {
    let Some(transformation) = transformation_de(id) else {
        return false;
    };
    appliquer_transformation(editor, transformation);
    true
}

/// Colle `texte` (déjà lu du presse-papiers) SANS mise en forme, à la place de la sélection.
pub fn coller_texte_brut(editor: &dyn Editor, texte: &str) {
    if texte.is_empty() {
        return;
    }
    let sel = editor.get_selection();
    editor.transaction(EditorTransactionSpec {
        changes: vec![EditorChangeSpec {
            from: sel.from,
            to: Some(sel.to),
            insert: texte.to_owned(),
        }],
        selection: Some(EditorSelectionSpec {
            anchor: sel.from + utf16_len(texte),
            head: None,
        }),
        scroll_into_view: true,
    });
}

/// `editor:select-all`.
pub fn tout_selectionner(editor: &dyn Editor) {
    editor.set_selection(0, Some(utf16_len(&editor.get_value())));
}

fn editeur_du_contexte(ctx: &EditorContext) -> Option<&EditeurPartage> {
    ctx.editor.downcast_ref::<EditeurPartage>()
}

/// Les commandes du plugin « Éditeur » que ce module sait enregistrer : toutes les
/// transformations, et `editor:select-all`.
///
/// ★ Ne sont PAS là (elles lisent le système ou la vue, `fr-t3-vue` les branche) :
///   `editor:paste-plain` (presse-papiers), `editor:save-file` (la vue), `editor:follow-link`
///   (le workspace ; `lien_sous_le_curseur` en est la partie pure).
#[must_use]
pub fn commandes_editeur() -> Vec<Command> {
    let mut commandes: Vec<Command> = ACTIONS
        .iter()
        .map(|a| {
            let transformation = a.transformation;
            let mut c = Command::new(&a.id_complet(), a.nom).with_editor_callback(move |ctx| {
                if let Some(editor) = editeur_du_contexte(ctx) {
                    appliquer_transformation(editor.as_ref(), transformation);
                }
            });
            if a.touche_mod.is_some() {
                c = c.with_hotkeys(a.raccourcis());
            }
            if a.repetable {
                c = c.repeatable();
            }
            c
        })
        .collect();
    commandes.push(
        Command::new("editor:select-all", "Tout sélectionner").with_editor_callback(|ctx| {
            if let Some(editor) = editeur_du_contexte(ctx) {
                tout_selectionner(editor.as_ref());
            }
        }),
    );
    commandes
}
