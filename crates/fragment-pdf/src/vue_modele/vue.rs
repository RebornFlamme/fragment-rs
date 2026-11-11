// Port de app/src/ui/views/pdf/PdfView.ts (appliquerEchelle, modes, ResizeObserver,
// IntersectionObserver, majPage, repeindre, finDeGeste, zoomerMolette, lirePosition,
// allerAPosition, getEphemeralState) et de app/src/ui/views/pdf/devicePixels.ts
//
// Le modèle de la vue PDF, sans DOM : le même enchaînement d'événements (zone
// redimensionnée, défilement, molette, touches, temps qui passe) donne les mêmes
// pages visibles, la même page courante, la même échelle et le même défilement
// que la vue TypeScript, et la liste des pages à peindre ou à libérer.
//
// Conventions :
// - les longueurs sont en px écran, l'origine du plan est le coin haut-gauche du sizer ;
// - un point (curseur) est donné RELATIVEMENT au coin haut-gauche de la zone qui défile ;
// - le temps est fourni par l'appelant en millisecondes (`avancer`), comme pour
//   `fragment_core::debounce` : aucun thread, aucun minuteur ici ;
// - les pages sont numérotées à partir de 1 dans l'API, de 0 en interne.

use std::cell::Cell;
use std::collections::BTreeSet;

use fragment_core::debounce::Debouncer;
use serde_json::Value;

use crate::pdf_geometry::PdfGeometrie;
use crate::pdf_zoom::{
    ECHELLE_MAX, ECHELLE_MIN, Sens, Taille, ZoomMode, borner_echelle, echelle_pour_mode,
    palier_suivant,
};

use super::champ::{ChampPage, analyser_entier};
use super::etat::{EtatEphemere, PositionLecture, entier_positif, mode_depuis_texte};
use super::pixels::{Pixels, taille_physique};
use super::rendu::{EtatPage, Peinture, PlanRendu};

/// Ctrl + molette et pinch : un cran de molette (deltaY d'environ 100) vaut à peu près x1,6.
/// Valeur de `ln(1,6) / 100`.
pub const SENSIBILITE_MOLETTE: f64 = 0.004_700_036_292_457_356;

/// ★ Aucun REPEINT pendant un geste de zoom : le rendu attend ce silence (ms) après
/// le dernier événement. À 50 ms, une pause du doigt au milieu d'un pinch suffisait
/// à repeindre tout le document. Le DÉFILEMENT, lui, n'est jamais différé : il ne
/// repeint rien, il peint seulement les pages qui entrent (différé, un défilement
/// continu laissait l'écran blanc jusqu'à l'arrêt).
pub const DELAI_FIN_DE_GESTE: u64 = 100;

/// On peint à l'avance une zone d'une hauteur d'écran au-dessus et en dessous
/// (marge de 100 % de la hauteur visible) : on défile sans voir de page blanche
/// sans garder tout le document en mémoire.
pub const MARGE_RENDU: f64 = 1.0;

/// Les modes du menu de zoom, avec leur libellé.
pub const MODES_ZOOM: [(ZoomMode, &str); 4] = [
    (ZoomMode::Auto, "Automatic"),
    (ZoomMode::PageActual, "Actual size"),
    (ZoomMode::PageFit, "Page fit"),
    (ZoomMode::PageWidth, "Page width"),
];

/// Un point dans la zone qui défile, en px depuis son coin haut-gauche.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// La zone qui défile.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Zone {
    /// Largeur utile : sans le padding (la marge de lecture en réserve une part).
    pub largeur_utile: f64,
    /// Somme des paddings gauche et droit.
    pub padding_h: f64,
    /// Hauteur visible.
    pub hauteur: f64,
}

/// Les raccourcis de zoom (touches physiques, `Ctrl` ou `Cmd` enfoncé).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raccourci {
    ZoomPlus,
    ZoomMoins,
    /// Retour au mode automatique.
    ModeAuto,
}

/// Le raccourci d'une touche PHYSIQUE (code) avec `Ctrl`/`Cmd` : `Equal`, `NumpadAdd`
/// ou `+` zooment (avec ou sans Maj : en AZERTY, « + » est Maj + « = ») ; `Minus`,
/// `NumpadSubtract` ou `-` dézooment ; `Digit0` ou `Numpad0` reviennent en auto
/// (en AZERTY Ctrl + 0 produit « à » : seul le code physique est fiable).
pub fn raccourci_pour(code: &str, maj: bool) -> Option<Raccourci> {
    match code {
        "Equal" | "NumpadAdd" | "+" => Some(Raccourci::ZoomPlus),
        "Minus" | "NumpadSubtract" | "-" if !maj => Some(Raccourci::ZoomMoins),
        "Digit0" | "Numpad0" if !maj => Some(Raccourci::ModeAuto),
        _ => None,
    }
}

/// Ce que devient un événement de molette.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultatMolette {
    /// La vue doit appeler `preventDefault` : sinon le navigateur défilerait en plus de zoomer.
    pub empeche_defaut: bool,
    pub rendu: PlanRendu,
}

/// La reprise d'une position de lecture demandée : on réessaie à chaque image
/// tant que la page bouge (un zoom ou la barre latérale restaurés dans la même
/// passe la déplacent encore).
#[derive(Debug, Clone, PartialEq)]
pub struct Reprise {
    position: PositionLecture,
    calme: u32,
}

