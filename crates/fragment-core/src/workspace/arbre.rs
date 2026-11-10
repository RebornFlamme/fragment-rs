// Port de app/src/ui/core/workspace/WorkspaceItem.ts et WorkspaceSplit.ts (la part modèle),
// avec la partie géométrique de Workspace.ts (espace des boutons de fenêtre, pile sous un point).
//
// L'arbre du workspace SANS DOM. Fragment fait vivre ses nœuds comme des objets qui se
// pointent mutuellement (enfant → parent, parent → enfants) et range la géométrie dans le
// `flex-grow` inline de leurs éléments. Ici, une ARÈNE : chaque nœud a un `ItemId`, les liens
// sont des identifiants, la proportion d'un pane est un champ du nœud (`grow`). Pas de
// `Rc<RefCell>`, pas de cycle, tout se teste sans GPUI (docs/tranches/T1.md §3, choix 2).
//
// Ce que le Workspace faisait EN PASSANT à chaque mutation (sauvegarde différée, émission de
// `layout-change`, recalcul de l'espace des boutons, traitement d'une pile vidée) devient ici
// des DRAPEAUX et une file que le Workspace vide explicitement : l'arbre ne connaît ni les
// événements, ni le disque, ni les vues.
//
// ★ Vocabulaire d'Obsidian, contre-intuitif mais repris tel quel (les thèmes et les plugins le
//   connaissent) : un split VERTICAL a une ligne de séparation verticale — ses enfants sont
//   côte à côte, en LIGNE ; un split HORIZONTAL les empile en colonne.

use std::collections::HashSet;

use thiserror::Error;

use super::SplitDirection;
use super::layout::{LayoutNode, LeafLayout, SplitLayout, TabsLayout};

/// L'identité d'un nœud dans l'arène. ★ Jamais réutilisée : un nœud libéré laisse sa case
/// vide, et le prochain nœud prend une case neuve. Deux feuilles ne peuvent donc pas
/// partager un identifiant, même l'une après la mort de l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(u32);

impl ItemId {
    /// Le rang de la case dans l'arène (diagnostic, journal).
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Un rectangle à l'écran, en px (coordonnées de la fenêtre). Fourni par la couche de rendu :
/// le cœur ne mesure rien, il raisonne sur ce qu'on lui donne.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, largeur: f64, hauteur: f64) -> Self {
        Self {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    pub fn droite(&self) -> f64 {
        self.x + self.largeur
    }

    pub fn bas(&self) -> f64 {
        self.y + self.hauteur
    }

    /// Bords INCLUS, comme le test de Fragment (`x < left || x > right` → dehors).
    pub fn contient(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.droite() && y >= self.y && y <= self.bas()
    }
}

/// Le côté d'un panneau latéral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cote {
    Gauche,
    Droite,
}

/// La racine d'une fenêtre (la zone centrale). Une seule par fenêtre. Vide pour l'instant :
/// c'est ici qu'iront la fenêtre et le document propres quand les fenêtres détachées
/// existeront (Obsidian intercale un `WorkspaceContainer` à cet étage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceRoot;

/// Ce qui est propre à un panneau latéral : son repli et la largeur choisie par l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkspaceSidedock {
    pub cote: Cote,
    pub collapsed: bool,
    /// `--sidebar-width` en px ; `None` tant que personne n'a tiré la poignée — et c'est ce
    /// qu'on sérialise alors : « pas de largeur choisie », la mise en page garde la main.
    pub width: Option<f64>,
}

/// Les trois sortes de split : un split intérieur (créé et détruit au gré des découpes), la
/// racine centrale et les deux docks — ces trois-là sont PERMANENTS, sans parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenreSplit {
    Interne,
    Root(WorkspaceRoot),
    Sidedock(WorkspaceSidedock),
}

/// Un split : des enfants côte à côte (vertical) ou empilés (horizontal), chacun avec sa part
/// de flex-grow (portée par le nœud enfant, voir `Arbre::grow`).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceSplit {
    pub direction: SplitDirection,
    pub genre: GenreSplit,
    pub(super) children: Vec<ItemId>,
}

/// Une pile d'onglets : des feuilles, une seule visible (`active_index`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorkspaceTabs {
    pub(super) children: Vec<ItemId>,
    pub(super) active_index: usize,
}

/// Ce qu'un nœud porte selon sa forme. `F` est la charge d'une feuille (la `WorkspaceLeaf`
/// du Workspace ; `()` dans les tests de l'arbre seul).
#[derive(Debug, Clone, PartialEq)]
pub enum Forme<F> {
    Split(WorkspaceSplit),
    Tabs(WorkspaceTabs),
    Leaf(F),
}

/// Une case de l'arène.
#[derive(Debug, Clone, PartialEq)]
pub struct Noeud<F> {
    pub(super) parent: Option<ItemId>,
    /// La part de flex-grow du nœud dans son split parent : LE MODÈLE de la géométrie.
    /// `None` = jamais posée = 1 (le `flex: 1` d'un pane neuf).
    pub(super) grow: Option<f64>,
    pub forme: Forme<F>,
}

