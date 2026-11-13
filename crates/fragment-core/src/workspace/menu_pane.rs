// Port de app/src/ui/core/workspace/View.ts (onPaneMenu) et ItemView.ts (l'ouvreur de la chaîne)
//! Le menu « ⋯ » d'un pane, côté cœur : un modèle de DONNÉES que l'ouvreur (l'en-tête de vue)
//! remplit puis dessine.
//!
//! ★ Interception par mutation : la vue reçoit un menu EN CONSTRUCTION et y AJOUTE ses entrées
//!   ([`View::on_pane_menu`](super::vue::View::on_pane_menu)) ; elle ne le crée pas, ne
//!   l'affiche pas, ne le ferme pas. L'ouvreur déclare l'ordre des groupes, y compris ceux que
//!   seules les vues peupleront. Le cœur ne connaît ni `gpui-kit` ni aucune vue par son nom : le
//!   dessin est l'affaire de la couche d'interface, qui convertit ce modèle en menu dessiné.

use std::rc::Rc;

use super::vue::View;

/// L'ordre des groupes déclaré par l'ouvreur (ordre Obsidian), y compris ceux que seules les vues
/// peupleront.
pub const SECTIONS_PANE: [&str; 6] = ["pane", "action", "view", "info", "system", "danger"];

/// Ce que fait une entrée au clic.
pub type ActionMenu = Rc<dyn Fn()>;

/// D'où le menu est ouvert (`source` de `onPaneMenu`).
pub const SOURCE_PLUS_D_OPTIONS: &str = "more-options";
/// Le menu de l'en-tête d'onglet.
pub const SOURCE_ENTETE_ONGLET: &str = "tab-header";

/// Une entrée du menu de pane.
#[derive(Clone, Default)]
pub struct EntreeMenuPane {
    pub titre: String,
    pub icone: String,
    pub section: String,
    pub desactivee: bool,
    pub avertissement: bool,
    pub action: Option<ActionMenu>,
    /// Le sous-menu, quand l'entrée en porte un (un seul groupe, sans section).
    pub sous_menu: Option<MenuPane>,
}

impl std::fmt::Debug for EntreeMenuPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntreeMenuPane({}, {})", self.section, self.titre)
    }
}

impl EntreeMenuPane {
    pub fn set_title(&mut self, titre: &str) -> &mut Self {
        self.titre = titre.to_owned();
        self
    }

    pub fn set_icon(&mut self, icone: &str) -> &mut Self {
        self.icone = icone.to_owned();
        self
    }

    pub fn set_section(&mut self, section: &str) -> &mut Self {
        self.section = section.to_owned();
        self
    }

    pub fn set_disabled(&mut self, desactivee: bool) -> &mut Self {
        self.desactivee = desactivee;
        self
    }

    pub fn set_warning(&mut self, avertissement: bool) -> &mut Self {
        self.avertissement = avertissement;
        self
    }

    pub fn on_click(&mut self, action: impl Fn() + 'static) -> &mut Self {
        self.action = Some(Rc::new(action));
        self
    }

    /// Crée (ou rend) le sous-menu de l'entrée.
    pub fn set_submenu(&mut self) -> &mut MenuPane {
        self.sous_menu.get_or_insert_with(MenuPane::default)
    }
}

/// Le menu de pane en construction.
#[derive(Clone, Default, Debug)]
pub struct MenuPane {
    sections: Vec<String>,
    entrees: Vec<EntreeMenuPane>,
}

impl MenuPane {
    pub fn new() -> Self {
        Self::default()
    }

    /// Déclare l'ordre des sections : réservé à celui qui OUVRE le menu.
    pub fn add_sections(&mut self, sections: &[&str]) -> &mut Self {
        self.sections
            .extend(sections.iter().map(|s| (*s).to_owned()));
        self
    }

    /// Le builder central, comme `Menu.addItem`.
    pub fn add_item(&mut self, cb: impl FnOnce(&mut EntreeMenuPane)) -> &mut Self {
        let mut e = EntreeMenuPane::default();
        cb(&mut e);
        self.entrees.push(e);
        self
    }

    /// L'ordre des sections déclaré par l'ouvreur.
    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    /// Les entrées, dans l'ordre d'ajout (le rangement par sections est celui du menu dessiné).
    pub fn entrees(&self) -> &[EntreeMenuPane] {
        &self.entrees
    }

    /// Les titres, dans l'ordre d'ajout (pratique pour comparer).
    pub fn titres(&self) -> Vec<&str> {
        self.entrees.iter().map(|e| e.titre.as_str()).collect()
    }
}

/// Compose le menu « ⋯ » d'un pane : l'ouvreur déclare les sections, ajoute ses entrées de base
/// (`de_base`), puis la vue ajoute les siennes ([`View::on_pane_menu`]).
pub fn menu_de_pane(vue: &dyn View, source: &str, de_base: impl FnOnce(&mut MenuPane)) -> MenuPane {
    let mut menu = MenuPane::new();
    menu.add_sections(&SECTIONS_PANE);
    de_base(&mut menu);
    vue.on_pane_menu(&mut menu, source);
    menu
}
