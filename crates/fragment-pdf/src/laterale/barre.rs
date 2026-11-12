// Port de app/src/ui/views/pdf/PdfSidebar.ts et de la section « barre latérale » de pdf-view.css
//
// Les constantes de mise en page viennent de pdf-view.css : bordure droite 1 px, vue des
// vignettes en padding 5 px / 7 px, vignette en padding 4 px, écart 8 px entre vignettes,
// largeur de la barre dans [120, 480] et jamais plus de la moitié de la vue.
// La zone d'observation des vignettes déborde de 50 % de la hauteur visible.

use crate::pdf_outline::EntreeToc;

use super::toc::{ArbreToc, Chemin, Destination, MESSAGE_SANS_TABLE};

pub const LARGEUR_DEFAUT: f64 = 200.0;
pub const LARGEUR_MIN: f64 = 120.0;
pub const LARGEUR_MAX: f64 = 480.0;
/// Plafond de la barre en part de la vue : le document prime en fenêtre étroite.
pub const RATIO_MAX_VUE: f64 = 0.5;

const BORDURE: f64 = 1.0;
const PADDING_VUE_H: f64 = 14.0;
const PADDING_VUE_V: f64 = 10.0;
const PADDING_VIGNETTE: f64 = 4.0;
const ECART: f64 = 8.0;
/// Débord de la zone d'observation des vignettes, en part de la hauteur visible.
const MARGE_OBSERVATION: f64 = 0.5;
/// Marge laissée quand on fait défiler la barre pour montrer une vignette.
const MARGE_DEFILEMENT: f64 = 8.0;

/// Ce que la barre affiche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModeBarre {
    #[default]
    Vignettes,
    TableDesMatieres,
}

impl ModeBarre {
    /// Le nom stocké dans l'état éphémère (identique à celui de Fragment).
    pub fn vers_texte(self) -> &'static str {
        match self {
            Self::Vignettes => "thumbnails",
            Self::TableDesMatieres => "outline",
        }
    }

    pub fn depuis_texte(texte: &str) -> Option<Self> {
        match texte {
            "thumbnails" => Some(Self::Vignettes),
            "outline" => Some(Self::TableDesMatieres),
            _ => None,
        }
    }
}

/// L'état éphémère de la barre (ouverte, mode, largeur).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EtatBarre {
    pub ouverte: bool,
    pub mode: ModeBarre,
    pub largeur: f64,
}

/// Le travail demandé à la vue après un changement : vignettes dont le canvas est jeté,
/// vignettes (1-based, ordre croissant) à peindre. Une seule peinture à la fois côté vue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanVignettes {
    pub a_liberer: Vec<usize>,
    pub a_peindre: Vec<usize>,
}

impl PlanVignettes {
    pub fn est_vide(&self) -> bool {
        self.a_liberer.is_empty() && self.a_peindre.is_empty()
    }
}

/// Le modèle de la barre latérale.
#[derive(Debug, Clone)]
pub struct BarreLaterale {
    mode: ModeBarre,
    ouverte: bool,
    largeur: f64,
    /// Rapport hauteur / largeur de chaque page (page 1-based = indice + 1).
    ratios: Vec<f64>,
    page_active: usize,
    toc: ArbreToc,
    hauteur_visible: f64,
    scroll_top: f64,
    /// Vignettes dont un canvas existe (peint ou en cours), croissantes.
    peintes: Vec<usize>,
    /// Poignée saisie : abscisse de départ et largeur à ce moment.
    glissement: Option<(f64, f64)>,
}

impl Default for BarreLaterale {
    fn default() -> Self {
        Self::new()
    }
}

impl BarreLaterale {
    pub fn new() -> Self {
        Self {
            mode: ModeBarre::Vignettes,
            ouverte: false,
            largeur: LARGEUR_DEFAUT,
            ratios: Vec::new(),
            page_active: 0,
            toc: ArbreToc::default(),
            hauteur_visible: 0.0,
            scroll_top: 0.0,
            peintes: Vec::new(),
            glissement: None,
        }
    }

    // ── État visible ─────────────────────────────────────────────────────

    pub fn mode(&self) -> ModeBarre {
        self.mode
    }

    pub fn est_ouverte(&self) -> bool {
        self.ouverte
    }

    /// La largeur demandée (bornée), avant le plafond de la moitié de la vue.
    pub fn largeur(&self) -> f64 {
        self.largeur
    }

    pub fn page_active(&self) -> usize {
        self.page_active
    }

    pub fn scroll_top(&self) -> f64 {
        self.scroll_top
    }

    pub fn toc(&self) -> &ArbreToc {
        &self.toc
    }

    pub fn a_une_table_des_matieres(&self) -> bool {
        !self.toc.est_vide()
    }