/// Ce que tout nœud sait dire de lui-même (le `WorkspaceItem` de Fragment, sans DOM).
pub trait WorkspaceItem {
    /// `None` pour un nœud détaché et pour les trois racines permanentes.
    fn parent(&self) -> Option<ItemId>;
    /// La part de flex-grow (1 si jamais posée).
    fn dimension(&self) -> f64;
    fn est_feuille(&self) -> bool;
}

impl<F> WorkspaceItem for Noeud<F> {
    fn parent(&self) -> Option<ItemId> {
        self.parent
    }

    fn dimension(&self) -> f64 {
        self.grow.filter(|g| g.is_finite()).unwrap_or(1.0)
    }

    fn est_feuille(&self) -> bool {
        matches!(self.forme, Forme::Leaf(_))
    }
}

/// Un nœud qui a des enfants (le `WorkspaceParent` de Fragment).
pub trait WorkspaceParent {
    fn children(&self) -> &[ItemId];
}

impl WorkspaceParent for WorkspaceSplit {
    fn children(&self) -> &[ItemId] {
        &self.children
    }
}

impl WorkspaceParent for WorkspaceTabs {
    fn children(&self) -> &[ItemId] {
        &self.children
    }
}

/// Les refus de l'arbre. Fragment ne vérifiait rien (le DOM aurait levé, ou pire, accepté) :
/// l'arène refuse tout ce qui la rendrait incohérente.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErreurArbre {
    #[error("nœud inconnu ou libéré : {0:?}")]
    Inconnu(ItemId),
    #[error("{0:?} n'a pas d'enfants (c'est une feuille)")]
    PasUnParent(ItemId),
    #[error("{0:?} est une racine permanente : elle n'a jamais de parent")]
    Racine(ItemId),
    #[error("{enfant:?} contient {parent:?} : l'insérer dedans ferait un cycle")]
    Cycle { parent: ItemId, enfant: ItemId },
    #[error("une pile ne reçoit que des feuilles, un split que des piles ou des splits")]
    FormeRefusee { parent: ItemId, enfant: ItemId },
    #[error("{0:?} est encore attaché : le retirer de son parent avant de le libérer")]
    Attache(ItemId),
}

/// Ramène des flex-grow à une somme égale à leur nombre (moyenne 1), proportions gardées.
/// Un seul élément → 1 ; somme nulle ou négative → tout à 1.
///
/// ★ La parade au piège de flexbox (CSS Flexbox §9.7) : quand la somme des flex-grow est
///   inférieure à 1, le conteneur ne distribue qu'une FRACTION de l'espace libre et un trou
///   s'ouvre. Le survivant d'un retrait hérite facilement d'un 0,95 laissé par un
///   redimensionnement antérieur. Le rendu GPUI répartit à la même règle : on garde
///   l'invariant pour la même raison, et pour que le fichier relu par Fragment reste juste.
pub fn normalize_grows(grows: &[f64]) -> Vec<f64> {
    if grows.len() <= 1 {
        return vec![1.0; grows.len()];
    }
    let somme: f64 = grows.iter().sum();
    if !somme.is_finite() || somme <= 0.0 {
        return vec![1.0; grows.len()];
    }
    let facteur = grows.len() as f64 / somme;
    grows.iter().map(|g| g * facteur).collect()
}

/// La pile qui reçoit l'espace des boutons de fenêtre, et ce qu'elle doit réserver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EspaceBoutons {
    pub pile: ItemId,
    /// Combien de px de la barre d'onglets passent sous les boutons (`--frame-right-inset`) ;
    /// `None` hors mise en page : la réserve entière, le repli du rendu.
    pub recouvrement: Option<f64>,
}

/// La mise en page connue de la couche de rendu : le rectangle du workspace et celui de
/// chaque nœud (`None` = pas à l'écran).
pub struct MiseEnPage<'a> {
    pub workspace: Rect,
    pub rect: &'a dyn Fn(ItemId) -> Option<Rect>,
}

/// L'arbre du workspace : l'arène, les trois racines permanentes, et ce que les mutations
/// ont DEMANDÉ au Workspace (drapeaux + file, vidés par lui).
#[derive(Debug, Clone)]
pub struct Arbre<F> {
    noeuds: Vec<Option<Noeud<F>>>,
    root: ItemId,
    left: ItemId,
    right: ItemId,
    /// La STRUCTURE a changé depuis le dernier vidage (un ajout, un retrait, un déplacement).
    structure_changee: bool,
    /// Une sauvegarde du layout est due.
    sauvegarde_demandee: bool,
    /// Les piles qui ont perdu leur dernier onglet, dans l'ordre (sans doublon).
    piles_videes: Vec<ItemId>,
    /// Le parent d'une insertion en cours : la cascade des splits vidés ne l'emporte pas.
    protege: Option<ItemId>,
}

