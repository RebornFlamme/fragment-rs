// Port de app/src/ui/core/DragManager.ts
//
// Le glisser-déposer d'un onglet ou d'un fichier : QUOI est tiré (`DragData`), et OÙ
// l'on le dépose (routage du lâcher). Le TS mélange ce routage avec le DOM (capture de
// pointeur, fantôme `.drag-ghost`, `elementFromPoint`) ; ici le routage seul est dans le
// cœur, derrière le trait `DragHost` (le Workspace + le hit-test de l'explorateur), et
// les évènements du pointeur arrivent par `on_move` / `on_up` / `on_cancel`. La capture
// du pointeur et le fantôme (une pilule suivie au curseur) restent dans fragment-ui,
// qui lit `ghost_label` et `GHOST_OFFSET`.
//
// Évènements (`Events`, noms du TS) : `drag-start` (`DragData`), `drag-end` (`()`),
// `file-drag-over` (`Option<String>` : le dossier survolé de l'explorateur), `file-move`
// (`FileMove`), `drop-external` (`DropExternal`).
//
// ★ ÉCHECS DU TEST TS (2 préexistants, NON reproduits comme échecs ici). Lecture du code
//   (le test n'a pas été relancé) : les deux cas lèvent vraisemblablement dans jsdom, qui
//   n'implémente pas `document.elementFromPoint`, appelé par `explorerPathAt` dès le
//   lâcher d'un fichier ; le second a en plus une attente PÉRIMÉE : « hors sidedock → drop-external »,
//   alors que le code ouvre désormais le fichier dans la pile sous le curseur dès qu'il
//   y en a une (le split décidé par la zone). Le premier cas est porté tel quel (il
//   passe : le routage est juste) ; le second est `#[ignore]` avec son attente d'origine,
//   et remplacé par des cas qui décrivent le comportement ACTUEL.

use crate::events::Events;

/// Le décalage du fantôme par rapport au curseur : il se pose À CÔTÉ, pas dessous.
pub const GHOST_OFFSET: (f64, f64) = (12.0, 8.0);

/// Ce que le Workspace (et le hit-test de l'explorateur) rendent au DragManager.
pub trait DragHost {
    type Leaf: Clone + 'static;
    type File: Clone + 'static;
    /// Une pile d'onglets (`WorkspaceTabs`).
    type Tabs: Clone;

    /// Le chemin visé DANS l'explorateur au point donné, ou `None` hors de lui. `""` =
    /// zone vide de l'arbre (racine du coffre).
    fn explorer_path_at(&self, x: f64, y: f64) -> Option<String>;
    fn clear_drop_indicator(&self);
    fn show_drop_indicator_at(&self, x: f64, y: f64);
    fn tabs_at_point(&self, x: f64, y: f64) -> Option<Self::Tabs>;
    fn create_leaf_in_parent(&self, tabs: &Self::Tabs) -> Self::Leaf;
    /// Cette pile est-elle DANS un panneau latéral (sidedock) ?
    fn in_sidedock(&self, tabs: &Self::Tabs) -> bool;
    fn drop_leaf(&self, tabs: &Self::Tabs, leaf: &Self::Leaf, x: f64, y: f64);
    /// Ouvre le fichier dans la feuille PUIS la rend active (le `.then(setActiveLeaf)` du TS).
    fn open_file_and_activate(&self, leaf: &Self::Leaf, file: &Self::File);
    /// Le fichier que montre une feuille, s'il y en a un.
    fn leaf_file(&self, leaf: &Self::Leaf) -> Option<Self::File>;
}

/// Ce qu'on tire.
#[derive(Debug, Clone, PartialEq)]
pub enum DragData<L, F> {
    Leaf(L),
    File(F),
}

/// Payload de `file-move` : déplacer `file` dans le dossier `target`.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMove<F> {
    pub file: F,
    pub target: String,
}

/// Payload de `drop-external` : `file` lâché au point (x, y) hors de toute pile.
#[derive(Debug, Clone, PartialEq)]
pub struct DropExternal<F> {
    pub file: F,
    pub x: f64,
    pub y: f64,
}

