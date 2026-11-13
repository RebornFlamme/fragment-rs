// Port de app/src/ui/views/markdown/MarkdownView.ts (wireContextMenu : le menu montré à la souris,
// onPaste) et de EditorCommandsPlugin.ts (collerTexteBrut, suivre).
//! Ce que la note fait avec la souris, le presse-papiers et le clavier de l'hôte : le menu
//! contextuel dessiné (le [`Menu`] du shell), le collage d'image, `editor:paste-plain` et
//! `editor:follow-link`.
//!
//! ★ Un clic de menu ou une commande n'ont ni fenêtre ni `App` : ils POSTENT une [`ActionMenu`]
//!   dans les [`Interaction`] de la vue, et le rendu (qui a le presse-papiers) l'exécute à sa
//!   prochaine image, avant de recopier le document dans l'éditeur.

use fragment_core::{Keymap, LocalDateTime, TFile};
use fragment_markdown::coller_image::FichierPresse;
use fragment_markdown::commandes::menu::{
    ActionMenu, DemandeHote, ItemMenu, SECTIONS_MENU_EDITEUR,
};
use fragment_markdown::editor_transforms::lien_sous_le_curseur;
use fragment_text::{Editor as _, TextSurface as _, utf16_to_byte};
use gpui_kit::{App, ClipboardEntry, ClipboardItem};

use super::collage::{IssueCollage, ReglageAttachements, coller_dans};
use super::menu::{executer_entree_de, menu_contextuel_de};
use super::vue::Liens;
use crate::menu::Menu;

/// L'état d'interaction d'une vue.
#[derive(Default)]
pub struct Interaction {
    /// Le clavier de l'application : le menu y empile sa portée Échap.
    pub clavier: Option<Keymap>,
    /// Le réglage « Default location for new attachments ».
    pub reglage: ReglageAttachements,
    /// Les actions à exécuter à la prochaine image.
    pub demandes: Vec<ActionMenu>,
}

/// Poste une action pour la prochaine image.
pub fn poster(liens: &Liens, action: ActionMenu) {
    liens.interaction.borrow_mut().demandes.push(action);
}

/// Le menu contextuel de la note, en [`Menu`] : les sections de l'ouvreur, les entrées grisées
/// du modèle, un clic qui poste son action.
pub fn menu_de_la_note(liens: &Liens) -> Menu {
    let mut menu = Menu::new();
    menu.add_sections(&SECTIONS_MENU_EDITEUR);
    remplir(&mut menu, &menu_contextuel_de(liens), liens);
    menu
}

fn remplir(menu: &mut Menu, items: &[ItemMenu], liens: &Liens) {
    for item in items {
        match item {
            ItemMenu::Entree(e) => {
                let (liens, action) = (liens.clone(), e.action);
                menu.add_item(|i| {
                    i.set_title(e.titre)
                        .set_icon(e.icone)
                        .set_section(e.section)
                        .set_disabled(e.desactivee)
                        .on_click(move || poster(&liens, action));
                });
            }
            ItemMenu::SousMenu(s) => {
                menu.add_item(|i| {
                    i.set_title(s.titre)
                        .set_icon(s.icone)
                        .set_section(s.section)
                        .set_disabled(s.desactive);
                    let sous = i.set_submenu();
                    let entrees: Vec<ItemMenu> =
                        s.entrees.iter().cloned().map(ItemMenu::Entree).collect();
                    remplir(sous, &entrees, liens);
                });
            }
        }
    }
}

/// Montre le menu contextuel de la note à `(x, y)` (coordonnées de la fenêtre).
pub fn montrer_menu(liens: &Liens, x: f32, y: f32) {
    let clavier = liens.interaction.borrow().clavier.clone();
    menu_de_la_note(liens).show_at_position(x, y, &clavier.unwrap_or_default());
}

/// L'heure locale (le nom de l'image collée) ; l'UTC si l'OS ne dit pas son fuseau.
pub fn heure_locale() -> LocalDateTime {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    LocalDateTime {
        year: t.year(),
        month: u32::from(u8::from(t.month())),
        day: u32::from(t.day()),
        hour: u32::from(t.hour()),
        minute: u32::from(t.minute()),
        second: u32::from(t.second()),
    }
}