impl<F> Default for Arbre<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> Arbre<F> {
    /// Un arbre neuf : la racine centrale (split vertical : des colonnes) et deux docks
    /// (splits horizontaux : des piles empilées), vides et dépliés.
    pub fn new() -> Self {
        let mut a = Self {
            noeuds: Vec::new(),
            root: ItemId(0),
            left: ItemId(0),
            right: ItemId(0),
            structure_changee: false,
            sauvegarde_demandee: false,
            piles_videes: Vec::new(),
            protege: None,
        };
        let dock = |cote| {
            GenreSplit::Sidedock(WorkspaceSidedock {
                cote,
                collapsed: false,
                width: None,
            })
        };
        a.left = a.inserer(Forme::Split(WorkspaceSplit {
            direction: SplitDirection::Horizontal,
            genre: dock(Cote::Gauche),
            children: Vec::new(),
        }));
        a.root = a.inserer(Forme::Split(WorkspaceSplit {
            direction: SplitDirection::Vertical,
            genre: GenreSplit::Root(WorkspaceRoot),
            children: Vec::new(),
        }));
        a.right = a.inserer(Forme::Split(WorkspaceSplit {
            direction: SplitDirection::Horizontal,
            genre: dock(Cote::Droite),
            children: Vec::new(),
        }));
        a
    }

    // ─── Les racines ───

    /// La zone centrale (`rootSplit`).
    pub fn root(&self) -> ItemId {
        self.root
    }

    /// Le dock gauche (`leftSplit`).
    pub fn left(&self) -> ItemId {
        self.left
    }

    /// Le dock droit (`rightSplit`).
    pub fn right(&self) -> ItemId {
        self.right
    }

    pub fn dock(&self, cote: Cote) -> ItemId {
        match cote {
            Cote::Gauche => self.left,
            Cote::Droite => self.right,
        }
    }

    /// Vrai pour la racine centrale et les deux docks.
    pub fn est_racine_permanente(&self, id: ItemId) -> bool {
        id == self.root || id == self.left || id == self.right
    }

    // ─── Création (nœuds DÉTACHÉS : à insérer par `add_child`) ───

    fn inserer(&mut self, forme: Forme<F>) -> ItemId {
        // ★ Toujours une case NEUVE : jamais de recyclage d'identifiant.
        let id = ItemId(self.noeuds.len() as u32);
        self.noeuds.push(Some(Noeud {
            parent: None,
            grow: None,
            forme,
        }));
        id
    }

    /// Un split intérieur, détaché.
    pub fn creer_split(&mut self, direction: SplitDirection) -> ItemId {
        self.inserer(Forme::Split(WorkspaceSplit {
            direction,
            genre: GenreSplit::Interne,
            children: Vec::new(),
        }))
    }

    /// Une pile vide, détachée.
    pub fn creer_pile(&mut self) -> ItemId {
        self.inserer(Forme::Tabs(WorkspaceTabs::default()))
    }

    /// Une feuille portant `charge`, détachée.
    pub fn creer_feuille(&mut self, charge: F) -> ItemId {
        self.inserer(Forme::Leaf(charge))
    }

    // ─── Lecture ───

    pub fn noeud(&self, id: ItemId) -> Option<&Noeud<F>> {
        self.noeuds.get(id.0 as usize).and_then(Option::as_ref)
    }

    fn noeud_mut(&mut self, id: ItemId) -> Option<&mut Noeud<F>> {
        self.noeuds.get_mut(id.0 as usize).and_then(Option::as_mut)
    }

    pub fn existe(&self, id: ItemId) -> bool {
        self.noeud(id).is_some()
    }

    pub fn parent(&self, id: ItemId) -> Option<ItemId> {
        self.noeud(id).and_then(|n| n.parent)
    }

    /// Les enfants d'un split ou d'une pile ; vide pour une feuille ou un nœud inconnu.
    pub fn children(&self, id: ItemId) -> &[ItemId] {
        match self.noeud(id).map(|n| &n.forme) {
            Some(Forme::Split(s)) => &s.children,
            Some(Forme::Tabs(t)) => &t.children,
            _ => &[],
        }
    }

    pub fn split(&self, id: ItemId) -> Option<&WorkspaceSplit> {
        match self.noeud(id).map(|n| &n.forme) {
            Some(Forme::Split(s)) => Some(s),
            _ => None,
        }
    }

    pub fn pile(&self, id: ItemId) -> Option<&WorkspaceTabs> {
        match self.noeud(id).map(|n| &n.forme) {
            Some(Forme::Tabs(t)) => Some(t),
            _ => None,
        }
    }

    pub(super) fn pile_mut(&mut self, id: ItemId) -> Option<&mut WorkspaceTabs> {
        match self.noeud_mut(id).map(|n| &mut n.forme) {
            Some(Forme::Tabs(t)) => Some(t),
            _ => None,
        }
    }

    pub fn est_pile(&self, id: ItemId) -> bool {
        self.pile(id).is_some()
    }

    pub fn est_feuille(&self, id: ItemId) -> bool {
        self.noeud(id).is_some_and(|n| n.est_feuille())
    }