    /// `Some(message)` quand la vue affiche le texte « pas de table » à la place de l'arbre.
    pub fn message_vide(&self) -> Option<&'static str> {
        (self.mode == ModeBarre::TableDesMatieres && self.toc.est_vide())
            .then_some(MESSAGE_SANS_TABLE)
    }

    pub fn etat(&self) -> EtatBarre {
        EtatBarre {
            ouverte: self.ouverte,
            mode: self.mode,
            largeur: self.largeur,
        }
    }

    pub fn nombre_vignettes(&self) -> usize {
        self.ratios.len()
    }

    pub fn set_ouverte(&mut self, ouverte: bool) -> PlanVignettes {
        self.ouverte = ouverte;
        self.appliquer()
    }

    /// Le bouton « Toggle sidebar » : ouvre si fermée, ferme si ouverte.
    pub fn basculer(&mut self) -> PlanVignettes {
        self.set_ouverte(!self.ouverte)
    }

    pub fn set_mode(&mut self, mode: ModeBarre) -> PlanVignettes {
        self.mode = mode;
        self.appliquer()
    }

    /// Borne `px` à [120, 480] et arrondit (le plafond de la moitié de la vue se lit
    /// à part : `largeur_effective`).
    pub fn set_largeur(&mut self, px: f64) {
        self.largeur = px.clamp(LARGEUR_MIN, LARGEUR_MAX).round();
    }

    /// La largeur réellement occupée dans une vue de `largeur_vue` px : 0 si fermée,
    /// sinon la largeur demandée plafonnée à la moitié de la vue.
    pub fn largeur_effective(&self, largeur_vue: f64) -> f64 {
        if !self.ouverte {
            return 0.0;
        }
        self.largeur.min(largeur_vue.max(0.0) * RATIO_MAX_VUE)
    }

    /// La largeur qui reste au document : celle que « Page width » doit suivre.
    pub fn zone_utile(&self, largeur_vue: f64) -> f64 {
        largeur_vue - self.largeur_effective(largeur_vue)
    }

    /// Les vignettes ne vivent que barre ouverte ET en mode vignettes (sinon la barre
    /// est masquée, rien ne s'intersecte, rien ne se peint).
    fn appliquer(&mut self) -> PlanVignettes {
        if self.ouverte && self.mode == ModeBarre::Vignettes {
            self.montrer_vignette(self.page_active);
        }
        self.recalculer()
    }

    // ── Document ─────────────────────────────────────────────────────────

    /// Bâtit les vignettes (`ratio` = hauteur / largeur de la page 1) et la table des matières.
    pub fn charger(&mut self, pages: usize, ratio: f64, toc: Vec<EntreeToc>) -> PlanVignettes {
        self.vider();
        self.ratios = vec![ratio; pages];
        self.toc.remplacer(toc);
        self.set_page_courante(self.page_active.max(1))
    }

    pub fn vider(&mut self) {
        self.ratios.clear();
        self.peintes.clear();
        self.toc.remplacer(Vec::new());
        self.scroll_top = 0.0;
        self.glissement = None;
    }

    /// Une page qui n'a pas le format de la page 1 corrige sa vignette (après sa peinture).
    pub fn format_page_connu(&mut self, page: usize, largeur: f64, hauteur: f64) -> PlanVignettes {
        if largeur > 0.0
            && let Some(r) = page.checked_sub(1).and_then(|i| self.ratios.get_mut(i))
        {
            *r = hauteur / largeur;
        }
        self.recalculer()
    }

    // ── Vignettes ────────────────────────────────────────────────────────

    /// La largeur de l'image d'une vignette pour une barre de `largeur_barre` px.
    pub fn largeur_image(largeur_barre: f64) -> f64 {
        (largeur_barre - BORDURE - 2.0 * PADDING_VUE_H - 2.0 * PADDING_VIGNETTE).max(1.0)
    }

    fn hauteur_vignette(&self, indice: usize) -> f64 {
        Self::largeur_image(self.largeur) * self.ratios[indice] + 2.0 * PADDING_VIGNETTE
    }

    /// Le haut de la vignette `indice` (0-based) dans le contenu défilant.
    fn haut_vignette(&self, indice: usize) -> f64 {
        (0..indice).fold(PADDING_VUE_V, |haut, i| {
            haut + self.hauteur_vignette(i) + ECART
        })
    }

    /// La hauteur visible de la barre (mesurée par la vue).
    pub fn definir_hauteur(&mut self, hauteur: f64) -> PlanVignettes {
        self.hauteur_visible = hauteur.max(0.0);
        self.recalculer()
    }

    /// La barre défile (molette, barre de défilement).
    pub fn defiler(&mut self, haut: f64) -> PlanVignettes {
        self.scroll_top = haut.max(0.0);
        self.recalculer()
    }

    /// La page courante de la vue change : sa vignette devient active et, si elle sort,
    /// la barre défile juste ce qu'il faut (jamais à chaque page tournée).
    pub fn set_page_courante(&mut self, page: usize) -> PlanVignettes {
        self.page_active = page;
        if self.ouverte && self.mode == ModeBarre::Vignettes {
            self.montrer_vignette(page);
        }
        self.recalculer()
    }

    fn montrer_vignette(&mut self, page: usize) {
        let Some(i) = page.checked_sub(1).filter(|i| *i < self.ratios.len()) else {
            return;
        };
        let haut = self.haut_vignette(i) - self.scroll_top;
        let bas = haut + self.hauteur_vignette(i);
        if haut < 0.0 {
            self.scroll_top += haut - MARGE_DEFILEMENT;
        } else if bas > self.hauteur_visible {
            self.scroll_top += bas - self.hauteur_visible + MARGE_DEFILEMENT;
        }
        self.scroll_top = self.scroll_top.max(0.0);
    }

    /// Les vignettes (1-based) dans la zone d'observation : vue + 50 % de part et d'autre.
    pub fn vignettes_visibles(&self) -> Vec<usize> {
        if !self.ouverte || self.mode != ModeBarre::Vignettes || self.hauteur_visible <= 0.0 {
            return Vec::new();
        }
        let marge = self.hauteur_visible * MARGE_OBSERVATION;
        let (debut, fin) = (
            self.scroll_top - marge,
            self.scroll_top + self.hauteur_visible + marge,
        );
        let mut visibles = Vec::new();
        let mut haut = PADDING_VUE_V;
        for i in 0..self.ratios.len() {
            let bas = haut + self.hauteur_vignette(i);
            if bas > debut && haut < fin {
                visibles.push(i + 1);
            }
            haut = bas + ECART;
            if haut >= fin {
                break;
            }
        }
        visibles
    }

    /// Les vignettes qui ont un canvas, croissantes.
    pub fn vignettes_peintes(&self) -> &[usize] {
        &self.peintes
    }

    /// Aligne les canvas sur les vignettes visibles : jette ceux qui sortent, demande les nouveaux.
    fn recalculer(&mut self) -> PlanVignettes {
        let visibles = self.vignettes_visibles();
        let a_liberer = self
            .peintes
            .iter()
            .copied()
            .filter(|p| !visibles.contains(p))
            .collect();
        let a_peindre = visibles
            .iter()
            .copied()
            .filter(|p| !self.peintes.contains(p))
            .collect();
        self.peintes = visibles;
        PlanVignettes {
            a_liberer,
            a_peindre,
        }
    }

    /// Un clic sur une vignette : la vue va à cette page (haut de page).
    pub fn cliquer_vignette(&self, page: usize) -> Option<Destination> {
        (1..=self.ratios.len())
            .contains(&page)
            .then_some(Destination {
                page: page as u32,
                y: None,
            })
    }

    // ── Table des matières ───────────────────────────────────────────────

    pub fn basculer_entree(&mut self, chemin: &[usize]) {
        self.toc.basculer(chemin);
    }

    pub fn cliquer_entree(&mut self, chemin: &[usize]) -> Option<Destination> {
        self.toc.cliquer(chemin)
    }

    /// « Reveal page in table of contents » : passe en table des matières, ouvre la barre,
    /// déplie et surligne l'entrée de la page. Rend le chemin à centrer.
    pub fn reveler(&mut self, page: u32) -> (PlanVignettes, Option<Chemin>) {
        self.mode = ModeBarre::TableDesMatieres;
        self.ouverte = true;
        let plan = self.appliquer();
        (plan, self.toc.reveler(page))
    }

    // ── Poignée ──────────────────────────────────────────────────────────

    pub fn poignee_saisir(&mut self, x: f64) {
        self.glissement = Some((x, self.largeur));
    }

    /// Déplacement de la poignée : la largeur suit, bornée.
    pub fn poignee_deplacer(&mut self, x: f64) {
        if let Some((depart, largeur_depart)) = self.glissement {
            self.set_largeur(largeur_depart + x - depart);
        }
    }

    /// Relâchement : les vignettes peintes à l'ancienne largeur seraient floues, on les
    /// jette toutes et on repeint les visibles.
    pub fn poignee_relacher(&mut self) -> PlanVignettes {
        if self.glissement.take().is_none() {
            return PlanVignettes::default();
        }
        let a_liberer = std::mem::take(&mut self.peintes);
        let plan = self.recalculer();
        PlanVignettes {
            a_liberer,
            a_peindre: plan.a_peindre,
        }
    }

    pub fn en_glissement(&self) -> bool {
        self.glissement.is_some()
    }

    /// Restaure le mode et la largeur de l'état éphémère (l'ouverture se règle à part).
    pub fn restaurer(&mut self, mode: Option<&str>, largeur: Option<f64>) -> PlanVignettes {
        if let Some(m) = mode.and_then(ModeBarre::depuis_texte) {
            self.mode = m;
        }
        if let Some(l) = largeur.filter(|l| l.is_finite()) {
            self.set_largeur(l);
        }
        self.appliquer()
    }
}