fn mime_de_extension(chemin: &std::path::Path) -> Option<&'static str> {
    let ext = chemin.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// Les fichiers image d'un presse-papiers : une capture d'écran, ou des fichiers copiés depuis
/// l'explorateur de l'OS.
pub fn images_du_presse_papiers(item: &ClipboardItem) -> Vec<FichierPresse> {
    let mut v = Vec::new();
    for e in item.entries() {
        match e {
            ClipboardEntry::Image(i) => v.push(FichierPresse {
                mime: i.format.mime_type().to_owned(),
                octets: i.bytes.clone(),
            }),
            ClipboardEntry::ExternalPaths(p) => {
                for chemin in p.paths() {
                    if let (Some(mime), Ok(octets)) =
                        (mime_de_extension(chemin), std::fs::read(chemin))
                    {
                        v.push(FichierPresse {
                            mime: mime.to_owned(),
                            octets,
                        });
                    }
                }
            }
            ClipboardEntry::String(_) => {}
        }
    }
    v
}

/// Colle les IMAGES du presse-papiers, s'il y en a. Rend vrai quand l'événement est consommé (le
/// collage de texte, lui, reste à l'éditeur).
pub fn coller_images(liens: &Liens, item: Option<&ClipboardItem>) -> bool {
    let Some(item) = item else { return false };
    let fichiers = images_du_presse_papiers(item);
    if fichiers.is_empty() {
        return false;
    }
    let reglage = liens.interaction.borrow().reglage.clone();
    let issue = coller_dans(liens, &fichiers, &[], &reglage, heure_locale);
    matches!(issue, IssueCollage::Insere(_) | IssueCollage::Echec(_))
}

/// Le texte sélectionné, s'il y en a.
fn selection_texte(liens: &Liens) -> Option<(usize, usize, String)> {
    let e = liens.editeur_interne();
    let sel = e.get_selection();
    if sel.from == sel.to {
        return None;
    }
    let texte = e.get_value();
    let (a, b) = (
        utf16_to_byte(&texte, sel.from)?,
        utf16_to_byte(&texte, sel.to)?,
    );
    Some((sel.from, sel.to, texte.get(a..b)?.to_owned()))
}

/// Exécute ce que le menu et les commandes ont posté.
pub fn traiter(liens: &Liens, cx: &mut App) {
    let demandes = std::mem::take(&mut liens.interaction.borrow_mut().demandes);
    for action in demandes {
        let texte_brut = if action == ActionMenu::CollerTexteBrut {
            cx.read_from_clipboard()
                .and_then(|c| c.text())
                .unwrap_or_default()
        } else {
            String::new()
        };
        match executer_entree_de(liens, action, &texte_brut) {
            Some(DemandeHote::Copier) => {
                if let Some((_, _, t)) = selection_texte(liens) {
                    cx.write_to_clipboard(ClipboardItem::new_string(t));
                }
            }
            Some(DemandeHote::Couper) => {
                if let Some((de, a, t)) = selection_texte(liens) {
                    cx.write_to_clipboard(ClipboardItem::new_string(t));
                    liens.editeur_interne().replace_range(de, a, "");
                }
            }
            Some(DemandeHote::Coller) => {
                let item = cx.read_from_clipboard();
                if !coller_images(liens, item.as_ref()) {
                    let t = item.and_then(|c| c.text()).unwrap_or_default();
                    let _ = executer_entree_de(liens, ActionMenu::CollerTexteBrut, &t);
                }
            }
            None => {}
        }
    }
}

/// `editor:follow-link` : la cible du lien sous le curseur, nue.
pub fn lien_a_suivre(liens: &Liens) -> Option<String> {
    if liens.lecture_seule() {
        return None;
    }
    let e = liens.editeur_interne();
    lien_sous_le_curseur(&e.get_value(), e.get_selection().from)
}

/// Le fichier que désigne une cible : le chemin nu d'abord (un lien vers un PDF porte son
/// extension), puis `<cible>.md`. Un lien externe (`https://…`) n'a pas de porte ici.
pub fn fichier_du_lien(liens: &Liens, cible: &str) -> Option<TFile> {
    if cible.contains("://") {
        return None;
    }
    let v = liens.vault();
    v.get_file_by_path(cible)
        .or_else(|| v.get_file_by_path(&format!("{cible}.md")))
}
