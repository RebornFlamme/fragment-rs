// Port de TableController et de CellEditor (app/src/ui/views/markdown/table/TableWidget.ts,
// cellEditor.ts) et du rendu de table.css.
//! Le TABLEAU du live preview : une vraie grille dont chaque cellule est un [`EditeurLive`] imbriqué
//! (révélation au focus seulement, sans fond, marges et interligne de cellule).
//!
//! Le flux, dans les deux sens :
//! - cellule → `set_cell` → `serialize_table` → UN remplacement minimal dans l'hôte (frappe : un
//!   caractère, que l'historique de l'hôte regroupe avec ses voisins) ;
//! - hôte (undo, modification externe) → [`Tableau::mettre_a_jour`] : même forme → les cellules qui
//!   diffèrent sont réécrites EN PLACE (le focus et le curseur survivent), sinon la grille est
//!   reconstruite.
//!
//! ★ Aucune cellule n'a d'historique à elle : Ctrl+Z / Ctrl+Y dans une cellule rejouent l'undo de
//!   l'HÔTE (l'action est envoyée au nœud de focus de l'éditeur hôte). Il n'y a qu'un historique,
//!   celui du fichier.
//! ★ Les gestes que seul le tableau sait traiter (Tab, Entrée, flèches aux bords) sont pris en
//!   CAPTURE sur le conteneur de la cellule, avant l'éditeur du fork.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::Keymap;
use fragment_markdown::commandes::menu::{ActionMenu, ContexteMenu};
use fragment_markdown::decorations::Revelation;
use fragment_markdown::syntaxe::modele_rangees::Plage;
use fragment_markdown::table_model::{
    self as t, Align, TableModel, cell_at_offset, col_count, get_cell, parse_table, row_count,
    serialize_table,
};
use fragment_maths::moteur::Moteur;
use fragment_text::editor::EditorChange;
use fragment_text::utf16::{byte_to_utf16, utf16_len, utf16_to_byte};
use fragment_theme::{Aplat, Couleur, Theme};
use gpui_kit::base::input::{
    Enter, IndentInline, MoveDown, MoveLeft, MoveRight, MoveUp, OutdentInline, Paste, Redo, Undo,
};
use gpui_kit::{
    App, AppContext as _, Bounds, Context, Entity, EntityInputHandler as _, EventEmitter,
    FocusHandle, Focusable as _, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, Subscription, TextAlign, TextRun,
    Window, canvas, div, font, point, px, size,
};

use super::controle::{self, Geste, Operation, Sortie, Suite, diff_minimal};
use super::geometrie::{
    Boite, Grille, Reglages, SEUIL_GLISSER, bande_colonne, bande_rangee, barre_insertion,
    destination, frontiere_proche, hauteur_bloc, largeurs_cellule, largeurs_colonnes,
    place_des_colonnes, poignee_colonne, poignee_rangee, soulevee,
};
use super::menus::{Executer, ExecuterEdition, menu_cellule, menu_poignee, menu_tableau};
use crate::editeur::{EditeurLive, OptionsEditeur};
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Les libellés des bandes « + » (et de leur infobulle).
pub const LIBELLE_AJOUT_COLONNE: &str = "Add column to the right";
pub const LIBELLE_AJOUT_LIGNE: &str = "Add row below";

/// Ce que fait une entrée du menu d'édition d'une cellule, sur l'éditeur de cette cellule.
pub type ActionEdition = Rc<dyn Fn(&Entity<EditeurLive>, ActionMenu, &mut Window, &mut App)>;

/// La configuration commune des tableaux d'une note.
#[derive(Clone)]
pub struct OptionsTableau {
    /// `--font-text-size`.
    pub taille_texte: f32,
    pub lecture_seule: bool,
    /// Le clavier de l'app : la portée Échap des menus s'y empile.
    pub keymap: Keymap,
    /// Le moteur de maths partagé (ADR-011) : les cellules rendent leurs formules en ligne.
    pub moteur: Option<Rc<Moteur>>,
}

/// La sélection de l'hôte (octets) que le tableau vient d'y poser en écrivant : le pont curseur ne
/// la renvoie pas dans une cellule.
pub type Synchro = Rc<RefCell<Option<std::ops::Range<usize>>>>;

/// Ce que le tableau signale à son hôte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvenementTableau {
    /// La hauteur du bloc a changé : l'hôte la dicte à la rangée du tableau.
    Hauteur(f32),
}

/// Une colonne (`colonne`) ou une ligne choisie par un clic sur sa poignée, ou soulevée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choix {
    pub colonne: bool,
    pub index: usize,
}

/// Un appui maintenu sur une poignée.
#[derive(Debug, Clone, Copy)]
struct Glisser {
    choix: Choix,
    depart: Point<Pixels>,
    actif: bool,
    cible: usize,
}

/// Une bande « + ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bande {
    Colonne,
    Ligne,
}

/// La géométrie de la dernière image, en coordonnées FENÊTRE (relue par les écouteurs et les tests).
#[derive(Debug, Clone, Default)]
pub struct GeoTableau {
    /// La boîte visible du bloc (`.cm-table-widget`).
    pub bloc: Bounds<Pixels>,
    /// Le coin haut-gauche de la grille (défilement horizontal compris).
    pub origine: Point<Pixels>,
    pub grille: Grille,
    pub bande_colonne: Option<Boite>,
    pub bande_ligne: Option<Boite>,
    pub poignee_colonne: Option<(usize, Boite)>,
    pub poignee_ligne: Option<(usize, Boite)>,
}

impl GeoTableau {
    fn fenetre(&self, (x, y, w, h): Boite) -> Bounds<Pixels> {
        Bounds {
            origin: point(self.origine.x + px(x), self.origine.y + px(y)),
            size: size(px(w), px(h)),
        }
    }

    /// La boîte de la cellule (r, c), bordures comprises.
    #[must_use]
    pub fn cellule(&self, r: usize, c: usize) -> Option<Bounds<Pixels>> {
        self.grille.cellule(r, c).map(|b| self.fenetre(b))
    }

