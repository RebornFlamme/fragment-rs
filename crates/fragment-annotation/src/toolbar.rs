// Port de app/src/ui/core/Toolbar.ts (le MODÈLE des sections et du placement)
//
// ═══════════════════════════════════════════════════════════════════════════
//  La barre d'outils flottante — le pendant PERSISTANT du menu contextuel.
//
//  ★ Crate choisie : fragment-annotation, la plus proche des quatre de cette tâche
//    (la barre d'outils est celle des sessions d'annotation, `annotation:toolbar`).
//    Le MODÈLE porté : les items (étiquette, section, groupe exclusif, état actif /
//    désactivé), le regroupement par SECTIONS rejoué à chaque ajout (une barre est
//    persistante), les séparateurs manuels, `clear`, les colonnes, l'orientation,
//    le bornage au parent (`moveTo`), le seuil de drag, et la durée de vie
//    (`show`/`hide`/`onHide`).
//
//  ★ Pas porté ici (présentation, fragment-ui) : le DOM (`.toolbar-section`, icônes
//    Lucide, `mousedown` qui ne vole pas le focus), la poignée et ses événements
//    pointeur, `register(porteePersistante(Échap))` (portée du Keymap), la rangée
//    d'options (`setOptions`) — voir parite.toml.
//
//  ★ Les SECTIONS sont un contrat de Menu (Obsidian 0.15.3) : chaque item déclare son
//    GROUPE par `set_section` et atterrit au bon endroit, peu importe qui l'a ajouté
//    quand. Les sections non déclarées suivent, dans l'ordre de première apparition ;
//    les items sans section forment le groupe ''. OPT-IN : sans aucun `set_section`
//    ni `add_sections`, rien ne bouge — les barres en `add_separator` manuel sont
//    inchangées ; en mode sections, les séparateurs manuels sont retirés.
// ═══════════════════════════════════════════════════════════════════════════

/// Le sens dans lequel la barre range ses items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarOrientation {
    Horizontal,
    Vertical,
}

/// Au-delà, on tient un déplacement et plus un clic (le seuil de DocWidget).
pub const DRAG_THRESHOLD: f64 = 4.0;

/// Le retrait gardé entre la barre et le bord de son parent (celui du menu).
pub const MARGE_BORD: f64 = 8.0;

/// Un item de barre (le `ToolbarItem` sans son DOM).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ToolbarItem {
    /// L'infobulle — et le seul a11y du projet aujourd'hui (`aria-label`).
    pub tooltip: String,
    pub label: String,
    pub icon: String,
    /// Le groupe d'exclusivité ('' = aucun).
    pub group: String,
    /// La section de cet item ('' = sans section).
    pub section: String,
    pub active: bool,
    pub disabled: bool,
}

/// Un enfant de la barre, dans l'ordre d'affichage (le `dom.children` du TS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enfant {
    /// Une `.toolbar-section` : son `data-section` ('' par défaut) et ses items (index).
    Section { data_section: String, items: Vec<usize> },
    /// Un `.toolbar-separator`.
    Separateur,
    /// La poignée : toujours le DERNIER enfant.
    Poignee,
}

/// La forme affichée : `Sep` pour un séparateur, sinon `(section, [étiquettes])`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Forme {
    Sep,
    Section(String, Vec<String>),
}

/// Le rectangle du parent (coordonnées CLIENT).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Le geste de déplacement à la poignée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grab {
    pub dx: f64,
    pub dy: f64,
    pub moved: bool,
}