    /// La charge d'une feuille.
    pub fn feuille(&self, id: ItemId) -> Option<&F> {
        match self.noeud(id).map(|n| &n.forme) {
            Some(Forme::Leaf(f)) => Some(f),
            _ => None,
        }
    }

    pub fn feuille_mut(&mut self, id: ItemId) -> Option<&mut F> {
        match self.noeud_mut(id).map(|n| &mut n.forme) {
            Some(Forme::Leaf(f)) => Some(f),
            _ => None,
        }
    }

    /// Les données d'un dock (`None` si `id` n'est pas un dock).
    pub fn sidedock(&self, id: ItemId) -> Option<&WorkspaceSidedock> {
        match self.split(id).map(|s| &s.genre) {
            Some(GenreSplit::Sidedock(d)) => Some(d),
            _ => None,
        }
    }

    fn sidedock_mut(&mut self, id: ItemId) -> Option<&mut WorkspaceSidedock> {
        match self.noeud_mut(id).map(|n| &mut n.forme) {
            Some(Forme::Split(WorkspaceSplit {
                genre: GenreSplit::Sidedock(d),
                ..
            })) => Some(d),
            _ => None,
        }
    }

    /// Remonte jusqu'à la racine (`getRoot`) : le nœud lui-même s'il est détaché.
    pub fn get_root(&self, id: ItemId) -> ItemId {
        let mut courant = id;
        while let Some(p) = self.parent(courant) {
            courant = p;
        }
        courant
    }

    /// Vrai si `ancetre` est `id` ou l'un de ses ascendants.
    pub fn est_dans(&self, id: ItemId, ancetre: ItemId) -> bool {
        let mut courant = Some(id);
        while let Some(c) = courant {
            if c == ancetre {
                return true;
            }
            courant = self.parent(c);
        }
        false
    }

    /// La part de flex-grow d'un nœud dans son split (1 si jamais posée) : LE MODÈLE.
    pub fn grow(&self, id: ItemId) -> f64 {
        self.noeud(id).map_or(1.0, WorkspaceItem::dimension)
    }

    /// Pose une part de flex-grow, telle quelle (le redimensionnement, la restauration).
    /// Aucune renormalisation ici : elle a lieu au prochain changement de structure ou par
    /// `reflow_geometry`.
    pub fn poser_grow(&mut self, id: ItemId, grow: f64) {
        if let Some(n) = self.noeud_mut(id) {
            n.grow = Some(grow);
        }
    }

    /// Toutes les feuilles de l'arbre, dans l'ordre de Fragment (`iterateAllLeaves`) : dock
    /// gauche, centre, dock droit ; en profondeur, enfants dans l'ordre.
    pub fn feuilles(&self) -> Vec<ItemId> {
        let mut out = Vec::new();
        for r in [self.left, self.root, self.right] {
            self.feuilles_de(r, &mut out);
        }
        out
    }

    /// Les feuilles d'un sous-arbre, en profondeur.
    pub fn feuilles_sous(&self, id: ItemId) -> Vec<ItemId> {
        let mut out = Vec::new();
        self.feuilles_de(id, &mut out);
        out
    }

    fn feuilles_de(&self, id: ItemId, out: &mut Vec<ItemId>) {
        if self.est_feuille(id) {
            out.push(id);
            return;
        }
        for &c in self.children(id) {
            self.feuilles_de(c, out);
        }
    }

    /// Les piles d'un sous-arbre, en profondeur.
    pub fn piles_sous(&self, id: ItemId) -> Vec<ItemId> {
        let mut out = Vec::new();
        self.piles_de(id, &mut out);
        out
    }

    fn piles_de(&self, id: ItemId, out: &mut Vec<ItemId>) {
        if self.est_pile(id) {
            out.push(id);
            return;
        }
        for &c in self.children(id) {
            self.piles_de(c, out);
        }
    }

    // ─── Ce que les mutations demandent au Workspace ───

    fn changement(&mut self) {
        self.structure_changee = true;
        self.sauvegarde_demandee = true;
    }

    /// Demande une sauvegarde du layout (fin d'un redimensionnement, changement d'onglet…).
    pub fn demander_sauvegarde(&mut self) {
        self.sauvegarde_demandee = true;
    }

    /// ★ Vrai UNE fois si la structure a changé depuis le dernier appel, puis faux. C'est le
    ///   remplaçant de la microtâche de Fragment (`requestLayoutChange`) : un déplacement
    ///   (retrait + ajout) ou un split (plusieurs ajouts) ne doit produire QU'UNE émission de
    ///   `layout-change`. Le Workspace vide le drapeau après chaque opération et décide
    ///   d'émettre (seulement si le layout est prêt : la restauration émet la sienne à la fin).
    pub fn prendre_changement_de_structure(&mut self) -> bool {
        std::mem::take(&mut self.structure_changee)
    }

    /// Vrai une fois si une sauvegarde a été demandée depuis le dernier appel.
    pub fn prendre_sauvegarde_demandee(&mut self) -> bool {
        std::mem::take(&mut self.sauvegarde_demandee)
    }