    /// La boîte de la bande « + ».
    #[must_use]
    pub fn bande(&self, b: Bande) -> Option<Bounds<Pixels>> {
        match b {
            Bande::Colonne => self.bande_colonne,
            Bande::Ligne => self.bande_ligne,
        }
        .map(|x| self.fenetre(x))
    }

    /// La poignée visible (colonne ou ligne) et son index.
    #[must_use]
    pub fn poignee(&self, colonne: bool) -> Option<(usize, Bounds<Pixels>)> {
        if colonne {
            self.poignee_colonne
        } else {
            self.poignee_ligne
        }
        .map(|(i, b)| (i, self.fenetre(b)))
    }

    /// La boîte de la grille entière.
    #[must_use]
    pub fn grille_fenetre(&self) -> Bounds<Pixels> {
        self.fenetre((0.0, 0.0, self.grille.largeur(), self.grille.hauteur()))
    }

    /// Les frontières (fenêtre) entre colonnes ou lignes.
    fn frontieres(&self, colonne: bool) -> Vec<f32> {
        if colonne {
            let x0 = f32::from(self.origine.x);
            let mut v: Vec<f32> = self.grille.x.iter().map(|x| x0 + x).collect();
            if let Some(d) = v.last_mut() {
                *d += self.grille.bordure;
            }
            v
        } else {
            let y0 = f32::from(self.origine.y);
            let mut v: Vec<f32> = self.grille.y.iter().map(|y| y0 + y).collect();
            if let Some(d) = v.last_mut() {
                *d += self.grille.bordure;
            }
            v
        }
    }

    /// La cellule sous le point (fenêtre).
    fn cellule_a(&self, p: Point<Pixels>) -> Option<(usize, usize)> {
        self.grille.cellule_a(
            f32::from(p.x - self.origine.x),
            f32::from(p.y - self.origine.y),
        )
    }
}

/// Une cellule : son éditeur et ce qu'on a mesuré de lui.
struct Cellule {
    editeur: Entity<EditeurLive>,
    focus: bool,
    /// Le nombre de rangées d'affichage (retours à la ligne compris) à la dernière image.
    rangees: usize,
}

/// Le tableau (une entité GPUI) posé à la place de la source `| … |` d'une note.
pub struct Tableau {
    hote: Entity<EditeurLive>,
    options: OptionsTableau,
    /// Le modèle affiché (lu par les menus, modifié par les opérations).
    modele: TableModel,
    /// Le source que la grille affiche : comparé à chaque mise à jour (écho de NOS écritures).
    source: String,
    /// La plage du tableau dans l'hôte, en lignes entières : tenue à jour par l'hôte.
    plage: Plage,
    premiere_ligne: usize,
    cellules: Vec<Vec<Cellule>>,
    choix: Option<Choix>,
    glisser: Option<Glisser>,
    survol: Option<(usize, usize)>,
    bande: Option<Bande>,
    /// Le tableau ENTIER est sélectionné dans l'hôte (1er Retour arrière) : contour d'accent.
    selectionne: bool,
    /// La largeur du bloc (la colonne de lecture), posée par l'hôte.
    largeur_bloc: f32,
    hauteur: f32,
    defilement: ScrollHandle,
    geo: Rc<RefCell<GeoTableau>>,
    operations: Rc<RefCell<Vec<Operation>>>,
    editions: Rc<RefCell<Vec<(usize, usize, ActionMenu)>>>,
    /// Ce que fait une entrée du menu d'édition d'une cellule (posé par la vue Markdown).
    edition: Option<ActionEdition>,
    /// Levé pendant une écriture du tableau dans l'hôte : le pont curseur l'ignore.
    ecriture: Synchro,
    /// Le focus du tableau lui-même : pris par un clic sur une poignée quand rien ne l'a (Suppr,
    /// Échap arrivent alors à l'hôte).
    focus: FocusHandle,
    _abonnements: Vec<Subscription>,
}

impl EventEmitter<EvenementTableau> for Tableau {}