pub struct Toolbar {
    items: Vec<ToolbarItem>,
    /// L'ordre des sections déclaré par l'OUVREUR de la barre (voir `add_sections`).
    declared_sections: Vec<String>,
    /// Les enfants SANS la poignée (rendue en dernier par `enfants`).
    children: Vec<Enfant>,
    /// L'index, dans `children`, de la section en cours de remplissage.
    section_courante: usize,
    hide_callbacks: Vec<Box<dyn FnMut()>>,
    shown: bool,
    orientation: ToolbarOrientation,
    columns: usize,
    /// L'utilisateur a-t-il posé la barre lui-même ? Une fois vrai, l'appelant doit
    /// cesser de la recentrer : sa position devient un choix, pas un défaut.
    user_placed: bool,
    grab: Option<Grab>,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

/// La poignée du builder : un item tout juste ajouté, modifiable en chaîne.
pub struct ItemMut<'a> {
    toolbar: &'a mut Toolbar,
    index: usize,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            declared_sections: Vec::new(),
            children: vec![Self::nouvelle_section_vide()],
            section_courante: 0,
            hide_callbacks: Vec::new(),
            shown: false,
            orientation: ToolbarOrientation::Horizontal,
            columns: 2,
            user_placed: false,
            grab: None,
        }
    }

    fn nouvelle_section_vide() -> Enfant {
        Enfant::Section {
            data_section: String::new(),
            items: Vec::new(),
        }
    }

    /// Ferme la section courante et en ouvre une neuve (à la fin).
    fn ouvrir_section(&mut self) {
        self.children.push(Self::nouvelle_section_vide());
        self.section_courante = self.children.len() - 1;
    }

    /// Le builder central.
    pub fn add_item(&mut self, cb: impl FnOnce(&mut ItemMut<'_>)) -> &mut Self {
        let index = self.items.len();
        self.items.push(ToolbarItem::default());
        if let Some(Enfant::Section { items, .. }) = self.children.get_mut(self.section_courante) {
            items.push(index);
        }
        cb(&mut ItemMut {
            toolbar: self,
            index,
        });
        self.apply_sections();
        self
    }

    /// Déclare l'ordre des sections de cette barre — appelé par celui qui la CONSTRUIT,
    /// avant de la passer aux plugins qui l'enrichissent.
    pub fn add_sections(&mut self, sections: &[&str]) -> &mut Self {
        self.declared_sections
            .extend(sections.iter().map(|s| s.to_string()));
        self.apply_sections();
        self
    }

    /// Regroupe les items par section : une section par groupe, un séparateur ENTRE
    /// deux groupes.
    ///
    /// ★ Différence avec Menu : un menu se fige UNE fois, à l'affichage ; une barre est
    ///   PERSISTANTE — un plugin y ajoute un item pendant qu'elle est montrée. Le
    ///   regroupement est donc rejoué à chaque ajout (`add_item`, `set_section`,
    ///   `add_sections`), de façon synchrone.
    pub fn apply_sections(&mut self) {
        if self.declared_sections.is_empty() && self.items.iter().all(|i| i.section.is_empty()) {
            return;
        }
        self.children.clear();

        let mut ordre = self.declared_sections.clone();
        for item in &self.items {
            if !ordre.contains(&item.section) {
                ordre.push(item.section.clone());
            }
        }

        let mut derniere: Option<usize> = None;
        for id in ordre {
            let groupe: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| i.section == id)
                .map(|(n, _)| n)
                .collect();
            if groupe.is_empty() {
                continue;
            }
            if derniere.is_some() {
                self.children.push(Enfant::Separateur);
            }
            self.children.push(Enfant::Section {
                data_section: id,
                items: groupe,
            });
            derniere = Some(self.children.len() - 1);
        }
        // Un `add_item` suivant atterrit dans la dernière section, puis est re-rangé.
        match derniere {
            Some(i) => self.section_courante = i,
            None => self.ouvrir_section(),
        }
    }

    /// Ferme la section courante et en ouvre une neuve.
    pub fn add_separator(&mut self) -> &mut Self {
        self.children.push(Enfant::Separateur);
        self.ouvrir_section();
        self
    }

    /// Vide la barre de ses items (sections et séparateurs) SANS la fermer : elle garde
    /// sa position, son orientation, et le fait d'avoir été placée à la main. Sert à la
    /// reconstruire quand son contenu change (un plugin qui ajoute un outil à chaud).
    pub fn clear(&mut self) -> &mut Self {
        self.children.clear();
        self.items.clear();
        // Reconstruire = redéclarer : l'ouvreur rappelle `add_sections`.
        self.declared_sections.clear();
        self.ouvrir_section();
        self
    }

    /// Le nombre de colonnes sur lequel CHAQUE section range ses items.
    ///
    /// ★ Ne mord qu'en mode VERTICAL. À l'horizontale, une section est une ligne :
    ///   parler de colonnes y voudrait dire « combien d'items avant de passer à la
    ///   ligne », c'est-à-dire l'inverse ; l'horizontale ignore le réglage. Défaut : 2.
    ///   Rend `true` si la valeur a changé (l'hôte rejoue alors `move_to`).
    pub fn set_columns(&mut self, columns: f64) -> bool {
        let n = columns.floor().max(1.0) as usize;
        if self.columns == n {
            return false;
        }
        self.columns = n;
        true
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn on_hide(&mut self, callback: impl FnMut() + 'static) -> &mut Self {
        self.hide_callbacks.push(Box::new(callback));
        self
    }

    /// Bascule le sens de la barre. Rend `true` si le sens a changé (le bornage doit
    /// être rejoué : une barre horizontale posée en bas de l'écran dépasserait en
    /// passant à la verticale).
    pub fn set_orientation(&mut self, orientation: ToolbarOrientation) -> bool {
        if self.orientation == orientation {
            return false;
        }
        self.orientation = orientation;
        true
    }

    pub fn toggle_orientation(&mut self) -> bool {
        self.set_orientation(match self.orientation {
            ToolbarOrientation::Vertical => ToolbarOrientation::Horizontal,
            ToolbarOrientation::Horizontal => ToolbarOrientation::Vertical,
        })
    }

    pub fn orientation(&self) -> ToolbarOrientation {
        self.orientation
    }

    /// L'icône de la poignée : elle pivote avec la barre (le grip d'un rangement vertical
    /// se lit à l'horizontale, et inversement).
    pub fn icone_de_poignee(&self) -> &'static str {
        match self.orientation {
            ToolbarOrientation::Vertical => "grip-horizontal",
            ToolbarOrientation::Horizontal => "grip-vertical",
        }
    }

    /// L'utilisateur a-t-il déplacé la barre à la main ?
    pub fn is_user_placed(&self) -> bool {
        self.user_placed
    }

    /// Montre la barre (l'hôte l'attache à son parent, puis appelle `position_dans_parent`).
    /// Sans effet si elle l'est déjà.
    pub fn show(&mut self) -> bool {
        if self.shown {
            return false;
        }
        self.shown = true;
        true
    }

    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// Tout le démontage vit dans `onunload` : la barre se ferme aussi bien par `hide()`
    /// direct que par la cascade `unload()` de son propriétaire. Les `on_hide` ne
    /// partent qu'une fois, et seulement si elle était montrée.
    pub fn hide(&mut self) -> &mut Self {
        if self.shown {
            self.shown = false;
            for cb in &mut self.hide_callbacks {
                cb();
            }
        }
        self
    }

    pub fn items(&self) -> &[ToolbarItem] {
        &self.items
    }

    /// Les enfants dans l'ordre d'affichage, la poignée TOUJOURS dernière.
    pub fn enfants(&self) -> Vec<Enfant> {
        let mut e = self.children.clone();
        e.push(Enfant::Poignee);
        e
    }

    /// Les sections affichées, dans l'ordre (les sections vides n'apparaissent pas).
    pub fn forme(&self) -> Vec<Forme> {
        self.children
            .iter()
            .filter_map(|c| match c {
                Enfant::Separateur => Some(Forme::Sep),
                Enfant::Section { data_section, items } if !items.is_empty() => {
                    Some(Forme::Section(
                        data_section.clone(),
                        items
                            .iter()
                            .map(|i| self.items[*i].tooltip.clone())
                            .collect(),
                    ))
                }
                _ => None,
            })
            .collect()
    }

    /// Un item d'un groupe vient d'être activé : on désactive ses pairs. C'est la barre
    /// qui arbitre, pas les items entre eux : un item ne connaît que son groupe, jamais
    /// ses voisins.
    fn activate_in_group(&mut self, index: usize) {
        let groupe = self.items[index].group.clone();
        if groupe.is_empty() {
            return;
        }
        for (i, other) in self.items.iter_mut().enumerate() {
            if i != index && other.group == groupe {
                other.active = false;
            }
        }
    }

    /// Le clic sur l'item `index` : rien s'il est désactivé ; un item de groupe
    /// s'active au passage. ★ On NE ferme PAS la barre (à l'inverse de MenuItem) :
    /// c'est la différence entre armer un mode et déclencher une action.
    /// Rend `true` si l'action de l'item doit être appelée.
    pub fn click(&mut self, index: usize) -> bool {
        let Some(item) = self.items.get(index) else {
            return false;
        };
        if item.disabled {
            return false;
        }
        if !item.group.is_empty() {
            ItemMut {
                toolbar: self,
                index,
            }
            .set_active(true);
        }
        true
    }

    // ── placement ────────────────────────────────────────────────────────────

    /// La position (`left`, `top`) à écrire dans le style de la barre, dans le repère du
    /// PARENT, pour un point CLIENT `(x, y)` : borne la barre (`largeur` × `hauteur`) à
    /// la zone du parent.
    ///
    /// ★ Le max APRÈS le min, comme le menu : un parent plus petit que la barre la range
    ///   au coin haut-gauche plutôt que de la pousser dehors. Les termes de scroll valent
    ///   zéro sur une `.view-content` mais rendent le calcul juste si le parent scrolle.
    pub fn position_dans_parent(
        x: f64,
        y: f64,
        largeur: f64,
        hauteur: f64,
        zone: Zone,
        scroll_left: f64,
        scroll_top: f64,
    ) -> (f64, f64) {
        let px = (zone.left + MARGE_BORD).max(x.min(zone.right - largeur - MARGE_BORD));
        let py = (zone.top + MARGE_BORD).max(y.min(zone.bottom - hauteur - MARGE_BORD));
        (px - zone.left + scroll_left, py - zone.top + scroll_top)
    }

    // ── le drag à la poignée ─────────────────────────────────────────────────

    /// `pointerdown` : l'écart entre le pointeur et le coin de la barre, figé.
    pub fn debut_de_drag(&mut self, client_x: f64, client_y: f64, left: f64, top: f64) {
        self.grab = Some(Grab {
            dx: client_x - left,
            dy: client_y - top,
            moved: false,
        });
    }

    /// `pointermove` : la position où poser la barre (CLIENT), ou `None` tant que le
    /// seuil n'est pas franchi.
    ///
    /// ★ Le seuil de 4px est celui de DocWidget : sans lui, un double-clic sur la poignée
    ///   (deux pointerdown suivis de micro-mouvements) déplacerait la barre de deux pixels
    ///   avant de la faire pivoter. Dès le premier vrai déplacement, la position devient
    ///   un CHOIX (`user_placed`).
    pub fn deplacement_de_drag(
        &mut self,
        client_x: f64,
        client_y: f64,
        left: f64,
        top: f64,
    ) -> Option<(f64, f64)> {
        let grab = self.grab.as_mut()?;
        let x = client_x - grab.dx;
        let y = client_y - grab.dy;
        if !grab.moved {
            if (x - left).abs() < DRAG_THRESHOLD && (y - top).abs() < DRAG_THRESHOLD {
                return None;
            }
            grab.moved = true;
        }
        self.user_placed = true;
        Some((x, y))
    }

    /// `pointerup` / `pointercancel` / `lostpointercapture` : perdre le focus fenêtre
    /// (alt-tab, capture d'écran) ne délivre jamais de pointerup, et la barre resterait
    /// collée au curseur — les trois fins de geste sont traitées pareil.
    pub fn fin_de_drag(&mut self) {
        self.grab = None;
    }

    pub fn en_drag(&self) -> bool {
        self.grab.is_some_and(|g| g.moved)
    }
}