    /// Les piles qui ont perdu leur dernier onglet depuis le dernier appel — seulement celles
    /// qui existent encore ET sont toujours vides.
    ///
    /// ★ Fragment traitait la pile vidée DANS `removeChild`, au milieu de l'opération. Le cas
    ///   « une feuille déposée au centre de sa propre pile » vidait alors la pile un instant,
    ///   et le Workspace la fermait (ou y ouvrait un onglet vide) avant que la feuille n'y
    ///   revienne. Traiter APRÈS l'opération, en revérifiant, supprime ce faux vide.
    pub fn prendre_piles_videes(&mut self) -> Vec<ItemId> {
        let file = std::mem::take(&mut self.piles_videes);
        file.into_iter()
            .filter(|&p| self.pile(p).is_some_and(|t| t.children.is_empty()))
            .collect()
    }

    // ─── Mutations de structure ───

    /// Insère `child` dans `parent` à `index` (`None` ou hors bornes : à la fin). Un nœud déjà
    /// attaché ailleurs en est d'abord retiré : un nœud n'est jamais dans deux arbres.
    ///
    /// Dans un split, la géométrie est rétablie (somme des flex-grow = nombre d'enfants) ; dans
    /// une pile, l'onglet inséré devient l'onglet visible.
    ///
    /// ★ Déplacer un nœud DANS son propre parent est un réordonnancement en place : Fragment
    ///   passait par un retrait complet, qui renormalisait les survivants (un pane 0,6 / 1,4
    ///   déplacé perdait ses proportions) et pouvait vider puis détruire le parent en chemin.
    pub fn add_child(
        &mut self,
        parent: ItemId,
        child: ItemId,
        index: Option<usize>,
    ) -> Result<(), ErreurArbre> {
        self.verifier_insertion(parent, child)?;

        if self.parent(child) == Some(parent) {
            self.reordonner(parent, child, index);
            return Ok(());
        }
        if let Some(ancien) = self.parent(child) {
            // ★ Le parent visé est PROTÉGÉ de la cascade : retirer `child` de son ancien split
            //   peut vider des splits intérieurs en chaîne, jusqu'au parent visé lui-même
            //   (un split qui ne contenait que l'ancien parent). Fragment insérait alors dans
            //   un split détaché, et le nœud disparaissait de l'arbre.
            self.protege = Some(parent);
            let retrait = self.remove_child(ancien, child);
            self.protege = None;
            retrait?;
        }

        let enfants = self
            .enfants_mut(parent)
            .ok_or(ErreurArbre::PasUnParent(parent))?;
        let i = match index {
            Some(i) if i < enfants.len() => i,
            _ => enfants.len(),
        };
        enfants.insert(i, child);
        if let Some(n) = self.noeud_mut(child) {
            n.parent = Some(parent);
        }
        self.changement();
        self.apres_ajout(parent, i);
        Ok(())
    }

    fn verifier_insertion(&self, parent: ItemId, child: ItemId) -> Result<(), ErreurArbre> {
        let p = self.noeud(parent).ok_or(ErreurArbre::Inconnu(parent))?;
        let c = self.noeud(child).ok_or(ErreurArbre::Inconnu(child))?;
        if p.est_feuille() {
            return Err(ErreurArbre::PasUnParent(parent));
        }
        if self.est_racine_permanente(child) {
            return Err(ErreurArbre::Racine(child));
        }
        if self.est_dans(parent, child) {
            return Err(ErreurArbre::Cycle {
                parent,
                enfant: child,
            });
        }
        let accepte = matches!(
            (&p.forme, &c.forme),
            (Forme::Tabs(_), Forme::Leaf(_)) | (Forme::Split(_), Forme::Split(_) | Forme::Tabs(_))
        );
        if accepte {
            Ok(())
        } else {
            Err(ErreurArbre::FormeRefusee {
                parent,
                enfant: child,
            })
        }
    }

    fn enfants_mut(&mut self, id: ItemId) -> Option<&mut Vec<ItemId>> {
        match self.noeud_mut(id).map(|n| &mut n.forme) {
            Some(Forme::Split(s)) => Some(&mut s.children),
            Some(Forme::Tabs(t)) => Some(&mut t.children),
            _ => None,
        }
    }

    /// Le déplacement dans le même parent : retrait du vecteur puis insertion à `index` dans
    /// la liste RACCOURCIE (même convention qu'un retrait suivi d'un ajout).
    fn reordonner(&mut self, parent: ItemId, child: ItemId, index: Option<usize>) {
        let Some(enfants) = self.enfants_mut(parent) else {
            return;
        };
        let Some(de) = enfants.iter().position(|&c| c == child) else {
            return;
        };
        enfants.remove(de);
        let vers = match index {
            Some(i) if i < enfants.len() => i,
            _ => enfants.len(),
        };
        enfants.insert(vers, child);
        if de != vers {
            self.changement();
        }
        if self.est_pile(parent) {
            self.select_tab(parent, vers);
        }
    }