impl Tableau {
    /// Le tableau de `source` (lignes entières), posé sur `plage` de l'éditeur `hote`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hote: Entity<EditeurLive>,
        source: &str,
        plage: Plage,
        premiere_ligne: usize,
        options: OptionsTableau,
        ecriture: Synchro,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut t = Tableau {
            hote,
            options,
            modele: parse_table(source),
            source: source.to_owned(),
            plage,
            premiere_ligne,
            cellules: Vec::new(),
            choix: None,
            glisser: None,
            survol: None,
            bande: None,
            selectionne: false,
            largeur_bloc: 0.0,
            hauteur: 0.0,
            defilement: ScrollHandle::new(),
            geo: Rc::new(RefCell::new(GeoTableau::default())),
            operations: Rc::new(RefCell::new(Vec::new())),
            editions: Rc::new(RefCell::new(Vec::new())),
            edition: None,
            ecriture,
            focus: cx.focus_handle(),
            _abonnements: Vec::new(),
        };
        t.construire(window, cx);
        t
    }

    // ── Lecture ──────────────────────────────────────────────────────────

    #[must_use]
    pub fn modele(&self) -> &TableModel {
        &self.modele
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn plage(&self) -> Plage {
        self.plage
    }

    #[must_use]
    pub fn premiere_ligne(&self) -> usize {
        self.premiere_ligne
    }

    /// L'éditeur de la cellule (r, c).
    #[must_use]
    pub fn cellule(&self, r: usize, c: usize) -> Option<Entity<EditeurLive>> {
        self.cellules.get(r)?.get(c).map(|x| x.editeur.clone())
    }

    /// Le nombre d'éditeurs de cellule.
    #[must_use]
    pub fn nombre_de_cellules(&self) -> usize {
        self.cellules.iter().map(Vec::len).sum()
    }

    /// La cellule qui a le focus.
    #[must_use]
    pub fn cellule_focalisee(&self, cx: &App) -> Option<(usize, usize)> {
        self.cellules.iter().enumerate().find_map(|(r, l)| {
            l.iter()
                .position(|c| c.editeur.read(cx).a_le_focus())
                .map(|c| (r, c))
        })
    }

    #[must_use]
    pub fn choix(&self) -> Option<Choix> {
        self.choix
    }

    #[must_use]
    pub fn est_selectionne(&self) -> bool {
        self.selectionne
    }

    /// La bande « + » survolée (son infobulle est visible).
    #[must_use]
    pub fn bande_survolee(&self) -> Option<Bande> {
        self.bande
    }

    #[must_use]
    pub fn geometrie(&self) -> GeoTableau {
        self.geo.borrow().clone()
    }

    #[must_use]
    pub fn defilement(&self) -> &ScrollHandle {
        &self.defilement
    }

    /// La hauteur du bloc à la dernière image (0 avant la première).
    #[must_use]
    pub fn hauteur(&self) -> f32 {
        self.hauteur
    }

    // ── Réglages venus de l'hôte ─────────────────────────────────────────

    /// La largeur offerte au bloc (la colonne de lecture).
    pub fn poser_largeur(&mut self, l: f32, cx: &mut Context<Self>) {
        if (self.largeur_bloc - l).abs() > f32::EPSILON {
            self.largeur_bloc = l;
            cx.notify();
        }
    }

    /// Le tableau entier est-il sélectionné dans l'hôte ?
    pub fn poser_selectionne(&mut self, s: bool, cx: &mut Context<Self>) {
        if self.selectionne != s {
            self.selectionne = s;
            cx.notify();
        }
    }

    /// Ce que fait une entrée du menu d'édition d'une cellule.
    pub fn poser_edition(&mut self, f: Option<ActionEdition>) {
        self.edition = f;
    }

    // ── Construction ─────────────────────────────────────────────────────

    fn reglages(&self, theme: &Theme) -> Reglages {
        Reglages::du_theme(theme, self.options.taille_texte)
    }

    /// (Re)construit la grille et ses éditeurs depuis le modèle.
    fn construire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = theme_ou_defaut(cx);
        let r = self.reglages(&theme);
        self.cellules.clear();
        self._abonnements.clear();
        for ligne in 0..row_count(&self.modele) {
            let mut l = Vec::new();
            for col in 0..col_count(&self.modele) {
                let options = OptionsEditeur {
                    // ★ Une cellule sans focus garde son rendu : ce qui la commence (`$x$`,
                    //   `**gras**`) ne reste pas en source.
                    revelation: Revelation::FocusSeulement,
                    lecture_seule: self.options.lecture_seule,
                    taille_texte: self.options.taille_texte,
                    fond_transparent: true,
                    marges: Some([
                        r.marge_cellule_v,
                        r.marge_cellule_h,
                        r.marge_cellule_v,
                        r.marge_cellule_h,
                    ]),
                    interligne: Some(r.interligne),
                    ..OptionsEditeur::default()
                };
                let texte = get_cell(&self.modele, ligne, col);
                let editeur = cx.new(|cx| EditeurLive::new(&texte, options, window, cx));
                if let Some(m) = &self.options.moteur {
                    crate::md_inline::calque::installer(&editeur, m.clone(), None, cx);
                }
                let (rr, cc) = (ligne, col);
                self._abonnements.push(cx.subscribe_in(
                    &editeur,
                    window,
                    move |this, _, ev: &EditorChange, window, cx| {
                        if ev.doc_changed {
                            this.sur_frappe(rr, cc, window, cx);
                        }
                    },
                ));
                self._abonnements
                    .push(cx.observe(&editeur, move |this, ed, cx| {
                        this.sur_focus(rr, cc, &ed, cx);
                    }));
                l.push(Cellule {
                    editeur,
                    focus: false,
                    rangees: 1,
                });
            }
            self.cellules.push(l);
        }
        self.aligner(cx);
    }

    /// Pose l'alignement de chaque colonne sur ses cellules (`text-align` du `<td>`).
    fn aligner(&self, cx: &mut Context<Self>) {
        for l in &self.cellules {
            for (c, cel) in l.iter().enumerate() {
                let a = match self.modele.aligns.get(c).copied().flatten() {
                    Some(Align::Center) => TextAlign::Center,
                    Some(Align::Right) => TextAlign::Right,
                    _ => TextAlign::Left,
                };
                let etat = cel.editeur.read(cx).etat().clone();
                etat.update(cx, |s, cx| s.set_text_align(a, cx));
            }
        }
    }

    fn sur_focus(&mut self, r: usize, c: usize, ed: &Entity<EditeurLive>, cx: &mut Context<Self>) {
        let focus = ed.read(cx).a_le_focus();
        let Some(cel) = self.cellules.get_mut(r).and_then(|l| l.get_mut(c)) else {
            return;
        };
        if cel.focus != focus {
            cel.focus = focus;
            if focus {
                // `onFocus` : focaliser une cellule défait le choix d'une poignée.
                self.effacer_choix(cx);
            }
            cx.notify();
        }
    }

    // ── Hôte ↔ widget ────────────────────────────────────────────────────

    /// L'hôte a changé ce tableau (écho, undo, modification externe) : la plage suit toujours ; le
    /// contenu est mis à jour EN PLACE si la forme est la même, sinon la grille est reconstruite.
    pub fn mettre_a_jour(
        &mut self,
        source: &str,
        plage: Plage,
        premiere_ligne: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.plage = plage;
        self.premiere_ligne = premiere_ligne;
        if source == self.source {
            return;
        }
        let suivant = parse_table(source);
        self.source = source.to_owned();
        if !t::same_shape(&self.modele, &suivant) {
            self.modele = suivant;
            self.construire(window, cx);
            cx.notify();
            return;
        }
        self.modele = suivant;
        for (r, l) in self.cellules.iter().enumerate() {
            for (c, cel) in l.iter().enumerate() {
                let texte = get_cell(&self.modele, r, c);
                ecrire_sans_echo(&cel.editeur, &texte, window, cx);
            }
        }
        self.aligner(cx);
        cx.notify();
    }

    /// Une frappe dans la cellule (r, c) : le modèle suit, puis la note. L'écho d'une réécriture
    /// (le texte de la cellule est déjà celui du modèle) est ignoré.
    fn sur_frappe(&mut self, r: usize, c: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ed) = self.cellule(r, c) else { return };
        let texte = ed.read(cx).texte().to_owned();
        if texte == get_cell(&self.modele, r, c) {
            return;
        }
        // Une cellule GFM est une ligne : un saut de ligne (collage) devient une espace.
        let texte = texte.replace(['\r', '\n'], " ");
        self.modele = t::set_cell(&self.modele, r, c, &texte);
        // ★ La frappe n'écrit QUE sa cellule (les colonnes du source ne sont pas réalignées) :
        //   l'historique du fork ne regroupe que des insertions pures, une re-sérialisation
        //   (remplacement d'espaces de bourrage) ferait un undo par caractère.
        let suivant = controle::ecrire_cellule(&self.source, r, c, &texte)
            .filter(|s| t::parse_table(s) == self.modele);
        self.ecrire(suivant, window, cx);
    }

    /// Écrit le modèle dans la note (`suivant`, sinon le modèle re-sérialisé) : UN remplacement
    /// minimal.
    fn ecrire(&mut self, suivant: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let suivant = suivant.unwrap_or_else(|| serialize_table(&self.modele));
        let hote = self.hote.read(cx);
        let texte = hote.texte();
        let Some(ancien) = texte.get(self.plage.octets.from..self.plage.octets.to) else {
            return;
        };
        let Some((a, b, insere)) = diff_minimal(ancien, &suivant) else {
            self.source = suivant;
            return;
        };
        let de = self.plage.utf16.from + byte_to_utf16(ancien, a);
        let a16 = self.plage.utf16.from + byte_to_utf16(ancien, b);
        let etat = hote.etat().clone();
        self.source = suivant;
        let ecriture = self.ecriture.clone();
        etat.update(cx, |s, cx| {
            s.replace_text_in_range(Some(de..a16), &insere, window, cx);
            *ecriture.borrow_mut() = Some(s.selected_range());
        });
    }

    /// Applique une opération de forme : reconstruit la grille, écrit, refocalise.
    pub fn executer(&mut self, op: Operation, window: &mut Window, cx: &mut Context<Self>) {
        if self.options.lecture_seule {
            return;
        }
        let (modele, focus) = controle::appliquer(&self.modele, op);
        self.restructurer(modele, focus, window, cx);
    }

    fn restructurer(
        &mut self,
        modele: TableModel,
        focus: Option<(usize, usize)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.effacer_choix(cx);
        self.modele = modele;
        self.construire(window, cx);
        self.ecrire(None, window, cx);
        if let Some((r, c)) = focus {
            self.focaliser(r, c, None, None, window, cx);
        }
        cx.notify();
    }

    /// Les opérations demandées par les menus (qui n'ont pas de contexte), puis les entrées
    /// d'édition.
    pub fn vider_operations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ops: Vec<_> = self.operations.borrow_mut().drain(..).collect();
        for op in ops {
            self.executer(op, window, cx);
        }
        let editions: Vec<_> = self.editions.borrow_mut().drain(..).collect();
        for (r, c, action) in editions {
            if let (Some(f), Some(ed)) = (self.edition.clone(), self.cellule(r, c)) {
                f(&ed, action, window, cx);
            }
        }
    }

    fn executeur(&self) -> Executer {
        let ops = self.operations.clone();
        Rc::new(move |op| ops.borrow_mut().push(op))
    }

    // ── Focus et navigation ──────────────────────────────────────────────

    /// Focalise la cellule (r, c), curseur à `ancre`/`tete` (UTF-16 ; défaut : fin du texte).
    pub fn focaliser(
        &mut self,
        r: usize,
        c: usize,
        ancre: Option<usize>,
        tete: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.cellule(r, c) else { return };
        let texte = ed.read(cx).texte().to_owned();
        let long = utf16_len(&texte);
        let a = ancre.unwrap_or(long).min(long);
        let h = tete.unwrap_or(a).min(long);
        let oa = utf16_to_byte(&texte, a).unwrap_or(texte.len());
        let oh = utf16_to_byte(&texte, h).unwrap_or(texte.len());
        let etat = ed.read(cx).etat().clone();
        etat.update(cx, |s, cx| {
            s.focus(window, cx);
            s.set_selected_range(oa.min(oh)..oa.max(oh), cx);
        });
    }

    /// Entrée par le bas (↑ depuis la ligne d'après) : la première cellule de la dernière ligne.
    pub fn focaliser_derniere_ligne(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = row_count(&self.modele).saturating_sub(1);
        self.focaliser(r, 0, Some(0), None, window, cx);
    }

    /// Focalise la cellule sous des offsets du SOURCE (le pont curseur de l'hôte).
    pub fn focaliser_a_l_offset(
        &mut self,
        ancre: usize,
        tete: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let a = cell_at_offset(&self.source, ancre);
        let h = cell_at_offset(&self.source, tete);
        let meme = a.row == h.row && a.col == h.col;
        let r = a.row.min(row_count(&self.modele).saturating_sub(1));
        let c = a.col.min(col_count(&self.modele).saturating_sub(1));
        let tete = if meme { h.in_cell } else { a.in_cell };
        self.focaliser(r, c, Some(a.in_cell), Some(tete), window, cx);
    }

    /// Un geste confié par la cellule (r, c) : rend `true` s'il est consommé.
    fn geste(
        &mut self,
        r: usize,
        c: usize,
        g: Geste,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match controle::naviguer(&self.modele, r, c, g) {
            Suite::Laisser => false,
            Suite::Focaliser { r, c, curseur } => {
                self.focaliser(r, c, curseur, None, window, cx);
                true
            }
            Suite::AjouterLigne { r, c } => {
                if self.options.lecture_seule {
                    return true;
                }
                let modele = t::add_row(&self.modele, None);
                self.restructurer(modele, Some((r, c)), window, cx);
                true
            }
            Suite::Sortir(s) => {
                self.sortir(s, window, cx);
                true
            }
        }
    }

    /// Sort du tableau vers l'hôte, juste avant ou juste après ; s'il n'y a rien de ce côté (tableau
    /// en tête ou en fin de note), la ligne qui manque est créée.
    pub fn sortir(&mut self, s: Sortie, window: &mut Window, cx: &mut Context<Self>) {
        let hote = self.hote.read(cx);
        let texte = hote.texte().to_owned();
        let etat = hote.etat().clone();
        let long = utf16_len(&texte);
        let plage = self.plage;
        let ecriture = self.ecriture.clone();
        etat.update(cx, |st, cx| {
            st.focus(window, cx);
            match s {
                Sortie::Apres if plage.utf16.to >= long => {
                    st.replace_text_in_range(Some(long..long), "\n", window, cx);
                    let n = st.text().len();
                    st.set_selected_range(n..n, cx);
                }
                Sortie::Apres => {
                    let o = (plage.octets.to + 1).min(texte.len());
                    st.set_selected_range(o..o, cx);
                }
                Sortie::Avant if plage.utf16.from == 0 => {
                    st.replace_text_in_range(Some(0..0), "\n", window, cx);
                    st.set_selected_range(0..0, cx);
                }
                Sortie::Avant => {
                    let o = plage.octets.from.saturating_sub(1);
                    st.set_selected_range(o..o, cx);
                }
            }
            *ecriture.borrow_mut() = Some(st.selected_range());
        });
    }

    /// Le geste ↑ / ↓ : le curseur de la cellule est-il sur sa première (dernière) ligne visuelle ?
    fn au_bord(&self, r: usize, c: usize, fin: bool, cx: &App) -> bool {
        let Some(ed) = self.cellule(r, c) else {
            return true;
        };
        let e = ed.read(cx).etat().read(cx);
        let ici = e.cursor();
        let bord = if fin { e.text().len() } else { 0 };
        match (
            e.range_to_bounds(&(ici..ici)),
            e.range_to_bounds(&(bord..bord)),
        ) {
            (Some(a), Some(b)) => (f32::from(a.origin.y) - f32::from(b.origin.y)).abs() < 2.0,
            _ => true,
        }
    }

    /// Rejoue l'historique de l'HÔTE (le seul).
    fn historique(&self, annuler: bool, window: &mut Window, cx: &mut Context<Self>) {
        let fh = self.hote.read(cx).etat().read(cx).focus_handle(cx);
        window.defer(cx, move |window, cx| {
            if annuler {
                fh.dispatch_action(&Undo, window, cx);
            } else {
                fh.dispatch_action(&Redo, window, cx);
            }
        });
    }

    // ── Choix d'une colonne / ligne ──────────────────────────────────────

    /// Choisit (clic sur sa poignée) la colonne ou la ligne `index`.
    pub fn choisir(&mut self, choix: Choix, cx: &mut Context<Self>) {
        self.choix = Some(choix);
        cx.notify();
    }

    pub fn effacer_choix(&mut self, cx: &mut Context<Self>) {
        if self.choix.take().is_some() || self.glisser.take().is_some() {
            cx.notify();
        }
    }

    /// Suppr / Retour arrière sur un choix : supprime la colonne ou la ligne (ni l'en-tête ni la
    /// dernière colonne). Rend `true` s'il y avait un choix (la touche est alors consommée).
    pub fn supprimer_choix(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ch) = self.choix else { return false };
        if self.options.lecture_seule {
            return true;
        }
        if let Some(op) = controle::suppression(&self.modele, ch.colonne, ch.index) {
            self.executer(op, window, cx);
        }
        true
    }

    // ── Souris ───────────────────────────────────────────────────────────

    fn survoler(&mut self, p: Point<Pixels>, cx: &mut Context<Self>) {
        let geo = self.geo.borrow().clone();
        let bande = [Bande::Colonne, Bande::Ligne]
            .into_iter()
            .find(|&b| geo.bande(b).is_some_and(|x| x.contains(&p)));
        let mut change = bande != self.bande;
        self.bande = bande;
        if !self.options.lecture_seule && self.glisser.is_none() && self.choix.is_none() {
            let sur_poignee = [true, false]
                .into_iter()
                .any(|c| geo.poignee(c).is_some_and(|(_, b)| b.contains(&p)));
            if let Some(cel) = geo.cellule_a(p)
                && geo.bloc.contains(&p)
            {
                change |= self.survol != Some(cel);
                self.survol = Some(cel);
            } else if !sur_poignee && !geo.bloc.contains(&p) && self.survol.is_some() {
                // `mouseleave` : les poignées disparaissent.
                self.survol = None;
                change = true;
            }
        }
        if change {
            cx.notify();
        }
    }

    /// Un appui gauche : poignée (choix, début d'un glisser), bande « + », ou ailleurs (le choix
    /// tombe). Rend `true` s'il est consommé.
    fn appui(&mut self, p: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let geo = self.geo.borrow().clone();
        if self.options.lecture_seule {
            return false;
        }
        for colonne in [true, false] {
            if let Some((index, b)) = geo.poignee(colonne)
                && b.contains(&p)
            {
                let choix = Choix { colonne, index };
                if window.focused(cx).is_none() {
                    window.focus(&self.focus, cx);
                }
                self.choix = Some(choix);
                self.glisser = Some(Glisser {
                    choix,
                    depart: p,
                    actif: false,
                    cible: index,
                });
                cx.notify();
                return true;
            }
        }
        for b in [Bande::Colonne, Bande::Ligne] {
            if geo.bande(b).is_some_and(|x| x.contains(&p)) {
                let op = match b {
                    Bande::Colonne => Operation::AjouterColonne(col_count(&self.modele)),
                    Bande::Ligne => Operation::AjouterLigne(row_count(&self.modele)),
                };
                self.executer(op, window, cx);
                return true;
            }
        }
        self.effacer_choix(cx);
        false
    }

    fn deplacer(&mut self, p: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(mut g) = self.glisser else { return };
        let d = f32::from((p.x - g.depart.x).abs() + (p.y - g.depart.y).abs());
        if !g.actif && d > SEUIL_GLISSER {
            g.actif = true;
        }
        if g.actif {
            let geo = self.geo.borrow();
            let f = geo.frontieres(g.choix.colonne);
            let v = if g.choix.colonne { p.x } else { p.y };
            g.cible = frontiere_proche(&f, f32::from(v));
        }
        self.glisser = Some(g);
        cx.notify();
    }

    fn relacher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(g) = self.glisser.take() else { return };
        if !g.actif {
            // Simple clic : le choix reste (jusqu'au prochain appui ailleurs, Échap, Suppr).
            cx.notify();
            return;
        }
        match destination(g.choix.index, g.cible) {
            None => self.effacer_choix(cx),
            Some(vers) => {
                let op = if g.choix.colonne {
                    Operation::DeplacerColonne {
                        de: g.choix.index,
                        vers,
                    }
                } else {
                    Operation::DeplacerLigne {
                        de: g.choix.index,
                        vers,
                    }
                };
                self.executer(op, window, cx);
            }
        }
    }

    /// Un clic droit à `p` : le menu de la poignée, celui d'édition d'une cellule (dans son texte),
    /// ou celui du tableau (bordure, marges, bandes). Rend `true` s'il est consommé.
    fn menu(&mut self, p: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let geo = self.geo.borrow().clone();
        if !geo.bloc.contains(&p) {
            return false;
        }
        if self.options.lecture_seule {
            return true;
        }
        let ex = self.executeur();
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        for colonne in [true, false] {
            if let Some((index, b)) = geo.poignee(colonne)
                && b.contains(&p)
            {
                menu_poignee(&self.modele, &ex, colonne, index).show_at_position(
                    x,
                    y,
                    &self.options.keymap,
                );
                cx.notify();
                return true;
            }
        }
        let cel = geo.cellule_a(p);
        let dans_le_texte = cel.and_then(|(r, c)| {
            let b = geo.cellule(r, c)?;
            let e = px(geo.grille.bordure);
            let dedans = Bounds {
                origin: point(b.origin.x + e, b.origin.y + e),
                size: size(b.size.width - e * 2.0, b.size.height - e * 2.0),
            };
            dedans.contains(&p).then_some((r, c))
        });
        let menu = if let Some((r, c)) = dans_le_texte {
            let editions = self.editions.clone();
            let ed: ExecuterEdition = Rc::new(move |a| editions.borrow_mut().push((r, c, a)));
            let vide = self.cellule(r, c).is_none_or(|e| {
                let s = e.read(cx).etat().read(cx).selected_range();
                s.start == s.end
            });
            let ctx = ContexteMenu {
                selection_vide: vide,
                lecture_seule: self.options.lecture_seule,
                imbrique: true,
            };
            menu_cellule(&self.modele, &ex, &ed, ctx, r, c)
        } else {
            let (r, c) = cel.unwrap_or((0, 0));
            menu_tableau(&self.modele, &ex, r, c)
        };
        menu.show_at_position(x, y, &self.options.keymap);
        cx.notify();
        true
    }

    // ── Mesure ───────────────────────────────────────────────────────────

    /// Les largeurs de colonnes : min- et max-content de chaque cellule (source affichée, en-tête en
    /// demi-gras), partagées comme une `<table>` en `table-layout: auto`.
    fn largeurs(&self, r: &Reglages, theme: &Theme, window: &Window) -> Vec<f32> {
        let ts = window.text_system().clone();
        let taille = self.options.taille_texte;
        let mesurer = |s: &str, gras: bool| -> f32 {
            if s.is_empty() {
                return 0.0;
            }
            let mut f = font(FAMILLE_INTERFACE);
            if gras {
                #[allow(clippy::cast_precision_loss)]
                let g = theme.typo.font_semibold as f32;
                f.weight = FontWeight(g);
            }
            let run = TextRun {
                len: s.len(),
                font: f,
                color: couleur(theme.palette.text_normal).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                ts.shape_line(s.to_owned().into(), px(taille), &[run], None)
                    .width,
            )
        };
        let n = col_count(&self.modele);
        let mut min = vec![r.largeur_min(); n];
        let mut max = vec![r.largeur_min(); n];
        for ligne in 0..row_count(&self.modele) {
            for c in 0..n {
                let texte = get_cell(&self.modele, ligne, c);
                let gras = ligne == 0;
                let mots = texte
                    .split_whitespace()
                    .map(|m| mesurer(m, gras))
                    .fold(0.0, f32::max);
                let (a, b) = largeurs_cellule(r, mots, mesurer(&texte, gras));
                min[c] = min[c].max(a);
                max[c] = max[c].max(b);
            }
        }
        largeurs_colonnes(&min, &max, place_des_colonnes(r, self.largeur_bloc, n))
    }

    /// Les hauteurs de rangées : la cellule la plus haute (rangées d'affichage du fork, relues à
    /// l'image précédente).
    fn hauteurs(&mut self, r: &Reglages, cx: &App) -> Vec<f32> {
        let ligne = r.ligne_texte();
        self.cellules
            .iter_mut()
            .map(|l| {
                let mut h: f32 = 0.0;
                for cel in l.iter_mut() {
                    let n = cel
                        .editeur
                        .read(cx)
                        .etat()
                        .read(cx)
                        .display_map()
                        .display_row_count()
                        .max(1);
                    cel.rangees = n;
                    #[allow(clippy::cast_precision_loss)]
                    let contenu = n as f32 * ligne;
                    h = h.max(r.hauteur_rangee(contenu));
                }
                h
            })
            .collect()
    }

    /// La hauteur du bloc pour un tableau de `source` jamais mesuré : une ligne de texte par rangée.
    #[must_use]
    pub fn hauteur_estimee(source: &str, r: &Reglages) -> f32 {
        let m = parse_table(source);
        let rangees = row_count(&m);
        let hauteurs = vec![r.hauteur_rangee(r.ligne_texte()); rangees];
        let g = Grille::new(&[0.0], &hauteurs, r.bordure);
        let [h, _, b, _] = r.marges_bloc;
        h + g.hauteur() + b
    }
}

