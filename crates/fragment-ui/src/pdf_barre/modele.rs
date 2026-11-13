// Port de app/src/ui/views/pdf/PdfView.ts (construireBarreOutils, ouvrirMenu, remplirMenuBarre,
// remplirMenuZoom, construireRaccourcis, champ de page)
//
//! Le modèle de la barre d'outils PDF : pas de GPUI, donc testable sans fenêtre.
//!
//! Les menus ([`Menu`]) déposent des [`ActionBarre`] dans une file partagée : leurs rappels
//! n'ont pas accès au modèle de la vue (ils vivent plus longtemps qu'un emprunt). La vue appelle
//! [`BarrePdf::traiter`] ensuite, qui rejoue la file sur le modèle et rend le plan de rendu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_pdf::pdf_zoom::{POURCENTAGES, Sens, ZoomMode};
use fragment_pdf::vue_modele::{
    MODES_ZOOM, PlanRendu, Point, ResultatMolette, VueModele, raccourci_pour,
};

use crate::menu::Menu;

/// Les boutons (et le champ) de la barre, dans l'ordre, avec leur `aria-label` : le contrat que
/// vérifie la spec e2e.
pub const LIBELLES_BOUTONS: [&str; 6] = [
    "Toggle sidebar",
    "Sidebar options",
    "Zoom out",
    "Zoom in",
    "Zoom options",
    "Page",
];

/// Un bouton de la barre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bouton {
    BasculerBarre,
    OptionsBarre,
    ZoomMoins,
    ZoomPlus,
    OptionsZoom,
}

impl Bouton {
    /// Dans l'ordre d'affichage.
    pub const TOUS: [Bouton; 5] = [
        Bouton::BasculerBarre,
        Bouton::OptionsBarre,
        Bouton::ZoomMoins,
        Bouton::ZoomPlus,
        Bouton::OptionsZoom,
    ];

    pub fn libelle(self) -> &'static str {
        match self {
            Bouton::BasculerBarre => "Toggle sidebar",
            Bouton::OptionsBarre => "Sidebar options",
            Bouton::ZoomMoins => "Zoom out",
            Bouton::ZoomPlus => "Zoom in",
            Bouton::OptionsZoom => "Zoom options",
        }
    }

    /// Le nom de l'icône du registre.
    pub fn icone(self) -> &'static str {
        match self {
            Bouton::BasculerBarre => "layout-list",
            Bouton::OptionsBarre | Bouton::OptionsZoom => "chevron-down",
            Bouton::ZoomMoins => "zoom-out",
            Bouton::ZoomPlus => "zoom-in",
        }
    }

    /// Un chevron de menu déroulant : plus étroit (20 px), collé à son bouton.
    pub fn est_menu(self) -> bool {
        matches!(self, Bouton::OptionsBarre | Bouton::OptionsZoom)
    }
}

/// Ce que montre la barre latérale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeBarre {
    Vignettes,
    Plan,
}

/// Ce qu'un menu demande.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionBarre {
    /// Choisit le contenu de la barre latérale et l'ouvre.
    ChoisirModeBarre(ModeBarre),
    /// Déplie la table des matières sur la page courante.
    RevelerPage,
    Mode(ZoomMode),
    Pourcentage(f64),
}

type File = Rc<RefCell<Vec<ActionBarre>>>;

/// L'état de la barre d'outils.
pub struct BarrePdf {
    ouverte: bool,
    mode: ModeBarre,
    /// La page à faire apparaître dans la table des matières (demandée par le menu, lue une fois
    /// par la barre latérale).
    revele: Option<usize>,
    /// Le bouton dont le menu est ouvert (remis à zéro par `on_hide`, d'où le partage).
    menu_ouvert: Rc<Cell<Option<Bouton>>>,
    /// ★ Le menu se ferme à l'APPUI extérieur, y compris sur le bouton qui l'a ouvert, dont le
    /// clic arrive juste après : sans ce garde, recliquer le chevron rouvrirait aussitôt le menu.
    menu_ferme: Option<Bouton>,
    file: File,
    /// La vue (son défilement, son champ) a le focus : seule portée des raccourcis de zoom.
    portee: bool,
}

impl Default for BarrePdf {
    fn default() -> Self {
        Self::new()
    }
}

impl BarrePdf {
    pub fn new() -> Self {
        Self {
            ouverte: false,
            mode: ModeBarre::Vignettes,
            revele: None,
            menu_ouvert: Rc::new(Cell::new(None)),
            menu_ferme: None,
            file: Rc::new(RefCell::new(Vec::new())),
            portee: false,
        }
    }

    // ── Barre latérale ───────────────────────────────────────────────────