impl Reprise {
    /// Un essai. Rend vrai quand la position est tenue (deux essais calmes de suite).
    pub fn pas(&mut self, vue: &mut VueModele) -> (bool, PlanRendu) {
        vue.pas_de_reprise(self)
    }
}

pub struct VueModele {
    geometrie: PdfGeometrie,
    /// Taille de la page 1 à l'échelle 1 : la référence des modes de zoom.
    base: Taille,
    echelle: f64,
    /// `None` : échelle libre (zoom +/- ou pourcentage). Un mode suit la taille de la zone.
    mode: Option<ZoomMode>,
    zone: Zone,
    densite: f64,
    scroll_top: f64,
    scroll_left: f64,
    /// La page affichée dans la barre d'outils (0 = aucune).
    page_vue: usize,
    /// Page DEMANDÉE (champ, vignette, table des matières) et le défilement qui l'a
    /// posée, tant qu'on n'a pas défilé depuis : en fin de document la page visée
    /// ne peut pas monter en haut et la « plus visible » serait sa voisine.
    page_forcee: Option<(usize, f64)>,
    champ: ChampPage,
    en_geste: bool,
    fin_de_geste: Debouncer<()>,
    recaler_pixels: Debouncer<()>,
    maintenant: u64,
    /// Plage (incluse) des pages dans la bande peinte, ou `None`.
    visibles: Option<(usize, usize)>,
    etats: Vec<EtatPage>,
    /// Les pages qui ne sont pas `Vide` (pour ne parcourir que celles-là).
    actives: BTreeSet<usize>,
    /// Taille physique mesurée par la vue, par page (écrase le calcul).
    pixels_livres: Vec<Option<Pixels>>,
    /// Taille du canvas actuellement peint, par page.
    canvas: Vec<Option<Pixels>>,
    page_en_attente: Option<usize>,
    lecture_en_attente: Option<PositionLecture>,
    /// La largeur de la page la plus large, en px écran (mémorisée).
    largeur_max: Cell<Option<f64>>,
}

impl Default for VueModele {
    fn default() -> Self {
        Self::new()
    }
}

impl VueModele {
    pub fn new() -> Self {
        Self {
            geometrie: PdfGeometrie::new(),
            base: Taille {
                largeur: 0.0,
                hauteur: 0.0,
            },
            echelle: 1.0,
            mode: Some(ZoomMode::Auto),
            zone: Zone::default(),
            densite: 1.0,
            scroll_top: 0.0,
            scroll_left: 0.0,
            page_vue: 0,
            page_forcee: None,
            champ: ChampPage::default(),
            en_geste: false,
            fin_de_geste: Debouncer::new(|()| {}, DELAI_FIN_DE_GESTE, true),
            recaler_pixels: Debouncer::new(|()| {}, DELAI_FIN_DE_GESTE, true),
            maintenant: 0,
            visibles: None,
            etats: Vec::new(),
            actives: BTreeSet::new(),
            pixels_livres: Vec::new(),
            canvas: Vec::new(),
            page_en_attente: None,
            lecture_en_attente: None,
            largeur_max: Cell::new(None),
        }
    }

    // ── Lecture de l'état ────────────────────────────────────────────────

    pub fn geometrie(&self) -> &PdfGeometrie {
        &self.geometrie
    }

    pub fn echelle(&self) -> f64 {
        self.echelle
    }

    pub fn mode(&self) -> Option<ZoomMode> {
        self.mode
    }

    pub fn zone(&self) -> Zone {
        self.zone
    }

    pub fn nombre_pages(&self) -> usize {
        self.geometrie.nombre()
    }

    pub fn scroll_top(&self) -> f64 {
        self.scroll_top
    }

    pub fn scroll_left(&self) -> f64 {
        self.scroll_left
    }

    /// La page affichée dans la barre d'outils (0 = aucune).
    pub fn page_vue(&self) -> usize {
        self.page_vue
    }

    pub fn champ(&self) -> &ChampPage {
        &self.champ
    }

    /// « of N » (vide tant qu'aucun document n'est ouvert).
    pub fn total_texte(&self) -> String {
        match self.nombre_pages() {
            0 => String::new(),
            n => format!("of {n}"),
        }
    }

    /// Un geste de zoom différé est en cours : rien n'est peint.
    pub fn en_geste(&self) -> bool {
        self.en_geste
    }

    pub fn etat_page(&self, page: usize) -> EtatPage {
        page.checked_sub(1)
            .and_then(|i| self.etats.get(i).copied())
            .unwrap_or_default()
    }

    /// La plage (incluse, pages à partir de 1) de la bande visible, marge comprise.
    pub fn pages_visibles(&self) -> Option<(usize, usize)> {
        self.visibles.map(|(a, b)| (a + 1, b + 1))
    }

    /// Les pages qui ont un rendu (en cours ou fini), numéros à partir de 1.
    pub fn pages_peintes(&self) -> Vec<usize> {
        self.actives.iter().map(|i| i + 1).collect()
    }