/// Réécrit le texte d'une cellule SANS écho (le modèle a déjà ce texte) ; le curseur garde sa place,
/// bornée à la nouvelle longueur.
fn ecrire_sans_echo(ed: &Entity<EditeurLive>, texte: &str, window: &mut Window, cx: &mut App) {
    if ed.read(cx).texte() == texte {
        return;
    }
    let etat = ed.read(cx).etat().clone();
    etat.update(cx, |s, cx| {
        let tete = s.cursor().min(texte.len());
        let mut tete = tete;
        while !texte.is_char_boundary(tete) {
            tete -= 1;
        }
        s.replace_all(texte.to_owned(), window, cx);
        s.set_selected_range(tete..tete, cx);
    });
}

fn boite_div(b: Boite) -> gpui_kit::Div {
    let (x, y, w, h) = b;
    div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h))
}

fn accent_translucide(p: &fragment_theme::Palette) -> Couleur {
    // `color-mix(in srgb, var(--table-accent) 12%, transparent)`.
    p.color_accent.melange(12.0, Couleur::octets(0, 0, 0, 0))
}

impl Render for Tableau {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.operations.borrow().is_empty() || !self.editions.borrow().is_empty() {
            cx.defer_in(window, |t, window, cx| t.vider_operations(window, cx));
        }
        let theme = theme_ou_defaut(cx);
        let p = &theme.palette;
        let r = self.reglages(&theme);
        let largeurs = self.largeurs(&r, &theme, window);
        let hauteurs = self.hauteurs(&r, cx);
        let grille = Grille::new(&largeurs, &hauteurs, r.bordure);
        let hauteur = hauteur_bloc(&grille, &r, self.largeur_bloc);
        if (hauteur - self.hauteur).abs() > 0.01 {
            self.hauteur = hauteur;
            cx.emit(EvenementTableau::Hauteur(hauteur));
        }
        let [mh, md, _, mg] = r.marges_bloc;
        let contenu_l = mg + grille.largeur() + md;
        let contenu_h = hauteur;
        let bordure = couleur(p.aplat(Aplat::Bordure, p.background_primary));
        let accent = couleur(p.color_accent);
        let lecture_seule = self.options.lecture_seule;