    fn apres_ajout(&mut self, parent: ItemId, index: usize) {
        if self.est_pile(parent) {
            self.select_tab(parent, index);
        } else {
            self.reflow_geometry(parent);
        }
    }

    /// Retire `child` de `parent` (sans effet s'il n'en est pas un enfant). Le nœud retiré
    /// reste dans l'arène, détaché : il peut être réinséré, ou libéré par `liberer`.
    ///
    /// - Pile : l'onglet VISIBLE le reste s'il n'est pas celui qu'on retire ; une pile vidée
    ///   est mise en file (`prendre_piles_videes`), le Workspace décide de son sort.
    /// - Split intérieur vidé : il se retire de son propre parent (cascade vers le haut) et
    ///   est libéré. Les racines permanentes restent, même vides.
    pub fn remove_child(&mut self, parent: ItemId, child: ItemId) -> Result<(), ErreurArbre> {
        if !self.existe(parent) {
            return Err(ErreurArbre::Inconnu(parent));
        }
        let visible = self.active_leaf(parent);
        let Some(enfants) = self.enfants_mut(parent) else {
            return Err(ErreurArbre::PasUnParent(parent));
        };
        let Some(i) = enfants.iter().position(|&c| c == child) else {
            return Ok(());
        };
        enfants.remove(i);
        let restants = enfants.len();
        if let Some(n) = self.noeud_mut(child) {
            n.parent = None;
        }
        self.changement();

        if self.est_pile(parent) {
            self.apres_retrait_de_pile(parent, child, visible);
            return Ok(());
        }

        // Split.
        let grand_parent = self.parent(parent);
        if restants == 0
            && self.protege != Some(parent)
            && let Some(gp) = grand_parent
        {
            self.remove_child(gp, parent)?;
            // Vide et détaché : plus personne ne le réclame.
            if let Some(case) = self.noeuds.get_mut(parent.0 as usize) {
                *case = None;
            }
            return Ok(());
        }
        self.reflow_geometry(parent);
        Ok(())
    }

    /// ★ L'onglet VISIBLE survit à un retrait situé AVANT lui. Garder seulement l'INDEX
    ///   montrait un autre onglet dès qu'on fermait un onglet placé avant (les suivants se
    ///   décalent) — et la feuille active se retrouvait cachée.
    fn apres_retrait_de_pile(&mut self, pile: ItemId, retire: ItemId, visible: Option<ItemId>) {
        let Some(t) = self.pile(pile) else {
            return;
        };
        let garde = visible
            .filter(|&v| v != retire)
            .and_then(|v| t.children.iter().position(|&c| c == v));
        let n = t.children.len();
        if n == 0 {
            if !self.piles_videes.contains(&pile) {
                self.piles_videes.push(pile);
            }
            return;
        }
        let cible = garde.unwrap_or_else(|| t.active_index.min(n - 1));
        self.select_tab(pile, cible);
    }

    /// Libère un nœud DÉTACHÉ et tout son sous-arbre ; rend les charges des feuilles
    /// libérées (dans l'ordre de parcours). Les racines permanentes ne se libèrent pas.
    pub fn liberer(&mut self, id: ItemId) -> Result<Vec<(ItemId, F)>, ErreurArbre> {
        if !self.existe(id) {
            return Err(ErreurArbre::Inconnu(id));
        }
        if self.est_racine_permanente(id) {
            return Err(ErreurArbre::Racine(id));
        }
        if self.parent(id).is_some() {
            return Err(ErreurArbre::Attache(id));
        }
        let mut out = Vec::new();
        self.liberer_rec(id, &mut out);
        Ok(out)
    }

    fn liberer_rec(&mut self, id: ItemId, out: &mut Vec<(ItemId, F)>) {
        let Some(noeud) = self.noeuds.get_mut(id.0 as usize).and_then(Option::take) else {
            return;
        };
        match noeud.forme {
            Forme::Leaf(f) => out.push((id, f)),
            Forme::Split(s) => s
                .children
                .into_iter()
                .for_each(|c| self.liberer_rec(c, out)),
            Forme::Tabs(t) => t
                .children
                .into_iter()
                .for_each(|c| self.liberer_rec(c, out)),
        }
    }

    /// Change la découpe d'un split (`setDirection`).
    pub fn set_direction(&mut self, split: ItemId, direction: SplitDirection) {
        let change = match self.noeud_mut(split) {
            Some(Noeud {
                forme: Forme::Split(s),
                ..
            }) if s.direction != direction => {
                s.direction = direction;
                true
            }
            _ => false,
        };
        if change {
            self.sauvegarde_demandee = true;
        }
    }

