// Port de app/src/ui/views/markdown/MarkdownView.ts (remplirMenuEditeur, wireContextMenu)
//! Le MODÈLE du menu contextuel de l'éditeur : sections, entrées, sous-menus, entrées grisées.
//! Aucune présentation : la vue le traduit en menu ; le modèle dit seulement quoi montrer, dans
//! quel ordre, et ce qui est grisé selon la sélection et le mode.
//!
//! Le menu d'Obsidian, entrée pour entrée (les titres restent en anglais, comme lui). Chaque
//! entrée applique la commande `editor:*` du même nom à l'éditeur visé : une seule
//! implémentation, deux portes. Dans un éditeur IMBRIQUÉ (cellule de tableau), les blocs
//! (Paragraph, Insert) n'ont pas de sens : grisés.

use fragment_text::Editor;

use super::actions::{appliquer_commande_editeur, coller_texte_brut, tout_selectionner};

/// L'ordre des sections, de l'ouvreur : liens, mise en forme, presse-papiers (celui d'Obsidian).
/// `selection` (plugins) reste en tête, `action` en queue.
pub const SECTIONS_MENU_EDITEUR: [&str; 7] = [
    "selection",
    "link",
    "format",
    "clipboard",
    "table",
    "table-sort",
    "action",
];

/// Ce qui décide des entrées grisées.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ContexteMenu {
    /// La sélection est un simple curseur.
    pub selection_vide: bool,
    pub lecture_seule: bool,
    /// L'éditeur visé est imbriqué (cellule de tableau, note transcluse).
    pub imbrique: bool,
}