        // Les poignées : celles de la cellule survolée, ou celle du choix (soulevée).
        let (pc, pl) = match (self.choix, self.survol) {
            (Some(ch), _) if ch.colonne => (Some(ch.index), None),
            (Some(ch), _) => (None, Some(ch.index)),
            (None, Some((rr, cc))) => (Some(cc), Some(rr)),
            (None, None) => (None, None),
        };
        let (pc, pl) = if lecture_seule {
            (None, None)
        } else {
            (pc, pl)
        };
        let active = |colonne: bool| self.choix.is_some_and(|c| c.colonne == colonne);
        let poignee_c = pc.and_then(|c| poignee_colonne(&grille, c, active(true)).map(|b| (c, b)));
        let poignee_l = pl.and_then(|rr| poignee_rangee(&grille, rr).map(|b| (rr, b)));
        let bande_c = (!lecture_seule).then(|| bande_colonne(&grille, &r));
        let bande_l = (!lecture_seule).then(|| bande_rangee(&grille, &r));
        {
            let mut g = self.geo.borrow_mut();
            g.grille = grille.clone();
            g.bande_colonne = bande_c;
            g.bande_ligne = bande_l;
            g.poignee_colonne = poignee_c;
            g.poignee_ligne = poignee_l;
        }

        let decaler = |(x, y, w, h): Boite| (x + mg, y + mh, w, h);
        let mut interieur = div().relative().w(px(contenu_l)).h(px(contenu_h));