    pub fn barre_ouverte(&self) -> bool {
        self.ouverte
    }

    pub fn mode_barre(&self) -> ModeBarre {
        self.mode
    }

    /// Actif : la barre latérale ouverte (bouton de bascule) ou le menu déployé (chevrons).
    pub fn bouton_actif(&self, bouton: Bouton) -> bool {
        match bouton {
            Bouton::BasculerBarre => self.ouverte,
            b => self.menu_ouvert.get() == Some(b),
        }
    }

    pub fn basculer_barre(&mut self) {
        self.ouverte = !self.ouverte;
    }

    /// Le contenu de la barre latérale, sans l'ouvrir (état éphémère restauré, fr-t4-vue-pdf).
    pub fn definir_mode_barre(&mut self, mode: ModeBarre) {
        self.mode = mode;
    }

    pub fn ouvrir_barre(&mut self, ouverte: bool) {
        self.ouverte = ouverte;
    }

    /// La page à révéler dans la table des matières, une seule fois.
    pub fn prendre_revele(&mut self) -> Option<usize> {
        self.revele.take()
    }

    // ── Menus ────────────────────────────────────────────────────────────

    /// Le bouton dont le menu est ouvert.
    pub fn menu_ouvert(&self) -> Option<Bouton> {
        self.menu_ouvert.get()
    }

    /// L'appui (souris enfoncée) sur un bouton : si son menu est ouvert, il va se fermer, et le
    /// clic qui suit ne doit pas le rouvrir.
    pub fn appui_sur(&mut self, bouton: Bouton) {
        if self.menu_ouvert.get() == Some(bouton) {
            self.menu_ferme = Some(bouton);
        }
    }

    /// Le clic sur un bouton. Pour un bouton de menu, rend le menu à montrer sous lui (`None` si
    /// ce clic ne fait que refermer). Pour un bouton de zoom, applique le cran tout de suite.
    pub fn cliquer(&mut self, bouton: Bouton, vue: &mut VueModele) -> (Option<Menu>, PlanRendu) {
        match bouton {
            Bouton::BasculerBarre => {
                self.basculer_barre();
                (None, PlanRendu::vide())
            }
            Bouton::ZoomMoins => (None, vue.zoomer_pas(Sens::Moins)),
            Bouton::ZoomPlus => (None, vue.zoomer_pas(Sens::Plus)),
            Bouton::OptionsBarre | Bouton::OptionsZoom => {
                if self.menu_ferme == Some(bouton) {
                    self.menu_ferme = None;
                    return (None, PlanRendu::vide());
                }
                let menu = if bouton == Bouton::OptionsBarre {
                    self.menu_barre()
                } else {
                    self.menu_zoom(vue)
                };
                (Some(self.marquer_ouvert(menu, bouton)), PlanRendu::vide())
            }
        }
    }

    fn marquer_ouvert(&self, mut menu: Menu, bouton: Bouton) -> Menu {
        self.menu_ouvert.set(Some(bouton));
        let ouvert = self.menu_ouvert.clone();
        menu.on_hide(move || ouvert.set(None));
        menu
    }