    /// LE point unique « les enfants de ce split ont changé » : rétablit l'invariant de
    /// géométrie (somme des flex-grow = nombre d'enfants, proportions gardées). Appelé par
    /// les ajouts, les retraits et la restauration — un seul propriétaire, pour qu'aucune
    /// mutation future ne puisse l'oublier (le défaut d'origine de Fragment : le retrait
    /// refaisait les poignées mais pas la géométrie). Les poignées, elles, sont du rendu.
    pub fn reflow_geometry(&mut self, split: ItemId) {
        let enfants: Vec<ItemId> = match self.split(split) {
            Some(s) => s.children.clone(),
            None => return,
        };
        if enfants.is_empty() {
            return;
        }
        let grows: Vec<f64> = enfants.iter().map(|&c| self.grow(c)).collect();
        for (c, g) in enfants.into_iter().zip(normalize_grows(&grows)) {
            self.poser_grow(c, g);
        }
    }

    // ─── Les docks ───

    /// Replie un dock ; `false` s'il l'était déjà (ou si `id` n'est pas un dock).
    pub fn collapse(&mut self, dock: ItemId) -> bool {
        self.poser_repli(dock, true)
    }

    /// Déplie un dock ; `false` s'il l'était déjà. Le Workspace matérialise alors ses
    /// feuilles visibles (déplier, c'est regarder).
    pub fn expand(&mut self, dock: ItemId) -> bool {
        self.poser_repli(dock, false)
    }

    /// Bascule le repli ; rend le nouvel état replié (`None` si `id` n'est pas un dock).
    pub fn toggle(&mut self, dock: ItemId) -> Option<bool> {
        let replie = self.sidedock(dock)?.collapsed;
        self.poser_repli(dock, !replie);
        Some(!replie)
    }

    fn poser_repli(&mut self, dock: ItemId, replie: bool) -> bool {
        let Some(d) = self.sidedock_mut(dock) else {
            return false;
        };
        if d.collapsed == replie {
            return false;
        }
        d.collapsed = replie;
        self.sauvegarde_demandee = true;
        true
    }

    /// Pose la largeur choisie d'un dock (`None` : la mise en page reprend la main).
    pub fn poser_largeur(&mut self, dock: ItemId, largeur: Option<f64>) {
        if let Some(d) = self.sidedock_mut(dock) {
            d.width = largeur;
        }
    }

    /// « Ouvert » au sens du bouton du panneau droit : déplié ET non vide. Un dock déplié mais
    /// vide a une largeur nulle à l'écran : on le traite comme fermé.
    pub fn dock_ouvert(&self, dock: ItemId) -> bool {
        self.sidedock(dock).is_some_and(|d| !d.collapsed) && !self.children(dock).is_empty()
    }

    // ─── Sérialisation (§8.2) ───

    /// Réduit une racine (ou tout split) à son layout. `feuille` sait sérialiser une feuille
    /// (son id, son état de vue) : l'arbre ne connaît pas les vues.
    ///
    /// ★ La proportion d'un enfant de split est lue dans le MODÈLE (`grow`), jamais dans la
    ///   géométrie à l'écran : un plugin qui PRÉSENTE un pane autrement (le snap des vues le
    ///   montre en tranche de 36 px) ne doit pas faire enregistrer sa taille affichée — sa
    ///   proportion serait perdue au redémarrage.
    pub fn serialiser_split(
        &self,
        split: ItemId,
        feuille: &dyn Fn(ItemId, &F) -> LeafLayout,
    ) -> Option<SplitLayout> {
        let s = self.split(split)?;
        let children = s
            .children
            .iter()
            .filter_map(|&c| {
                let mut n = self.serialiser(c, feuille)?;
                poser_dimension(&mut n, self.grow(c));
                Some(n)
            })
            .collect();
        let (collapsed, width) = match s.genre {
            GenreSplit::Sidedock(d) => (Some(d.collapsed), d.width.filter(|w| w.is_finite())),
            _ => (None, None),
        };
        Some(SplitLayout {
            direction: s.direction,
            children,
            collapsed,
            width,
            dimension: None,
        })
    }

    /// Réduit n'importe quel nœud à son layout (sans `dimension` : c'est le PARENT split qui
    /// la pose, et une pile ne la pose pas sur ses feuilles).
    pub fn serialiser(
        &self,
        id: ItemId,
        feuille: &dyn Fn(ItemId, &F) -> LeafLayout,
    ) -> Option<LayoutNode> {
        match &self.noeud(id)?.forme {
            Forme::Split(_) => self.serialiser_split(id, feuille).map(LayoutNode::Split),
            Forme::Tabs(t) => Some(LayoutNode::Tabs(TabsLayout {
                current_tab: t.active_index as i64,
                children: t
                    .children
                    .iter()
                    .filter_map(|&c| self.serialiser(c, feuille))
                    .collect(),
                dimension: None,
            })),
            Forme::Leaf(f) => Some(LayoutNode::Leaf(LeafLayout {
                dimension: None,
                ..feuille(id, f)
            })),
        }
    }

    // ─── L'espace des boutons de fenêtre ───