impl ItemMut<'_> {
    pub fn set_icon(&mut self, icon: &str) -> &mut Self {
        self.toolbar.items[self.index].icon = icon.into();
        self
    }

    pub fn set_label(&mut self, label: &str) -> &mut Self {
        self.toolbar.items[self.index].label = label.into();
        self
    }

    /// L'infobulle — et le seul a11y du projet aujourd'hui.
    pub fn set_tooltip(&mut self, tooltip: &str) -> &mut Self {
        self.toolbar.items[self.index].tooltip = tooltip.into();
        self
    }

    /// Inscrit cet item dans un groupe exclusif : l'activer désactive ses pairs. C'est ce
    /// qui fait des boutons crayon / surligneur / gomme un vrai choix d'outil plutôt que
    /// trois bascules indépendantes.
    pub fn set_group(&mut self, group: &str) -> &mut Self {
        self.toolbar.items[self.index].group = group.into();
        self
    }

    /// Place cet item dans une section — le placement devient DÉCLARATIF. Rangé tout de
    /// suite, que l'appel vienne du builder (avant que l'item soit compté) ou après.
    pub fn set_section(&mut self, section: &str) -> &mut Self {
        self.toolbar.items[self.index].section = section.into();
        self.toolbar.apply_sections();
        self
    }

    pub fn set_active(&mut self, active: bool) -> &mut Self {
        self.toolbar.items[self.index].active = active;
        if active {
            self.toolbar.activate_in_group(self.index);
        }
        self
    }

    pub fn is_active(&self) -> bool {
        self.toolbar.items[self.index].active
    }

    pub fn set_disabled(&mut self, disabled: bool) -> &mut Self {
        self.toolbar.items[self.index].disabled = disabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn sec(id: &str, labels: &[&str]) -> Forme {
        Forme::Section(id.into(), labels.iter().map(|s| s.to_string()).collect())
    }

    // ═══ Les SECTIONS de la barre — le contrat de Menu.setSection / addSections ═══

    #[test]
    fn range_par_section_declaree_puis_les_autres_dans_l_ordre_de_1re_apparition() {
        let mut bar = Toolbar::new();
        bar.add_sections(&["tool", "style"]);
        bar.add_item(|i| {
            i.set_tooltip("Rouge").set_section("style");
        });
        bar.add_item(|i| {
            i.set_tooltip("Ask").set_section("agent");
        });
        bar.add_item(|i| {
            i.set_tooltip("Crayon").set_section("tool");
        });
        bar.add_item(|i| {
            i.set_tooltip("Gomme").set_section("tool");
        });
        assert_eq!(
            bar.forme(),
            vec![
                sec("tool", &["Crayon", "Gomme"]),
                Forme::Sep,
                sec("style", &["Rouge"]),
                Forme::Sep,
                sec("agent", &["Ask"]),
            ]
        );
    }

    #[test]
    fn pas_de_separateur_pour_une_section_declaree_vide_les_separateurs_manuels_sont_retires() {
        let mut bar = Toolbar::new();
        bar.add_sections(&["a", "vide", "b"]);
        bar.add_item(|i| {
            i.set_tooltip("x").set_section("a");
        });
        bar.add_separator();
        bar.add_item(|i| {
            i.set_tooltip("y").set_section("b");
        });
        assert_eq!(
            bar.forme(),
            vec![sec("a", &["x"]), Forme::Sep, sec("b", &["y"])]
        );
    }

    #[test]
    fn opt_in_sans_section_la_barre_garde_ses_separateurs_manuels() {
        let mut bar = Toolbar::new();
        bar.add_item(|i| {
            i.set_tooltip("x");
        });
        bar.add_separator();
        bar.add_item(|i| {
            i.set_tooltip("y");
        });
        assert_eq!(
            bar.forme(),
            vec![sec("", &["x"]), Forme::Sep, sec("", &["y"])]
        );
    }

    #[test]
    fn un_item_ajoute_apres_l_affichage_est_range_la_poignee_reste_derniere() {
        let mut bar = Toolbar::new();
        bar.add_sections(&["tool", "style"]);
        bar.add_item(|i| {
            i.set_tooltip("Rouge").set_section("style");
        });
        bar.show();
        bar.add_item(|i| {
            i.set_tooltip("Crayon").set_section("tool");
        });
        assert_eq!(
            bar.forme(),
            vec![sec("tool", &["Crayon"]), Forme::Sep, sec("style", &["Rouge"])]
        );
        assert_eq!(bar.enfants().last(), Some(&Enfant::Poignee));
        bar.hide();
    }

    #[test]
    fn clear_oublie_les_sections_declarees_l_ouvreur_les_redeclare() {
        let mut bar = Toolbar::new();
        bar.add_sections(&["b", "a"]);
        bar.add_item(|i| {
            i.set_tooltip("x").set_section("a");
        });
        bar.clear();
        bar.add_item(|i| {
            i.set_tooltip("y");
        });
        bar.add_separator();
        bar.add_item(|i| {
            i.set_tooltip("z");
        });
        assert_eq!(
            bar.forme(),
            vec![sec("", &["y"]), Forme::Sep, sec("", &["z"])]
        );
    }

    // ═══ ajouts du port : groupes, durée de vie, placement, drag ═══

    #[test]
    fn un_groupe_est_exclusif_et_le_clic_active_l_item() {
        let mut bar = Toolbar::new();
        for nom in ["crayon", "gomme"] {
            bar.add_item(|i| {
                i.set_tooltip(nom).set_group("outil");
            });
        }
        assert!(bar.click(0));
        assert!(bar.items()[0].active && !bar.items()[1].active);
        assert!(bar.click(1));
        assert!(!bar.items()[0].active && bar.items()[1].active);
    }

    #[test]
    fn un_item_desactive_ne_repond_pas() {
        let mut bar = Toolbar::new();
        bar.add_item(|i| {
            i.set_group("g").set_disabled(true);
        });
        assert!(!bar.click(0));
        assert!(!bar.items()[0].active);
    }

    #[test]
    fn on_hide_ne_part_qu_une_fois_et_seulement_si_montree() {
        let appels = Rc::new(Cell::new(0));
        let mut bar = Toolbar::new();
        let a = appels.clone();
        bar.on_hide(move || a.set(a.get() + 1));
        bar.hide();
        assert_eq!(appels.get(), 0);
        assert!(bar.show());
        assert!(!bar.show());
        bar.hide();
        bar.hide();
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn colonnes_et_orientation() {
        let mut bar = Toolbar::new();
        assert_eq!(bar.columns(), 2);
        assert!(!bar.set_columns(2.9));
        assert!(bar.set_columns(0.0));
        assert_eq!(bar.columns(), 1);
        assert_eq!(bar.icone_de_poignee(), "grip-vertical");
        assert!(bar.toggle_orientation());
        assert_eq!(bar.orientation(), ToolbarOrientation::Vertical);
        assert_eq!(bar.icone_de_poignee(), "grip-horizontal");
        assert!(!bar.set_orientation(ToolbarOrientation::Vertical));
    }

    #[test]
    fn le_bornage_au_parent_le_max_apres_le_min() {
        let zone = Zone {
            left: 100.0,
            top: 50.0,
            right: 400.0,
            bottom: 250.0,
        };
        let pos = |x, y, w, h| Toolbar::position_dans_parent(x, y, w, h, zone, 0.0, 0.0);
        // Dans la zone : repère du parent.
        assert_eq!(pos(150.0, 100.0, 40.0, 30.0), (50.0, 50.0));
        // Trop à gauche / en haut : retenue de 8px.
        assert_eq!(pos(0.0, 0.0, 40.0, 30.0), (8.0, 8.0));
        // Trop à droite / en bas.
        assert_eq!(pos(999.0, 999.0, 40.0, 30.0), (400.0 - 40.0 - 8.0 - 100.0, 250.0 - 30.0 - 8.0 - 50.0));
        // Un parent plus petit que la barre : coin haut-gauche.
        assert_eq!(pos(150.0, 100.0, 900.0, 900.0), (8.0, 8.0));
        // Le scroll du parent s'ajoute.
        assert_eq!(
            Toolbar::position_dans_parent(150.0, 100.0, 40.0, 30.0, zone, 5.0, 7.0),
            (55.0, 57.0)
        );
    }

    #[test]
    fn le_drag_attend_le_seuil_puis_la_position_devient_un_choix() {
        let mut bar = Toolbar::new();
        // Le pointeur attrape la poignée à (110, 60) ; la barre est en (100, 50).
        bar.debut_de_drag(110.0, 60.0, 100.0, 50.0);
        // 2px de mouvement : sous le seuil, rien.
        assert_eq!(bar.deplacement_de_drag(112.0, 61.0, 100.0, 50.0), None);
        assert!(!bar.is_user_placed() && !bar.en_drag());
        // 10px : on déplace, de l'écart gardé.
        assert_eq!(
            bar.deplacement_de_drag(120.0, 60.0, 100.0, 50.0),
            Some((110.0, 50.0))
        );
        assert!(bar.is_user_placed() && bar.en_drag());
        // Une fois lancé, plus de seuil.
        assert_eq!(
            bar.deplacement_de_drag(121.0, 60.0, 110.0, 50.0),
            Some((111.0, 50.0))
        );
        bar.fin_de_drag();
        assert!(!bar.en_drag());
        assert_eq!(bar.deplacement_de_drag(500.0, 500.0, 0.0, 0.0), None);
        assert!(bar.is_user_placed(), "le choix de l'utilisateur reste");
    }
}