    fn empiler(&self, action: ActionBarre) -> impl Fn() + 'static {
        let file = self.file.clone();
        move || file.borrow_mut().push(action)
    }

    /// Le menu « ▾ » de la barre latérale : le contenu coché est celui en vigueur.
    pub fn menu_barre(&self) -> Menu {
        let mut menu = Menu::new();
        for (titre, icone, mode) in [
            ("Thumbnails", "layout-grid", ModeBarre::Vignettes),
            ("Table of contents", "list", ModeBarre::Plan),
        ] {
            let coche = self.mode == mode;
            let action = self.empiler(ActionBarre::ChoisirModeBarre(mode));
            menu.add_item(|i| {
                i.set_title(titre)
                    .set_icon(icone)
                    .set_checked(coche)
                    .on_click(action);
            });
        }
        menu.add_separator();
        let action = self.empiler(ActionBarre::RevelerPage);
        menu.add_item(|i| {
            i.set_title("Reveal page in table of contents")
                .set_icon("list-tree")
                .on_click(action);
        });
        menu
    }

    /// Le menu « ▾ » du zoom : les quatre modes, puis les pourcentages ; ce qui est coché est ce
    /// que la vue a en vigueur (un mode, ou une échelle libre égale au pourcentage).
    pub fn menu_zoom(&self, vue: &VueModele) -> Menu {
        let mut menu = Menu::new();
        for (mode, titre) in MODES_ZOOM {
            let coche = vue.mode_coche(mode);
            let action = self.empiler(ActionBarre::Mode(mode));
            menu.add_item(|i| {
                i.set_title(titre).set_checked(coche).on_click(action);
            });
        }
        menu.add_separator();
        for p in POURCENTAGES {
            let coche = vue.pourcentage_coche(p);
            let action = self.empiler(ActionBarre::Pourcentage(p));
            let titre = format!("{}%", (p * 100.0).round());
            menu.add_item(|i| {
                i.set_title(&titre).set_checked(coche).on_click(action);
            });
        }
        menu
    }

    /// Rejoue sur le modèle ce que les menus ont demandé depuis le dernier appel.
    pub fn traiter(&mut self, vue: &mut VueModele) -> PlanRendu {
        let actions: Vec<ActionBarre> = self.file.borrow_mut().drain(..).collect();
        let mut plan = PlanRendu::vide();
        for action in actions {
            match action {
                ActionBarre::ChoisirModeBarre(mode) => {
                    self.mode = mode;
                    self.ouverte = true;
                }
                ActionBarre::RevelerPage => {
                    self.mode = ModeBarre::Plan;
                    self.ouverte = true;
                    self.revele = Some(vue.page_vue().max(1));
                }
                ActionBarre::Mode(mode) => plan.fusionner(vue.choisir_mode(mode)),
                ActionBarre::Pourcentage(p) => plan.fusionner(vue.choisir_pourcentage(p)),
            }
        }
        plan
    }

    // ── Champ de page ────────────────────────────────────────────────────

    /// Le champ prend le focus : tout son contenu est sélectionné.
    pub fn champ_focus(&self, vue: &mut VueModele) {
        vue.champ_focaliser();
    }

    /// Une touche tapée dans le champ. `texte` est le caractère produit (s'il y en a un).
    /// Entrée va à la page (bornée), Échap restaure la page courante et rend le focus ; le reste
    /// édite le texte. Rend le plan de rendu (non vide quand on a changé de page).
    pub fn touche_champ(
        &self,
        vue: &mut VueModele,
        touche: &str,
        texte: Option<&str>,
    ) -> PlanRendu {
        match touche {
            "enter" => return vue.champ_valider(),
            "escape" => vue.champ_echap(),
            "backspace" => {
                let mut v = vue.champ().valeur.clone();
                // Tout sélectionné : effacer vide le champ.
                if vue.champ().tout_selectionne {
                    v.clear();
                } else {
                    v.pop();
                }
                vue.champ_saisir(&v);
            }
            _ => {
                if let Some(t) = texte.filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
                {
                    let mut v = if vue.champ().tout_selectionne {
                        String::new()
                    } else {
                        vue.champ().valeur.clone()
                    };
                    v.push_str(t);
                    vue.champ_saisir(&v);
                }
            }
        }
        PlanRendu::vide()
    }

    /// Le champ perd le focus : une saisie en cours vaut validation, comme l'événement
    /// `change` du navigateur.
    pub fn champ_perdre_focus(&self, vue: &mut VueModele) -> PlanRendu {
        let modifie = vue.champ().valeur != vue.page_vue().to_string();
        let plan = if modifie && vue.champ().focus {
            vue.champ_valider()
        } else {
            PlanRendu::vide()
        };
        vue.champ_perdre_focus();
        plan
    }

    // ── Raccourcis et molette ────────────────────────────────────────────

    /// La vue entre ou sort de la portée des raccourcis (focus dans la vue).
    pub fn definir_portee(&mut self, active: bool) {
        self.portee = active;
    }

    /// Ctrl/Cmd + `+`, `-`, `0` sur le CODE PHYSIQUE de la touche (en AZERTY, Ctrl + 0 produit
    /// « à » : seul le code est fiable). `Some(plan)` = consommé (la touche ne doit pas aller
    /// plus loin, notamment pas zoomer l'application) ; `None` = pas pour nous (hors de la portée
    /// de la vue, sans Ctrl, ou une autre touche).
    pub fn touche(
        &self,
        vue: &mut VueModele,
        code: &str,
        ctrl_ou_cmd: bool,
        maj: bool,
    ) -> Option<PlanRendu> {
        if !self.portee || !ctrl_ou_cmd {
            return None;
        }
        raccourci_pour(code, maj).map(|r| vue.executer_raccourci(r))
    }

    /// Ctrl + molette et pinch : zoom continu ancré sous le curseur.
    pub fn molette(
        &self,
        vue: &mut VueModele,
        delta_y: f64,
        delta_mode: u32,
        ctrl_ou_cmd: bool,
        point: Point,
    ) -> ResultatMolette {
        vue.zoomer_molette(delta_y, delta_mode, ctrl_ou_cmd, point)
    }
}