    /// Descend vers le coin haut-droit par la STRUCTURE : dernier enfant d'un split en ligne
    /// (vertical), premier d'un split en colonne (horizontal).
    pub fn find_top_right_tabs(&self, id: ItemId) -> Option<ItemId> {
        if self.est_pile(id) {
            return Some(id);
        }
        let s = self.split(id)?;
        let enfant = match s.direction {
            SplitDirection::Vertical => s.children.last(),
            SplitDirection::Horizontal => s.children.first(),
        }?;
        self.find_top_right_tabs(*enfant)
    }

    /// La pile dont le rectangle contient (x, y) : centre d'abord, puis dock gauche, puis dock
    /// droit ; `None` hors de toute pile. Seuls les nœuds de l'arbre comptent (un fantôme de
    /// glisser ou une surcouche n'existent pas pour lui).
    pub fn get_tabs_at_point(
        &self,
        rect: &dyn Fn(ItemId) -> Option<Rect>,
        x: f64,
        y: f64,
    ) -> Option<ItemId> {
        [self.root, self.left, self.right]
            .into_iter()
            .find_map(|r| self.pile_touchee(r, rect, x, y))
    }

    fn pile_touchee(
        &self,
        id: ItemId,
        rect: &dyn Fn(ItemId) -> Option<Rect>,
        x: f64,
        y: f64,
    ) -> Option<ItemId> {
        if !rect(id)?.contient(x, y) {
            return None;
        }
        if self.est_pile(id) {
            return Some(id);
        }
        self.children(id)
            .iter()
            .find_map(|&c| self.pile_touchee(c, rect, x, y))
    }

    /// La pile qui porte l'espace des boutons de fenêtre (`mod-top-right-space`), et sa
    /// réserve.
    ///
    /// ★ PAR LA GÉOMÉTRIE d'abord : la pile SOUS le point juste à gauche des boutons, en haut.
    ///   Par la seule structure (dernier enfant), un pane ÉTROIT au coin (une tranche du snap,
    ///   36 px) recevait la place, et la barre d'onglets de son voisin passait sous les
    ///   boutons. Hors mise en page (restauration, rien de mesuré), ou si aucune pile n'est
    ///   sous le point : la règle de structure — le dock droit s'il est ouvert, sinon le
    ///   centre.
    /// ★ La réserve est le RECOUVREMENT réel de la barre par les boutons : le pane étroit à sa
    ///   droite en couvre déjà une partie (138 − 36 px, pas 138).
    pub fn espace_des_boutons(
        &self,
        mise_en_page: Option<&MiseEnPage<'_>>,
        largeur_boutons: f64,
    ) -> Option<EspaceBoutons> {
        let sous_les_boutons = mise_en_page
            .filter(|m| m.workspace.largeur > 0.0)
            .and_then(|m| {
                let w = m.workspace;
                self.get_tabs_at_point(m.rect, w.droite() - largeur_boutons - 1.0, w.y + 1.0)
            });
        let pile = sous_les_boutons.or_else(|| {
            if self.dock_ouvert(self.right) {
                self.find_top_right_tabs(self.right)
            } else {
                self.find_top_right_tabs(self.root)
            }
        })?;
        let recouvrement = mise_en_page.and_then(|m| {
            let p = (m.rect)(pile)?;
            if m.workspace.largeur == 0.0 || p.largeur == 0.0 {
                return None;
            }
            let bord = m.workspace.droite() - largeur_boutons;
            Some((p.droite() - bord).clamp(0.0, largeur_boutons.max(0.0)))
        });
        Some(EspaceBoutons { pile, recouvrement })
    }

    /// Les identifiants vivants (diagnostic, tests d'invariants).
    pub fn ids_vivants(&self) -> HashSet<ItemId> {
        self.noeuds
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_some())
            .map(|(i, _)| ItemId(i as u32))
            .collect()
    }
}

fn poser_dimension(n: &mut LayoutNode, d: f64) {
    let champ = match n {
        LayoutNode::Split(s) => &mut s.dimension,
        LayoutNode::Tabs(t) => &mut t.dimension,
        LayoutNode::Leaf(l) => &mut l.dimension,
    };
    *champ = Some(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Ajoutés : normalizeGrows n'a pas de test TS ───

    #[test]
    fn normalize_grows_garde_les_proportions_et_ramene_la_somme_au_nombre() {
        let g = normalize_grows(&[0.3, 0.7]);
        assert!((g[0] - 0.6).abs() < 1e-12 && (g[1] - 1.4).abs() < 1e-12);
    }

    #[test]
    fn normalize_grows_un_seul_survivant_remplit() {
        assert_eq!(normalize_grows(&[0.95]), vec![1.0]);
    }

    #[test]
    fn normalize_grows_somme_nulle_tout_a_un() {
        assert_eq!(normalize_grows(&[0.0, 0.0, 0.0]), vec![1.0; 3]);
        assert!(normalize_grows(&[]).is_empty());
    }

    #[test]
    fn rect_bords_inclus() {
        let r = Rect::new(10.0, 10.0, 5.0, 5.0);
        assert!(r.contient(10.0, 15.0));
        assert!(!r.contient(15.1, 12.0));
    }
}