        // Le tableau entier sélectionné : contour d'accent décalé d'1 px, fond translucide.
        if self.selectionne {
            let e = 2.0 * r.bordure;
            let b = decaler((
                -1.0 - e,
                -1.0 - e,
                grille.largeur() + 2.0 * (1.0 + e),
                grille.hauteur() + 2.0 * (1.0 + e),
            ));
            interieur = interieur.child(
                boite_div(b)
                    .border(px(e))
                    .border_color(accent)
                    .bg(couleur(accent_translucide(p))),
            );
        }

        // Les cellules : une boîte bordée chacune (les bordures voisines se superposent, comme
        // `border-collapse`), l'éditeur imbriqué dedans.
        for (rr, l) in self.cellules.iter().enumerate() {
            for (cc, cel) in l.iter().enumerate() {
                let Some(b) = grille.cellule(rr, cc) else {
                    continue;
                };
                let mut d = boite_div(decaler(b))
                    .border(px(r.bordure))
                    .border_color(bordure)
                    .overflow_hidden();
                if rr == 0 {
                    #[allow(clippy::cast_precision_loss)]
                    let g = theme.typo.font_semibold as f32;
                    d = d.font_weight(FontWeight(g));
                }
                let d = d
                    .capture_action(cx.listener(move |t, _: &IndentInline, w, cx| {
                        if t.geste(rr, cc, Geste::Suivante, w, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &OutdentInline, w, cx| {
                        if t.geste(rr, cc, Geste::Precedente, w, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &Enter, w, cx| {
                        if t.geste(rr, cc, Geste::Entree, w, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &MoveUp, w, cx| {
                        let bord = t.au_bord(rr, cc, false, cx);
                        if t.geste(rr, cc, Geste::Haut { bord }, w, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &MoveDown, w, cx| {
                        let bord = t.au_bord(rr, cc, true, cx);
                        if t.geste(rr, cc, Geste::Bas { bord }, w, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &MoveLeft, w, cx| {
                        let Some(ed) = t.cellule(rr, cc) else { return };
                        let s = ed.read(cx).etat().read(cx).selected_range();
                        if s.start == s.end && s.start == 0 && t.geste(rr, cc, Geste::Gauche, w, cx)
                        {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(move |t, _: &MoveRight, w, cx| {
                        let Some(ed) = t.cellule(rr, cc) else { return };
                        let long = ed.read(cx).texte().len();
                        let s = ed.read(cx).etat().read(cx).selected_range();
                        if s.start == s.end
                            && s.end == long
                            && t.geste(rr, cc, Geste::Droite, w, cx)
                        {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|t, _: &Undo, w, cx| {
                        t.historique(true, w, cx);
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(|t, _: &Redo, w, cx| {
                        t.historique(false, w, cx);
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(move |t, _: &Paste, w, cx| {
                        // Un texte multiligne est aplati : une cellule GFM est une ligne.
                        let Some(texte) = cx.read_from_clipboard().and_then(|c| c.text()) else {
                            return;
                        };
                        if !texte.contains('\n') {
                            return;
                        }
                        let Some(ed) = t.cellule(rr, cc) else { return };
                        let plat = texte.replace("\r\n", " ").replace('\n', " ");
                        let etat = ed.read(cx).etat().clone();
                        etat.update(cx, |s, cx| s.replace(plat, w, cx));
                        cx.stop_propagation();
                    }))
                    .child(cel.editeur.clone());
                interieur = interieur.child(d);
            }
        }

        // La colonne / ligne choisie ou soulevée.
        if let Some(ch) = self.choix
            && let Some(b) = soulevee(&grille, ch.colonne, ch.index)
        {
            interieur = interieur.child(
                boite_div(decaler(b))
                    .border(px(2.0 * r.bordure))
                    .border_color(accent)
                    .rounded(px(theme.forme.radius_sm))
                    .bg(couleur(accent_translucide(p))),
            );
        }
        // La barre d'insertion d'un glisser.
        if let Some(g) = self.glisser.filter(|g| g.actif) {
            let f = if g.choix.colonne {
                &grille.x
            } else {
                &grille.y
            };
            if let Some(&a) = f.get(g.cible) {
                let a = if g.cible + 1 == f.len() {
                    a + r.bordure
                } else {
                    a
                };
                interieur = interieur.child(
                    boite_div(decaler(barre_insertion(&grille, g.choix.colonne, a)))
                        .bg(accent)
                        .rounded(px(theme.forme.radius_xxs)),
                );
            }
        }
        // Les poignées.
        for (colonne, poignee, icone) in [
            (true, poignee_c, "grip-horizontal"),
            (false, poignee_l, "grip-vertical"),
        ] {
            let Some((_, b)) = poignee else { continue };
            let est_active = active(colonne);
            let mut d = boite_div(decaler(b))
                .flex()
                .items_center()
                .justify_center()
                .text_color(couleur(p.text_faint));
            if est_active {
                let rayon = px(theme.forme.radius_sm);
                d = d.bg(accent).text_color(couleur(p.text_on_accent));
                d = if colonne {
                    d.rounded_tl(rayon).rounded_tr(rayon)
                } else {
                    d.rounded_tl(rayon).rounded_bl(rayon)
                };
            }
            let c = if est_active {
                p.text_on_accent
            } else {
                p.text_faint
            };
            interieur = interieur.child(
                d.child(
                    crate::icone::icone_image()
                        .path(icones::chemin(icone))
                        .size(px(theme.forme.icon_xs))
                        .text_color(couleur(c)),
                ),
            );
        }
        // Les bandes « + », visibles au survol seulement, et leur infobulle.
        for (bande, boite, libelle) in [
            (Bande::Colonne, bande_c, LIBELLE_AJOUT_COLONNE),
            (Bande::Ligne, bande_l, LIBELLE_AJOUT_LIGNE),
        ] {
            let Some(b) = boite else { continue };
            if self.bande != Some(bande) {
                continue;
            }
            let b = decaler(b);
            interieur = interieur.child(
                boite_div(b)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme.forme.radius_s))
                    .bg(couleur(p.aplat(Aplat::Survol, p.background_primary)))
                    .child(
                        crate::icone::icone_image()
                            .path(icones::chemin("plus"))
                            .size(px(theme.forme.icon_xs))
                            .text_color(couleur(p.text_muted)),
                    ),
            );
            interieur = interieur.child(infobulle(libelle, b, &theme, window));
        }

        // Mesure (bornes de la boîte intérieure, défilement compris) et écouteurs de la souris.
        let geo = self.geo.clone();
        let entite = cx.entity();
        interieur = interieur.child(
            canvas(
                move |bornes, _, _| {
                    let mut g = geo.borrow_mut();
                    g.origine = point(bornes.origin.x + px(mg), bornes.origin.y + px(mh));
                },
                move |_, (), window, _| ecouter_souris(&entite, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let geo = self.geo.clone();
        div()
            .id(("md-tableau", cx.entity_id()))
            .track_focus(&self.focus)
            .size_full()
            .occlude()
            .overflow_x_scroll()
            .track_scroll(&self.defilement)
            .child(interieur)
            .child(
                canvas(
                    move |bornes, _, _| geo.borrow_mut().bloc = bornes,
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

/// L'infobulle sombre sous une bande (`.cm-table-tooltip`), centrée sous elle.
fn infobulle(texte: &str, (x, y, w, h): Boite, theme: &Theme, window: &Window) -> gpui_kit::Div {
    let f = &theme.forme;
    let corps = theme.typo.font_ui_small;
    let ts = window.text_system().clone();
    let run = TextRun {
        len: texte.len(),
        font: font(FAMILLE_INTERFACE),
        color: couleur(theme.palette.text_normal).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let largeur = f32::from(
        ts.shape_line(texte.to_owned().into(), px(corps), &[run], None)
            .width,
    ) + 2.0 * f.size_4_2;
    div()
        .absolute()
        .left(px(x + w / 2.0 - largeur / 2.0))
        .top(px(y + h + 6.0))
        .px(px(f.size_4_2))
        .py(px(f.size_4_1))
        .rounded(px(f.radius_s))
        .text_size(px(corps))
        .whitespace_nowrap()
        .text_color(couleur(theme.palette.text_normal))
        .bg(couleur(theme.palette.background_secondary_alt))
        .child(texte.to_owned())
}

/// Les écouteurs de la souris du tableau (posés à la peinture, rejoués à chaque image).
fn ecouter_souris(t: &Entity<Tableau>, window: &mut Window) {
    let h = t.clone();
    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _window, cx| {
        if !phase.capture() {
            return;
        }
        let p = ev.position;
        h.update(cx, |t, cx| {
            if t.glisser.is_some() {
                t.deplacer(p, cx);
            } else {
                t.survoler(p, cx);
            }
        });
    });
    let h = t.clone();
    window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
        if !phase.capture() {
            return;
        }
        let p = ev.position;
        let pris = h.update(cx, |t, cx| match ev.button {
            MouseButton::Left => t.appui(p, window, cx),
            MouseButton::Right => t.menu(p, cx),
            _ => false,
        });
        if pris {
            cx.stop_propagation();
        }
    });
    let h = t.clone();
    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
        if phase.capture() {
            h.update(cx, |t, cx| t.relacher(window, cx));
        }
    });
}