    /// Le prochain instant (ms) où `avancer` aura quelque chose à faire.
    pub fn prochaine_echeance(&self) -> Option<u64> {
        [
            self.fin_de_geste.next_deadline(),
            self.recaler_pixels.next_deadline(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Menu de zoom : le mode coché est le mode en vigueur.
    pub fn mode_coche(&self, mode: ZoomMode) -> bool {
        self.mode == Some(mode)
    }

    /// Menu de zoom : un pourcentage est coché quand l'échelle est libre et égale à lui.
    pub fn pourcentage_coche(&self, pourcentage: f64) -> bool {
        self.mode.is_none() && (self.echelle - pourcentage).abs() < 1e-3
    }

    // ── Ouverture et zone ────────────────────────────────────────────────

    /// La zone qui défile, lue UNE fois (calibrage) : elle est ensuite livrée par
    /// `zone_redimensionnee`. Aucune réaction (ni mode, ni rendu).
    pub fn definir_zone(&mut self, largeur_utile: f64, padding_h: f64, hauteur: f64) {
        self.zone = Zone {
            largeur_utile,
            padding_h,
            hauteur,
        };
        self.geometrie.set_largeur_utile(largeur_utile);
    }

    /// Un document s'ouvre : `nombre_pages` boîtes, toutes de la taille de la page 1
    /// (`base`, corrigée page par page par `taille_page_connue`). Le mode de zoom se
    /// résout tout de suite, les boîtes naissent donc à la bonne taille. Rejoue la
    /// page ou la position demandées avant l'ouverture.
    pub fn ouvrir(
        &mut self,
        nombre_pages: usize,
        base: Taille,
        gap: f64,
        pad_haut: f64,
        densite: f64,
    ) -> PlanRendu {
        self.fermer();
        self.base = base;
        if let Some(mode) = self.mode {
            self.echelle = self.echelle_du_mode(mode);
        }
        self.densite = if densite > 0.0 { densite } else { 1.0 };
        self.geometrie
            .reinitialiser(nombre_pages, base.largeur, base.hauteur);
        self.geometrie.set_espacements(gap, pad_haut);
        self.geometrie.set_echelle(self.echelle);
        self.geometrie.set_densite(self.densite);
        self.geometrie.set_largeur_utile(self.zone.largeur_utile);
        self.etats = vec![EtatPage::Vide; nombre_pages];
        self.pixels_livres = vec![None; nombre_pages];
        self.canvas = vec![None; nombre_pages];
        self.largeur_max.set(None);

        let mut plan = PlanRendu::vide();
        if let Some(page) = self.page_en_attente.take() {
            plan.fusionner(self.aller_a_la_page(page, None));
        }
        if let Some(position) = self.lecture_en_attente.take() {
            let (_, p) = self.aller_a_position_interne(position);
            plan.fusionner(p);
        }
        self.recalculer_visibles();
        plan.fusionner(self.synchroniser_rendu());
        self.maj_page();
        plan.notifier = true;
        plan
    }

    /// Le document se ferme : plus de pages, plus de rendu. Le mode et l'échelle restent.
    pub fn fermer(&mut self) {
        self.geometrie.reinitialiser(0, 0.0, 0.0);
        self.etats.clear();
        self.actives.clear();
        self.pixels_livres.clear();
        self.canvas.clear();
        self.visibles = None;
        self.scroll_top = 0.0;
        self.scroll_left = 0.0;
        self.page_vue = 0;
        self.page_forcee = None;
        self.en_geste = false;
        self.fin_de_geste.cancel();
        self.recaler_pixels.cancel();
        self.champ.restaurer(0);
        self.largeur_max.set(None);
    }

    /// Les espacements de la colonne (CSS `gap` et `padding-top`) : lus une fois au
    /// calibrage. Un changement SANS recalibrage fausse les positions.
    pub fn recalibrer_espacements(&mut self, gap: f64, pad_haut: f64) -> PlanRendu {
        self.geometrie.set_espacements(gap, pad_haut);
        self.reclamper();
        self.recalculer_visibles();
        let mut plan = self.synchroniser_rendu();
        plan.notifier = true;
        plan
    }

    /// La taille réelle d'une page (à l'échelle 1), connue au rendu : les pages
    /// inégales gardent leur taille, aux zooms suivants comme maintenant.
    pub fn taille_page_connue(&mut self, page: usize, largeur: f64, hauteur: f64) -> PlanRendu {
        let Some(i) = page.checked_sub(1) else {
            return PlanRendu::vide();
        };
        if !self.geometrie.set_taille(i, largeur, hauteur) {
            return PlanRendu::vide();
        }
        self.largeur_max.set(None);
        self.reclamper();
        self.recalculer_visibles();
        let mut plan = self.synchroniser_rendu();
        plan.notifier = true;
        plan
    }

    /// La densité de l'écran (ou le zoom de l'app) a changé : la grille de mise en
    /// page change, les canvas peints à l'ancienne densité seraient flous. On repeint.
    pub fn set_densite(&mut self, densite: f64) -> PlanRendu {
        if densite.is_nan() || densite <= 0.0 || (densite - self.densite).abs() < f64::EPSILON {
            return PlanRendu::vide();
        }
        self.densite = densite;
        self.geometrie.set_densite(densite);
        self.largeur_max.set(None);
        self.pixels_livres.iter_mut().for_each(|p| *p = None);
        if self.nombre_pages() == 0 {
            return PlanRendu::vide();
        }
        self.reclamper();
        self.repeindre()
    }

    /// La zone a changé de taille (fenêtre, barre latérale, split, dock). Une
    /// largeur de 0 est IGNORÉE tout entière : la zone est cachée (onglet en
    /// arrière-plan, dock replié), et adapter le zoom à 0 px écraserait les pages
    /// puis ferait perdre la lecture au retour. Les modes suivent la taille, en
    /// geste différé : un dock qui s'anime change la largeur à chaque image.
    pub fn zone_redimensionnee(&mut self, largeur_utile: f64, hauteur: f64) -> PlanRendu {
        if largeur_utile <= 0.0 {
            return PlanRendu::vide();
        }
        self.zone.largeur_utile = largeur_utile;
        self.zone.hauteur = hauteur;
        let change = self.geometrie.set_largeur_utile(largeur_utile);
        let mut plan = PlanRendu::vide();
        plan.notifier = change && self.nombre_pages() > 0;

        if let Some(mode) = self.mode
            && self.nombre_pages() > 0
        {
            let cible = self.echelle_du_mode(mode);
            // Le seuil évite le va-et-vient quand l'échelle oscille avec l'apparition
            // d'une barre de défilement.
            if (cible - self.echelle).abs() > 0.005
                && let Some(p) = self.appliquer_echelle(cible, None, true)
            {
                plan.fusionner(p);
            }
        }
        self.reclamper();
        self.recalculer_visibles();
        plan.fusionner(self.synchroniser_rendu());
        if change {
            self.verifier_pixels();
        }
        self.maj_page();
        plan
    }

    // ── Défilement et pages ──────────────────────────────────────────────

    /// Le défilement a changé (événement de défilement, une fois par image). Peint
    /// seulement les pages qui ENTRENT dans la bande et libère celles qui en sortent ;
    /// les pages déjà peintes ne sont pas touchées.
    pub fn defiler(&mut self, haut: f64, gauche: f64) -> PlanRendu {
        self.scroll_top = haut;
        self.scroll_left = gauche;
        self.reclamper();
        self.recalculer_visibles();
        self.maj_page();
        self.synchroniser_rendu()
    }

    /// La page sous le haut de la zone (à partir de 1).
    pub fn page_courante(&self) -> usize {
        self.geometrie.page_sous_px(self.scroll_top) + 1
    }

    /// La page la PLUS visible (celle de la barre d'outils, comme pdf.js). À égalité
    /// à 0,5 px près, la plus haute l'emporte.
    pub fn page_plus_visible(&self) -> usize {
        let n = self.nombre_pages();
        let g = &self.geometrie;
        let vue_haut = self.scroll_top;
        let vue_bas = vue_haut + self.zone.hauteur;
        let mut meilleure = 0;
        let mut surface = -1.0;
        for i in g.page_sous_px(vue_haut)..n {
            let top = g.top_px(i);
            let bas = top + g.hauteur_px(i);
            if top >= vue_bas {
                break;
            }
            let visible = bas.min(vue_bas) - top.max(vue_haut);
            if visible > surface + 0.5 {
                surface = visible;
                meilleure = i;
            }
        }
        meilleure + 1
    }

    /// Synchronise la page de la barre d'outils (et le champ) avec le défilement.
    /// Rend vrai si la page affichée a changé.
    pub fn maj_page(&mut self) -> bool {
        if self.nombre_pages() == 0 {
            return false;
        }
        let mut page = self.page_plus_visible();
        if let Some((forcee, haut)) = self.page_forcee {
            if (self.scroll_top - haut).abs() < 2.0 {
                page = forcee;
            } else {
                self.page_forcee = None;
            }
        }
        if page == self.page_vue {
            return false;
        }
        self.page_vue = page;
        self.champ.synchroniser(page);
        true
    }

    /// Amène une page en haut de la zone. `y` (coordonnées PDF, origine en BAS de la
    /// page : une destination de table des matières) vise une hauteur précise.
    pub fn aller_a_la_page(&mut self, numero: usize, y: Option<f64>) -> PlanRendu {
        if numero == 0 || numero > self.nombre_pages() {
            return PlanRendu::vide();
        }
        let i = numero - 1;
        let decalage = y.map_or(0.0, |y| {
            (self.geometrie.hauteur_px(i) - y * self.echelle).max(0.0)
        });
        self.scroll_top = self.borner_haut(self.geometrie.top_px(i) + decalage);
        self.page_forcee = Some((numero, self.scroll_top));
        self.recalculer_visibles();
        self.maj_page();
        self.synchroniser_rendu()
    }

    // ── Champ de page ────────────────────────────────────────────────────

    pub fn champ_focaliser(&mut self) {
        self.champ.focaliser();
    }

    pub fn champ_saisir(&mut self, texte: &str) {
        self.champ.saisir(texte);
    }

    /// Entrée (ou perte du focus après une saisie) : va à la page saisie, bornée à
    /// `1..=N` ; une saisie illisible n'y va pas. Le champ affiche ensuite la page courante.
    pub fn champ_valider(&mut self) -> PlanRendu {
        let total = self.nombre_pages();
        let mut plan = PlanRendu::vide();
        if let Some(n) = analyser_entier(&self.champ.valeur)
            && total > 0
        {
            let borne = n.clamp(1, i64::try_from(total).unwrap_or(i64::MAX));
            plan = self.aller_a_la_page(usize::try_from(borne).unwrap_or(1), None);
        }
        self.champ.restaurer(self.page_vue);
        plan
    }

    /// Échap : restaure la page courante et rend le focus.
    pub fn champ_echap(&mut self) {
        self.champ.restaurer(self.page_vue);
        self.champ.focus = false;
    }

    pub fn champ_perdre_focus(&mut self) {
        self.champ.focus = false;
    }

    // ── Zoom ─────────────────────────────────────────────────────────────

    fn echelle_du_mode(&self, mode: ZoomMode) -> f64 {
        // La largeur UTILE (sans padding) : « page width » ajuste à la zone de
        // lecture, pas à la marge que réserve un calque.
        echelle_pour_mode(
            mode,
            self.base,
            Taille {
                largeur: self.zone.largeur_utile,
                hauteur: self.zone.hauteur,
            },
        )
    }

    /// Zoom +/- d'un cran : on quitte le mode, l'échelle devient libre.
    pub fn zoomer_pas(&mut self, sens: Sens) -> PlanRendu {
        self.mode = None;
        let cible = palier_suivant(self.echelle, sens);
        self.appliquer_echelle(cible, None, false)
            .unwrap_or_default()
    }

    pub fn choisir_mode(&mut self, mode: ZoomMode) -> PlanRendu {
        self.mode = Some(mode);
        let cible = self.echelle_du_mode(mode);
        self.appliquer_echelle(cible, None, false)
            .unwrap_or_default()
    }

    /// Un pourcentage du menu : échelle libre.
    pub fn choisir_pourcentage(&mut self, pourcentage: f64) -> PlanRendu {
        self.mode = None;
        self.appliquer_echelle(pourcentage, None, false)
            .unwrap_or_default()
    }

    /// Ctrl/Cmd + `+`, `-`, `0`.
    pub fn executer_raccourci(&mut self, raccourci: Raccourci) -> PlanRendu {
        match raccourci {
            Raccourci::ZoomPlus => self.zoomer_pas(Sens::Plus),
            Raccourci::ZoomMoins => self.zoomer_pas(Sens::Moins),
            Raccourci::ModeAuto => self.choisir_mode(ZoomMode::Auto),
        }
    }

    /// Ctrl + molette et PINCH d'un trackpad (que le navigateur livre comme une
    /// molette `ctrlKey` en petits deltas). Échelle CONTINUE : `exp(−delta × sensibilité)`,
    /// ancrée sous le curseur, repeint différé. `delta_mode` 1 = en lignes, ramené à
    /// des pixels (x16). Sans Ctrl/Cmd, l'événement n'est pas pour nous.
    pub fn zoomer_molette(
        &mut self,
        delta_y: f64,
        delta_mode: u32,
        ctrl_ou_cmd: bool,
        point: Point,
    ) -> ResultatMolette {
        if !ctrl_ou_cmd {
            return ResultatMolette {
                empeche_defaut: false,
                rendu: PlanRendu::vide(),
            };
        }
        let mut res = ResultatMolette {
            empeche_defaut: true,
            rendu: PlanRendu::vide(),
        };
        if self.nombre_pages() == 0 {
            return res;
        }
        let delta = if delta_mode == 1 {
            delta_y * 16.0
        } else {
            delta_y
        };
        self.mode = None;
        let cible = self.echelle * (-delta * SENSIBILITE_MOLETTE).exp();
        res.rendu = self
            .appliquer_echelle(cible, Some(point), true)
            .unwrap_or_default();
        res
    }

    /// LE chemin unique de changement d'échelle : boutons, menu, modes,
    /// redimensionnement, état restauré, pinch. `None` si l'échelle (bornée) ne bouge pas.
    ///
    /// Le point FIXE est sous le curseur (`point`) ou, à défaut, au milieu du haut de
    /// la zone. On retient la page sous lui ET la fraction de cette page où il
    /// tombe, puis on recale le défilement dessus, sinon le zoom ferait sauter le
    /// lecteur ailleurs. L'ancre se lit dans la géométrie, avant ET après : aucune
    /// mesure du DOM. Horizontalement on n'ancre que si un point est donné.
    ///
    /// `differe` : les canvas sont étirés, rien n'est repeint avant [`DELAI_FIN_DE_GESTE`]
    /// ms de silence (cf. [`Self::avancer`]). Sinon, repeint tout de suite.
    pub fn appliquer_echelle(
        &mut self,
        valeur: f64,
        point: Option<Point>,
        differe: bool,
    ) -> Option<PlanRendu> {
        let cible = borner_echelle(valeur);
        if (cible - self.echelle).abs() < 1e-6 {
            return None;
        }
        let largeur_vue = self.zone.largeur_utile + self.zone.padding_h;
        let p = point.unwrap_or(Point {
            x: largeur_vue / 2.0,
            y: 0.0,
        });
        let n = self.nombre_pages();
        let (i, fx, fy) = {
            let g = &self.geometrie;
            let x_plan = p.x + self.scroll_left;
            let y_plan = p.y + self.scroll_top;
            let i = g.page_sous_px(y_plan);
            let fx = if n > 0 && g.largeur_px(i) > 0.0 {
                (x_plan - g.left_px(i)) / g.largeur_px(i)
            } else {
                0.0
            };
            let fy = if n > 0 && g.hauteur_px(i) > 0.0 {
                (y_plan - g.top_px(i)) / g.hauteur_px(i)
            } else {
                0.0
            };
            (i, fx, fy)
        };

        self.echelle = cible;
        self.geometrie.set_echelle(cible);
        self.largeur_max.set(None);
        self.pixels_livres.iter_mut().for_each(|p| *p = None);

        if n > 0 {
            let g = &self.geometrie;
            let haut = g.top_px(i) + fy * g.hauteur_px(i) - p.y;
            let gauche = point.map(|pt| g.left_px(i) + fx * g.largeur_px(i) - pt.x);
            self.scroll_top = haut;
            if let Some(gauche) = gauche {
                self.scroll_left = gauche;
            }
            self.reclamper();
        }
        self.recalculer_visibles();

        let mut plan = if differe {
            self.en_geste = true;
            self.fin_de_geste.call((), self.maintenant);
            PlanRendu {
                notifier: true,
                en_zoom: true,
                ..PlanRendu::vide()
            }
        } else {
            self.repeindre()
        };
        plan.notifier = true;
        self.maj_page();
        Some(plan)
    }

    // ── Temps, repeint, pixels ───────────────────────────────────────────

    /// Fait avancer l'horloge (ms). À la fin d'un geste (100 ms sans nouvel
    /// événement) tout est repeint à la nouvelle échelle ; après un silence, les
    /// canvas dont la taille physique a changé sont recalés.
    pub fn avancer(&mut self, maintenant_ms: u64) -> PlanRendu {
        self.maintenant = maintenant_ms;
        let mut plan = PlanRendu::vide();
        if self.fin_de_geste.poll(maintenant_ms) {
            plan.fusionner(self.repeindre());
        }
        if self.recaler_pixels.poll(maintenant_ms) {
            plan.fusionner(self.recaler());
        }
        plan
    }

    /// Fin d'un geste, ou changement non différé : chaque page visible est repeinte
    /// à la nouvelle échelle, l'ancienne image restant dessous jusqu'à ce que la
    /// nouvelle soit prête ; les pages hors bande sont libérées.
    pub fn repeindre(&mut self) -> PlanRendu {
        self.fin_de_geste.cancel();
        self.recaler_pixels.cancel();
        self.en_geste = false;
        self.recalculer_visibles();
        let mut plan = PlanRendu::vide();
        let mut concernees: BTreeSet<usize> = self.actives.clone();
        if let Some((a, b)) = self.visibles {
            concernees.extend(a..=b);
        }
        for i in concernees {
            self.repeindre_page(i, &mut plan);
        }
        plan.notifier = true;
        plan
    }

    /// Une page repeinte, l'ancienne image gardée dessous ; hors bande, libérée.
    fn repeindre_page(&mut self, i: usize, plan: &mut PlanRendu) {
        let ancienne = matches!(
            self.etats.get(i),
            Some(EtatPage::Prete | EtatPage::EnRendu { ancienne: true })
        );
        if self.est_visible(i) {
            self.marquer_en_rendu(i, ancienne);
            plan.peindre.push(Peinture {
                page: i + 1,
                garder_ancienne: ancienne,
            });
        } else if self.etats.get(i).is_some_and(|e| *e != EtatPage::Vide) {
            self.liberer(i);
            plan.liberer.push(i + 1);
        }
    }

    /// La vue a fini de peindre une page : son canvas fait `taille` pixels.
    pub fn page_prete(&mut self, page: usize, taille: Pixels) {
        let Some(i) = page.checked_sub(1) else { return };
        if !matches!(self.etats.get(i), Some(EtatPage::EnRendu { .. })) {
            return;
        }
        self.etats[i] = EtatPage::Prete;
        self.canvas[i] = Some(taille);
        self.verifier_pixels();
    }

    /// La taille PHYSIQUE à donner au canvas d'une page : celle que mesure la vue
    /// (`pixels_livres`) sinon celle que calcule le modèle depuis la géométrie.
    /// `None` : la page est vide ou cachée, rien à peindre.
    pub fn pixels_attendus(&self, page: usize) -> Option<Pixels> {
        let i = page.checked_sub(1)?;
        if i >= self.nombre_pages() {
            return None;
        }
        if let Some(p) = self.pixels_livres.get(i).copied().flatten() {
            return Some(p);
        }
        let g = &self.geometrie;
        taille_physique(
            g.left_px(i),
            g.top_px(i),
            g.largeur_px(i),
            g.hauteur_px(i),
            self.densite,
        )
    }

    /// La vue a MESURÉ la taille physique d'une page (`None` : cachée, ignorée). Si
    /// le canvas peint n'a plus cette taille sans que l'échelle ait changé (la page
    /// a glissé d'une fraction de pixel), un recalage est programmé au calme.
    pub fn pixels_livres(&mut self, page: usize, taille: Option<Pixels>) {
        let (Some(i), Some(t)) = (page.checked_sub(1), taille) else {
            return;
        };
        if i >= self.nombre_pages() {
            return;
        }
        self.pixels_livres[i] = Some(t);
        self.verifier_pixels();
    }

    /// Programme un recalage si un canvas prêt n'a plus la taille attendue.
    fn verifier_pixels(&mut self) {
        let desaccord = self.actives.iter().any(|&i| {
            self.etats[i] == EtatPage::Prete
                && match (self.canvas[i], self.pixels_attendus(i + 1)) {
                    (Some(c), Some(p)) => c != p,
                    _ => false,
                }
        });
        if desaccord {
            self.recaler_pixels.call((), self.maintenant);
        }
    }

    /// Repeint les pages prêtes dont le canvas n'a plus la bonne taille physique.
    /// Rien pendant un geste : sa fin repeint tout.
    fn recaler(&mut self) -> PlanRendu {
        let mut plan = PlanRendu::vide();
        if self.en_geste {
            return plan;
        }
        let a_refaire: Vec<usize> = self
            .actives
            .iter()
            .copied()
            .filter(|&i| {
                self.etats[i] == EtatPage::Prete
                    && matches!(
                        (self.canvas[i], self.pixels_attendus(i + 1)),
                        (Some(c), Some(p)) if c != p
                    )
            })
            .collect();
        for i in a_refaire {
            self.repeindre_page(i, &mut plan);
            plan.notifier = true;
        }
        plan
    }

    // ── Virtualisation ───────────────────────────────────────────────────

    fn est_visible(&self, i: usize) -> bool {
        self.visibles.is_some_and(|(a, b)| (a..=b).contains(&i))
    }

    fn marquer_en_rendu(&mut self, i: usize, ancienne: bool) {
        self.etats[i] = EtatPage::EnRendu { ancienne };
        self.actives.insert(i);
        self.canvas[i] = None;
    }

    fn liberer(&mut self, i: usize) {
        self.etats[i] = EtatPage::Vide;
        self.actives.remove(&i);
        self.canvas[i] = None;
    }

    /// Les pages dans la bande peinte : la zone visible plus une hauteur d'écran
    /// au-dessus et en dessous. Zone cachée (hauteur 0) : aucune page.
    fn recalculer_visibles(&mut self) {
        let n = self.nombre_pages();
        if n == 0 || self.zone.hauteur <= 0.0 {
            self.visibles = None;
            return;
        }
        let marge = self.zone.hauteur * MARGE_RENDU;
        let haut = self.scroll_top - marge;
        let bas = self.scroll_top + self.zone.hauteur + marge;
        let g = &self.geometrie;
        let premiere = g.page_sous_px(haut);
        if g.top_px(premiere) + g.hauteur_px(premiere) < haut {
            self.visibles = None;
            return;
        }
        let mut derniere = premiere;
        while derniere + 1 < n && g.top_px(derniere + 1) <= bas {
            derniere += 1;
        }
        self.visibles = (g.top_px(premiere) <= bas).then_some((premiere, derniere));
    }

    /// Met les pages à jour d'après la bande : peint celles qui entrent (et sont
    /// vides), libère celles qui sortent. Pendant un geste, rien : la fin du geste
    /// s'en charge (`repeindre`), sinon chaque pas d'un pinch peindrait à une
    /// échelle périmée.
    fn synchroniser_rendu(&mut self) -> PlanRendu {
        let mut plan = PlanRendu::vide();
        if self.en_geste {
            return plan;
        }
        let sortantes: Vec<usize> = self
            .actives
            .iter()
            .copied()
            .filter(|&i| !self.est_visible(i))
            .collect();
        for i in sortantes {
            self.liberer(i);
            plan.liberer.push(i + 1);
        }
        if let Some((a, b)) = self.visibles {
            for i in a..=b {
                if self.etats[i] == EtatPage::Vide {
                    self.marquer_en_rendu(i, false);
                    plan.peindre.push(Peinture {
                        page: i + 1,
                        garder_ancienne: false,
                    });
                }
            }
        }
        plan.notifier = !plan.peindre.is_empty() || !plan.liberer.is_empty();
        plan
    }

    // ── Bornes du défilement ─────────────────────────────────────────────

    fn largeur_max_px(&self) -> f64 {
        if let Some(l) = self.largeur_max.get() {
            return l;
        }
        let l = (0..self.nombre_pages())
            .map(|i| self.geometrie.largeur_px(i))
            .fold(0.0, f64::max);
        self.largeur_max.set(Some(l));
        l
    }

    fn defilement_max_y(&self) -> f64 {
        (self.geometrie.hauteur_totale_px() - self.zone.hauteur).max(0.0)
    }

    /// Largeur défilable : le sizer (zone utile ou page la plus large) plus les paddings.
    fn largeur_defilable(&self) -> f64 {
        self.zone.largeur_utile.max(self.largeur_max_px()) + self.zone.padding_h
    }

    fn defilement_max_x(&self) -> f64 {
        (self.largeur_defilable() - (self.zone.largeur_utile + self.zone.padding_h)).max(0.0)
    }

    fn borner_haut(&self, haut: f64) -> f64 {
        haut.clamp(0.0, self.defilement_max_y())
    }

    /// Le navigateur borne le défilement quand le contenu change : le modèle aussi.
    fn reclamper(&mut self) {
        self.scroll_top = self.borner_haut(self.scroll_top);
        self.scroll_left = self.scroll_left.clamp(0.0, self.defilement_max_x());
    }

    // ── Position de lecture et état éphémère ─────────────────────────────

    /// La position de lecture : page sous le haut de la zone, fraction de hauteur
    /// dans cette page, fraction horizontale du défilement (arrondies à 1e-4).
    /// Sans pages, la position demandée et pas encore appliquée.
    pub fn lire_position(&self) -> Option<PositionLecture> {
        if self.nombre_pages() == 0 {
            return self.lecture_en_attente;
        }
        let g = &self.geometrie;
        let i = g.page_sous_px(self.scroll_top);
        let h = g.hauteur_px(i);
        let largeur = self.largeur_defilable();
        let arrondi4 = |x: f64| (x * 1e4 + 0.5).floor() / 1e4;
        Some(PositionLecture {
            page: i + 1,
            fy: if h > 0.0 {
                arrondi4((self.scroll_top - g.top_px(i)) / h)
            } else {
                0.0
            },
            fx: if largeur > 0.0 {
                arrondi4(self.scroll_left / largeur)
            } else {
                0.0
            },
        })
    }

    /// Va à une position de lecture. Sans pages elle est mémorisée et rejouée à
    /// `ouvrir`. Sinon rend la reprise à poursuivre à chaque image tant que la
    /// position bouge (`None` : déjà tenue).
    pub fn aller_a_position(&mut self, position: PositionLecture) -> (Option<Reprise>, PlanRendu) {
        if self.nombre_pages() == 0 {
            self.lecture_en_attente = Some(position);
            return (None, PlanRendu::vide());
        }
        self.aller_a_position_interne(position)
    }

    fn aller_a_position_interne(
        &mut self,
        position: PositionLecture,
    ) -> (Option<Reprise>, PlanRendu) {
        let mut reprise = Reprise { position, calme: 0 };
        let (fini, plan) = self.pas_de_reprise(&mut reprise);
        (if fini { None } else { Some(reprise) }, plan)
    }

    fn pas_de_reprise(&mut self, reprise: &mut Reprise) -> (bool, PlanRendu) {
        let n = self.nombre_pages();
        if n == 0 {
            return (true, PlanRendu::vide());
        }
        let p = reprise.position;
        let i = p.page.clamp(1, n) - 1;
        let g = &self.geometrie;
        let cible = g.top_px(i) + p.fy * g.hauteur_px(i);
        let ecart = cible - self.scroll_top;
        self.scroll_top = cible;
        self.scroll_left = p.fx * self.largeur_defilable();
        self.reclamper();
        let au_bout = self.scroll_top >= self.defilement_max_y() - 1.0;
        reprise.calme = if ecart.abs() < 1.5 || au_bout {
            reprise.calme + 1
        } else {
            0
        };
        self.recalculer_visibles();
        self.maj_page();
        (reprise.calme >= 2, self.synchroniser_rendu())
    }

    /// La part de l'état éphémère que porte le modèle.
    pub fn etat_ephemere(&self) -> EtatEphemere {
        EtatEphemere {
            page: if self.nombre_pages() == 0 {
                self.page_en_attente.unwrap_or(0)
            } else {
                self.page_courante()
            },
            scale: self.echelle,
            zoom_mode: self.mode,
        }
    }

    /// Restaure `page`, `scale`, `zoomMode` d'un état sauvegardé. Un mode connu prime
    /// sur l'échelle (qui n'en est que le résultat du moment). Une échelle hors bornes
    /// est ignorée. La page est ignorée si une position `scroll` (plus précise)
    /// l'accompagne ; sans document elle est mémorisée et rejouée à `ouvrir`.
    pub fn restaurer_etat_ephemere(&mut self, etat: &Value) -> PlanRendu {
        let mut plan = PlanRendu::vide();
        let mode = etat
            .get("zoomMode")
            .and_then(Value::as_str)
            .and_then(mode_depuis_texte);
        let echelle = etat
            .get("scale")
            .and_then(Value::as_f64)
            .filter(|s| (ECHELLE_MIN..=ECHELLE_MAX).contains(s));
        let pages = self.nombre_pages() > 0;
        if let Some(m) = mode {
            self.mode = Some(m);
            if pages {
                let cible = self.echelle_du_mode(m);
                if let Some(p) = self.appliquer_echelle(cible, None, false) {
                    plan.fusionner(p);
                }
            }
        } else if let Some(s) = echelle {
            self.mode = None;
            if pages {
                if let Some(p) = self.appliquer_echelle(s, None, false) {
                    plan.fusionner(p);
                }
            } else {
                self.echelle = s;
            }
        }

        let Some(page) = etat.get("page").and_then(entier_positif) else {
            return plan;
        };
        if etat.get("scroll").and_then(|s| s.get("page")).is_some() {
            return plan;
        }
        if pages {
            plan.fusionner(self.aller_a_la_page(page, None));
        } else {
            self.page_en_attente = Some(page);
        }
        plan
    }
}