/// Ce qu'une entrée déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionMenu {
    /// `editor:<id>` (id SANS préfixe).
    Commande(&'static str),
    /// Presse-papiers du système : c'est l'hôte qui s'en charge.
    Couper,
    Copier,
    Coller,
    /// Colle du texte brut : l'hôte lit le presse-papiers et le passe à [`executer_action`].
    CollerTexteBrut,
    ToutSelectionner,
    /// Entrée sans effet (« New base » : pas de sous-système Bases).
    Aucune,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntreeMenu {
    pub titre: &'static str,
    /// Nom d'icône Lucide.
    pub icone: &'static str,
    pub section: &'static str,
    pub desactivee: bool,
    pub action: ActionMenu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SousMenu {
    pub titre: &'static str,
    pub icone: &'static str,
    pub section: &'static str,
    pub desactive: bool,
    pub entrees: Vec<EntreeMenu>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemMenu {
    Entree(EntreeMenu),
    SousMenu(SousMenu),
}

impl ItemMenu {
    #[must_use]
    pub fn titre(&self) -> &'static str {
        match self {
            ItemMenu::Entree(e) => e.titre,
            ItemMenu::SousMenu(s) => s.titre,
        }
    }

    #[must_use]
    pub fn section(&self) -> &'static str {
        match self {
            ItemMenu::Entree(e) => e.section,
            ItemMenu::SousMenu(s) => s.section,
        }
    }
}

fn entree(
    titre: &'static str,
    icone: &'static str,
    section: &'static str,
    desactivee: bool,
    action: ActionMenu,
) -> EntreeMenu {
    EntreeMenu {
        titre,
        icone,
        section,
        desactivee,
        action,
    }
}

/// `(titre, icône, id de commande ou None, section)`.
type Ligne = (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
);

fn sous_menu(
    titre: &'static str,
    icone: &'static str,
    desactive: bool,
    lignes: &[Ligne],
) -> ItemMenu {
    ItemMenu::SousMenu(SousMenu {
        titre,
        icone,
        section: "format",
        desactive,
        entrees: lignes
            .iter()
            .map(|&(t, ic, id, section)| {
                entree(
                    t,
                    ic,
                    section,
                    id.is_none(),
                    id.map_or(ActionMenu::Aucune, ActionMenu::Commande),
                )
            })
            .collect(),
    })
}

/// Le menu de l'éditeur pour ce contexte, dans l'ordre d'ajout (la vue range par section).
#[must_use]
pub fn modele_menu_editeur(ctx: ContexteMenu) -> Vec<ItemMenu> {
    let ContexteMenu {
        selection_vide: vide,
        lecture_seule: ro,
        imbrique,
    } = ctx;
    let blocs_grises = ro || imbrique;
    vec![
        ItemMenu::Entree(entree(
            "Add link",
            "link",
            "link",
            ro,
            ActionMenu::Commande("insert-wikilink"),
        )),
        ItemMenu::Entree(entree(
            "Add external link",
            "external-link",
            "link",
            ro,
            ActionMenu::Commande("insert-link"),
        )),
        sous_menu(
            "Format",
            "paintbrush",
            ro,
            &[
                ("Bold", "bold", Some("toggle-bold"), "style"),
                ("Italic", "italic", Some("toggle-italics"), "style"),
                (
                    "Strikethrough",
                    "strikethrough",
                    Some("toggle-strikethrough"),
                    "style",
                ),
                (
                    "Highlight",
                    "highlighter",
                    Some("toggle-highlight"),
                    "style",
                ),
                ("Code", "code", Some("toggle-code"), "style"),
                ("Math", "sigma", Some("toggle-inline-math"), "style"),
                ("Comment", "percent", Some("toggle-comment"), "style"),
                (
                    "Clear formatting",
                    "eraser",
                    Some("clear-formatting"),
                    "clear",
                ),
            ],
        ),
        sous_menu(
            "Paragraph",
            "pilcrow",
            blocs_grises,
            &[
                ("Bullet list", "list", Some("toggle-bullet-list"), "list"),
                (
                    "Numbered list",
                    "list-ordered",
                    Some("toggle-numbered-list"),
                    "list",
                ),
                (
                    "Task list",
                    "list-checks",
                    Some("toggle-checklist-status"),
                    "list",
                ),
                ("Heading 1", "heading-1", Some("set-heading-1"), "heading"),
                ("Heading 2", "heading-2", Some("set-heading-2"), "heading"),
                ("Heading 3", "heading-3", Some("set-heading-3"), "heading"),
                ("Heading 4", "heading-4", Some("set-heading-4"), "heading"),
                ("Heading 5", "heading-5", Some("set-heading-5"), "heading"),
                ("Heading 6", "heading-6", Some("set-heading-6"), "heading"),
                ("Body", "pilcrow", Some("set-heading-0"), "body"),
                ("Quote", "quote", Some("toggle-blockquote"), "body"),
            ],
        ),
        sous_menu(
            "Insert",
            "list-plus",
            blocs_grises,
            &[
                ("Footnote", "footprints", Some("insert-footnote"), "inline"),
                ("Table", "table", Some("insert-table"), "inline"),
                (
                    "Callout",
                    "message-square-quote",
                    Some("insert-callout"),
                    "inline",
                ),
                (
                    "Horizontal rule",
                    "minus",
                    Some("insert-horizontal-rule"),
                    "inline",
                ),
                ("Code block", "code-xml", Some("insert-codeblock"), "block"),
                (
                    "Math block",
                    "square-sigma",
                    Some("insert-mathblock"),
                    "block",
                ),
                ("New base", "database", None, "block"),
            ],
        ),
        ItemMenu::Entree(entree(
            "Cut",
            "scissors",
            "clipboard",
            ro || vide,
            ActionMenu::Couper,
        )),
        ItemMenu::Entree(entree(
            "Copy",
            "copy",
            "clipboard",
            vide,
            ActionMenu::Copier,
        )),
        ItemMenu::Entree(entree(
            "Paste",
            "clipboard",
            "clipboard",
            ro,
            ActionMenu::Coller,
        )),
        ItemMenu::Entree(entree(
            "Paste as plain text",
            "clipboard-type",
            "clipboard",
            ro,
            ActionMenu::CollerTexteBrut,
        )),
        ItemMenu::Entree(entree(
            "Select all",
            "square-dashed",
            "clipboard",
            false,
            ActionMenu::ToutSelectionner,
        )),
    ]
}

/// Ce que l'hôte doit faire lui-même (il possède le presse-papiers du système).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemandeHote {
    Couper,
    Copier,
    Coller,
}

/// Exécute une entrée de menu sur l'éditeur visé, puis lui rend le focus. `texte_brut` est le
/// contenu du presse-papiers, lu par l'hôte (seulement utile à `CollerTexteBrut`).
#[must_use]
pub fn executer_action(
    editor: &dyn Editor,
    action: ActionMenu,
    texte_brut: &str,
) -> Option<DemandeHote> {
    let demande = match action {
        ActionMenu::Commande(id) => {
            appliquer_commande_editeur(editor, id);
            None
        }
        ActionMenu::ToutSelectionner => {
            tout_selectionner(editor);
            None
        }
        ActionMenu::CollerTexteBrut => {
            coller_texte_brut(editor, texte_brut);
            None
        }
        ActionMenu::Couper => Some(DemandeHote::Couper),
        ActionMenu::Copier => Some(DemandeHote::Copier),
        ActionMenu::Coller => Some(DemandeHote::Coller),
        ActionMenu::Aucune => return None,
    };
    editor.focus();
    demande
}