pub struct DragManager<H: DragHost> {
    host: H,
    events: Events,
    pub source: Option<DragData<H::Leaf, H::File>>,
    /// Le texte de la pilule fantôme (présentation : lu par fragment-ui).
    pub ghost_label: Option<String>,
    /// Un drag est en cours (le `captureEl` du TS) : rend `end_drag` idempotent.
    active: bool,
}

impl<H: DragHost> DragManager<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            events: Events::new(),
            source: None,
            ghost_label: None,
            active: false,
        }
    }

    pub fn events(&self) -> &Events {
        &self.events
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn is_dragging(&self) -> bool {
        self.active
    }

    pub fn start_drag(&mut self, data: DragData<H::Leaf, H::File>, label: &str) {
        self.source = Some(data.clone());
        self.ghost_label = Some(label.to_owned());
        self.active = true;
        self.events.trigger("drag-start", &data);
    }

    /// `pointermove`. `alt` = mode « widget flottant » : pas d'aperçu de split, pour ne
    /// PAS entrer en conflit avec le splitscreen du workspace.
    pub fn on_move(&self, x: f64, y: f64, alt: bool) {
        match &self.source {
            // Drag de feuille : le trait d'insertion des onglets (partout).
            Some(DragData::Leaf(_)) => self.indicateur(x, y, alt),
            // Drag de FICHIER — au-dessus de l'EXPLORATEUR, c'est un DÉPLACEMENT dans un
            // dossier, surtout PAS un split de vue : aucun indicateur, et on signale le
            // dossier survolé (l'explorateur colore sa ligne).
            Some(DragData::File(_)) => match self.host.explorer_path_at(x, y) {
                Some(cible) => {
                    self.host.clear_drop_indicator();
                    self.events.trigger("file-drag-over", &Some(cible));
                }
                None => {
                    self.events.trigger("file-drag-over", &None::<String>);
                    // Ailleurs, MÊME aperçu qu'un drag d'onglet.
                    self.indicateur(x, y, alt);
                }
            },
            None => {}
        }
    }

    fn indicateur(&self, x: f64, y: f64, alt: bool) {
        if alt {
            self.host.clear_drop_indicator();
        } else {
            self.host.show_drop_indicator_at(x, y);
        }
    }

    /// `pointerup` : le lâcher.
    pub fn on_up(&mut self, x: f64, y: f64, alt: bool) {
        let src = self.source.clone();
        // end_drag AVANT le commit : drop_leaf reconstruit l'arbre et détruit l'onglet
        // capturé ; on relâche la capture tant qu'il est encore vivant.
        self.end_drag();
        let Some(src) = src else {
            return;
        };

        match src {
            // Drag de FICHIER (explorateur).
            DragData::File(file) => {
                // Au-dessus de l'EXPLORATEUR → déplacement dans le dossier survolé.
                // Prioritaire sur tout le reste.
                if let Some(target) = self.host.explorer_path_at(x, y) {
                    self.events.trigger("file-move", &FileMove { file, target });
                    return;
                }
                // Alt → widget flottant (doc-widget), comme pour un onglet.
                if alt {
                    self.events
                        .trigger("drop-external", &DropExternal { file, x, y });
                    return;
                }
                // Sinon → ouvrir le fichier dans la pile sous le curseur. Dans le MAIN, la
                // ZONE (centre = rejoint, bord = split) décide, comme un drag d'onglet ;
                // dans un sidedock, on rejoint simplement (pas de split).
                if let Some(pile) = self.host.tabs_at_point(x, y) {
                    let leaf = self.host.create_leaf_in_parent(&pile);
                    if !self.host.in_sidedock(&pile) {
                        self.host.drop_leaf(&pile, &leaf, x, y);
                    }
                    self.host.open_file_and_activate(&leaf, &file);
                    return;
                }
                // Hors de toute pile → widget flottant.
                self.events
                    .trigger("drop-external", &DropExternal { file, x, y });
            }
            // Drag d'ONGLET : Alt = widget (sur son fichier), sinon split/move dans une
            // pile — les deux chemins ne se croisent jamais.
            DragData::Leaf(leaf) => {
                if alt {
                    if let Some(file) = self.host.leaf_file(&leaf) {
                        self.events
                            .trigger("drop-external", &DropExternal { file, x, y });
                    }
                    return;
                }
                if let Some(pile) = self.host.tabs_at_point(x, y) {
                    self.host.drop_leaf(&pile, &leaf, x, y);
                }
            }
        }
    }

    /// Le drag est INTERROMPU (capture perdue, fenêtre défocalisée…) : le `pointerup`
    /// n'arrivera jamais. On avorte proprement, sans commit.
    pub fn on_cancel(&mut self) {
        self.end_drag();
    }

    /// Idempotent (`pointercancel` après `pointerup`, double blur…).
    pub fn end_drag(&mut self) {
        if !self.active {
            return;
        }
        self.source = None;
        self.ghost_label = None;
        self.active = false;
        self.host.clear_drop_indicator();
        self.events.trigger("drag-end", &());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Journal {
        leaves_creees: usize,
        ouvertures: Vec<String>,
        drops: Vec<(f64, f64)>,
        indicateurs: usize,
        effaces: usize,
    }

    /// Un workspace factice : une pile sous le curseur (ou non), dans un sidedock (ou non).
    #[derive(Clone)]
    struct Hote {
        pile: bool,
        sidedock: bool,
        explorateur: Option<String>,
        j: Rc<RefCell<Journal>>,
    }

    impl Hote {
        fn new(pile: bool, sidedock: bool) -> Self {
            Self {
                pile,
                sidedock,
                explorateur: None,
                j: Rc::default(),
            }
        }
    }

    impl DragHost for Hote {
        type Leaf = &'static str;
        type File = &'static str;
        type Tabs = ();
        fn explorer_path_at(&self, _: f64, _: f64) -> Option<String> {
            self.explorateur.clone()
        }
        fn clear_drop_indicator(&self) {
            self.j.borrow_mut().effaces += 1;
        }
        fn show_drop_indicator_at(&self, _: f64, _: f64) {
            self.j.borrow_mut().indicateurs += 1;
        }
        fn tabs_at_point(&self, _: f64, _: f64) -> Option<()> {
            self.pile.then_some(())
        }
        fn create_leaf_in_parent(&self, _: &()) -> &'static str {
            self.j.borrow_mut().leaves_creees += 1;
            "feuille"
        }
        fn in_sidedock(&self, _: &()) -> bool {
            self.sidedock
        }
        fn drop_leaf(&self, _: &(), _: &&'static str, x: f64, y: f64) {
            self.j.borrow_mut().drops.push((x, y));
        }
        fn open_file_and_activate(&self, _: &&'static str, file: &&'static str) {
            self.j.borrow_mut().ouvertures.push((*file).to_owned());
        }
        fn leaf_file(&self, leaf: &&'static str) -> Option<&'static str> {
            (*leaf == "onglet").then_some("note.md")
        }
    }

    const FICHIER: &str = "note.md";

    fn ecoute_drop_external(
        dm: &DragManager<Hote>,
    ) -> Rc<RefCell<Vec<DropExternal<&'static str>>>> {
        let vu: Rc<RefCell<Vec<DropExternal<&'static str>>>> = Rc::default();
        let v = vu.clone();
        dm.events().on("drop-external", move |d| {
            if let Some(e) = d.downcast_ref::<DropExternal<&'static str>>() {
                v.borrow_mut().push(e.clone());
            }
        });
        vu
    }

    // ─── DragManager — drop de fichier ───

    #[test]
    fn lache_sur_un_sidedock_ouvre_le_fichier_comme_feuille_dans_la_pile() {
        let hote = Hote::new(true, true);
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(500.0, 300.0, false);

        let j = j.borrow();
        assert_eq!(j.leaves_creees, 1);
        assert_eq!(j.ouvertures, vec!["note.md"]);
        // Dans un sidedock on rejoint simplement : pas de split.
        assert!(j.drops.is_empty());
    }

    #[test]
    #[ignore = "attente PÉRIMÉE du test TS (échec préexistant) : hors sidedock, le code \
                ouvre désormais le fichier dans la pile sous le curseur ; voir \
                lache_hors_dun_sidedock_ouvre_dans_la_pile_et_la_zone_decide"]
    fn lache_hors_dun_sidedock_widget_flottant_drop_external() {
        let hote = Hote::new(true, false);
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        let externe = ecoute_drop_external(&dm);
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(900.0, 400.0, false);

        assert_eq!(j.borrow().leaves_creees, 0);
        assert_eq!(
            *externe.borrow(),
            vec![DropExternal {
                file: FICHIER,
                x: 900.0,
                y: 400.0
            }]
        );
    }

    // ─── Ajoutés : le comportement ACTUEL du routage ───

    #[test]
    fn lache_hors_dun_sidedock_ouvre_dans_la_pile_et_la_zone_decide() {
        let hote = Hote::new(true, false);
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        let externe = ecoute_drop_external(&dm);
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(900.0, 400.0, false);

        let j = j.borrow();
        assert_eq!(j.leaves_creees, 1);
        assert_eq!(j.drops, vec![(900.0, 400.0)]);
        assert_eq!(j.ouvertures, vec!["note.md"]);
        assert!(externe.borrow().is_empty());
    }

    #[test]
    fn hors_de_toute_pile_widget_flottant() {
        let mut dm = DragManager::new(Hote::new(false, false));
        let externe = ecoute_drop_external(&dm);
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(10.0, 20.0, false);
        assert_eq!(externe.borrow().len(), 1);
    }

    #[test]
    fn alt_sur_un_fichier_widget_flottant_sans_toucher_aux_piles() {
        let hote = Hote::new(true, false);
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        let externe = ecoute_drop_external(&dm);
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(1.0, 2.0, true);
        assert_eq!(externe.borrow().len(), 1);
        assert_eq!(j.borrow().leaves_creees, 0);
    }

    #[test]
    fn lache_sur_lexplorateur_deplace_dans_le_dossier_prioritaire() {
        let mut hote = Hote::new(true, true);
        hote.explorateur = Some("dossier/sous".into());
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        let vu: Rc<RefCell<Vec<FileMove<&'static str>>>> = Rc::default();
        let v = vu.clone();
        dm.events().on("file-move", move |d| {
            if let Some(e) = d.downcast_ref::<FileMove<&'static str>>() {
                v.borrow_mut().push(e.clone());
            }
        });
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_up(5.0, 5.0, false);
        assert_eq!(
            *vu.borrow(),
            vec![FileMove {
                file: FICHIER,
                target: "dossier/sous".into()
            }]
        );
        assert_eq!(j.borrow().leaves_creees, 0);
    }

    #[test]
    fn drag_donglet_alt_widget_sur_son_fichier_sinon_drop_dans_la_pile() {
        let mut dm = DragManager::new(Hote::new(true, false));
        let externe = ecoute_drop_external(&dm);
        dm.start_drag(DragData::Leaf("onglet"), "titre");
        dm.on_up(7.0, 8.0, true);
        assert_eq!(externe.borrow().len(), 1);

        let hote = Hote::new(true, false);
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        dm.start_drag(DragData::Leaf("onglet"), "titre");
        dm.on_up(7.0, 8.0, false);
        assert_eq!(j.borrow().drops, vec![(7.0, 8.0)]);
    }

    #[test]
    fn end_drag_est_idempotent_et_annonce_drag_end_une_fois() {
        let mut dm = DragManager::new(Hote::new(false, false));
        let fins = Rc::new(RefCell::new(0));
        let f = fins.clone();
        dm.events().on("drag-end", move |_| *f.borrow_mut() += 1);
        dm.start_drag(DragData::Leaf("onglet"), "t");
        assert!(dm.is_dragging());
        dm.on_cancel();
        dm.end_drag();
        dm.on_up(0.0, 0.0, false); // pointerup tardif : rien à valider
        assert_eq!(*fins.borrow(), 1);
        assert!(dm.source.is_none());
    }

    #[test]
    fn le_survol_signale_le_dossier_et_masque_lindicateur_de_split() {
        let mut hote = Hote::new(true, false);
        hote.explorateur = Some(String::new());
        let j = hote.j.clone();
        let mut dm = DragManager::new(hote);
        let survols: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
        let s = survols.clone();
        dm.events().on("file-drag-over", move |d| {
            if let Some(c) = d.downcast_ref::<Option<String>>() {
                s.borrow_mut().push(c.clone());
            }
        });
        dm.start_drag(DragData::File(FICHIER), "note");
        dm.on_move(1.0, 1.0, false);
        assert_eq!(*survols.borrow(), vec![Some(String::new())]);
        assert_eq!(j.borrow().indicateurs, 0);
    }
}
