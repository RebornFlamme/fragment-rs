// Port de app/src/ui/views/pdf/PdfSidebar.ts (charger, vider, peindreVignette, libererVignette,
// setPageCourante, reveler, poignée) et de app/src/ui/views/pdf/devicePixels.ts (taille mesurée)
//
// L'état de la barre latérale sur GPUI. Le modèle (`BarreLaterale`) décide de tout : vignettes
// visibles, page active, arbre, largeur. Cette entité :
// - mesure la zone de contenu et la densité, et livre la hauteur visible au modèle ;
// - applique ses plans aux vignettes (images jetées, rendus annulés) ;
// - mesure, à chaque image, les pixels physiques où chaque vignette sera peinte et demande au fil
//   un rendu EXACTEMENT à cette taille (comme `PixelsPhysiques` côté TS) ;
// - relaie en événements ce que la vue montée doit faire (aller à une page).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use fragment_pdf::laterale::{BarreLaterale, Chemin, ModeBarre, PlanVignettes};
use fragment_pdf::pdf_outline::EntreeToc;
use gpui_kit::{
    Bounds, Context, EventEmitter, Pixels as PixelsGpui, RenderImage, ScrollHandle, Task,
};

use super::RECOLTE_MS;
use crate::echelle::Echelle;
use crate::pdf_barre::{self, BarrePdf};
use crate::pdf_pages::Demande;
use crate::pdf_pages::Pixels;
use crate::pdf_pages::fil::{FilDeRendu, Reponse};

/// Ce que la barre demande à la vue montée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvenementLaterale {
    /// Aller à une page (à partir de 1) ; `y` en coordonnées PDF, `None` = haut de page.
    Aller { page: u32, y: Option<f64> },
    /// La poignée est relâchée : la largeur demandée change (à écrire dans l'état éphémère).
    LargeurChangee(f64),
}

/// Ce qu'une vignette affiche, pour les tests et la vue montée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EtatVignette {
    /// La taille physique de l'image courante.
    pub image: Option<Pixels>,
    /// La dernière taille physique mesurée de sa boîte.
    pub mesure: Option<Pixels>,
    /// Un rendu est demandé ou en vol.
    pub en_cours: bool,
}

/// Une vignette qui a un canvas (peint ou en cours).
#[derive(Default)]
pub(super) struct Vignette {
    pub(super) image: Option<(Arc<RenderImage>, Pixels)>,
    /// Le rendu demandé : son jeton et la taille qu'il vise.
    pub(super) en_cours: Option<(u64, Pixels)>,
}

/// La barre latérale d'un PDF ouvert.
pub struct LateralePdf {
    pub(super) modele: BarreLaterale,
    pub(super) fil: Option<FilDeRendu>,
    attend_ouverture: bool,
    pub(super) erreur: Option<String>,
    /// La table des matières à poser dès que le document est ouvert.
    toc_en_attente: Vec<EntreeToc>,
    /// Hauteur / largeur de chaque page (page 1-based = indice + 1).
    pub(super) ratios: Vec<f64>,
    pub(super) vignettes: HashMap<usize, Vignette>,
    /// La dernière taille physique mesurée de chaque vignette visible.
    livrees: HashMap<usize, Pixels>,
    prochain_jeton: u64,
    /// Les mesures posées au prépeint des images, lues après l'image.
    pub(super) mesures_image: Rc<RefCell<Vec<(usize, Pixels)>>>,
    /// Les images jetées, à retirer de l'atlas de GPUI à la prochaine peinture.
    pub(super) a_jeter: Rc<RefCell<Vec<Arc<RenderImage>>>>,
    /// Le contenu mesuré (px CSS) : largeur, hauteur.
    pub(super) contenu: (f64, f64),
    pub(super) defilement_toc: ScrollHandle,
    /// L'entrée à centrer dans l'arbre au prochain rendu (« Reveal »).
    pub(super) a_centrer: Option<Chemin>,
    recolte: Option<Task<()>>,
    recolte_active: bool,
}

impl EventEmitter<EvenementLaterale> for LateralePdf {}

impl LateralePdf {
    /// Une barre sur le document que rend `fil` (ouvert par lui, de façon asynchrone) ; `toc` est
    /// sa table des matières déjà résolue (vide s'il n'en a pas).
    pub fn new(fil: FilDeRendu, toc: Vec<EntreeToc>, cx: &mut Context<Self>) -> Self {
        let mut v = Self {
            modele: BarreLaterale::new(),
            fil: None,
            attend_ouverture: false,
            erreur: None,
            toc_en_attente: Vec::new(),
            ratios: Vec::new(),
            vignettes: HashMap::new(),
            livrees: HashMap::new(),
            prochain_jeton: 0,
            mesures_image: Rc::new(RefCell::new(Vec::new())),
            a_jeter: Rc::new(RefCell::new(Vec::new())),
            contenu: (0.0, 0.0),
            defilement_toc: ScrollHandle::new(),
            a_centrer: None,
            recolte: None,
            recolte_active: false,
        };
        v.remplacer(fil, toc, cx);
        v
    }

    /// Ferme le document courant et ouvre celui que rend `fil`. L'ouverture, le mode et la
    /// largeur de la barre restent.
    pub fn remplacer(&mut self, fil: FilDeRendu, toc: Vec<EntreeToc>, cx: &mut Context<Self>) {
        self.vider();
        self.fil = Some(fil);
        self.toc_en_attente = toc;
        self.attend_ouverture = true;
        self.lancer_recolte(cx);
        cx.notify();
    }

    /// Jette vignettes, images, arbre : rien n'est plus demandé au fil.
    fn vider(&mut self) {
        if let Some(fil) = self.fil.as_mut() {
            for v in self.vignettes.values() {
                if let Some((jeton, _)) = v.en_cours {
                    fil.annuler(jeton);
                }
            }
        }
        for (_, v) in self.vignettes.drain() {
            self.a_jeter
                .borrow_mut()
                .extend(v.image.map(|(image, _)| image));
        }
        self.livrees.clear();
        self.ratios.clear();
        self.erreur = None;
        self.a_centrer = None;
        self.modele.vider();
    }

    // ── Lecture ──────────────────────────────────────────────────────────

    /// Le modèle (mode, largeur, page active, arbre…).
    pub fn modele(&self) -> &BarreLaterale {
        &self.modele
    }

    /// L'erreur d'ouverture du document, s'il y en a une.
    pub fn erreur(&self) -> Option<&str> {
        self.erreur.as_deref()
    }

    /// Ce que montre la vignette `page` (à partir de 1) ; `None` si elle n'a pas de canvas.
    pub fn etat_vignette(&self, page: usize) -> Option<EtatVignette> {
        self.vignettes.get(&page).map(|v| EtatVignette {
            image: v.image.as_ref().map(|(_, p)| *p),
            mesure: self.livrees.get(&page).copied(),
            en_cours: v.en_cours.is_some(),
        })
    }

    /// Les vignettes qui ont un canvas, croissantes.
    pub fn vignettes_occupees(&self) -> Vec<usize> {
        let mut pages: Vec<usize> = self.vignettes.keys().copied().collect();
        pages.sort_unstable();
        pages
    }

    /// Un rendu au moins est demandé ou en vol.
    pub fn rendus_en_cours(&self) -> bool {
        self.attend_ouverture || self.fil.as_ref().is_some_and(FilDeRendu::occupe)
    }

    /// La largeur que la barre occupe dans une vue de `largeur_vue` px CSS (0 si fermée).
    pub fn largeur_effective(&self, largeur_vue: f64) -> f64 {
        self.modele.largeur_effective(largeur_vue)
    }

    // ── Actions ──────────────────────────────────────────────────────────

    /// LA porte d'entrée des changements du modèle : `f` agit sur lui, son plan est appliqué.
    pub fn agir(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut BarreLaterale) -> PlanVignettes,
    ) {
        let plan = f(&mut self.modele);
        self.appliquer_plan(plan, cx);
        cx.notify();
    }

    /// Ouvre ou ferme la barre (le bouton « Toggle sidebar »).
    pub fn set_ouverte(&mut self, ouverte: bool, cx: &mut Context<Self>) {
        self.agir(cx, |m| m.set_ouverte(ouverte));
    }

    /// Choisit le contenu : vignettes ou table des matières.
    pub fn set_mode(&mut self, mode: ModeBarre, cx: &mut Context<Self>) {
        self.agir(cx, |m| m.set_mode(mode));
    }

    /// La page courante de la vue change : sa vignette devient active et, si elle sort, la barre
    /// défile juste ce qu'il faut.
    pub fn set_page_courante(&mut self, page: usize, cx: &mut Context<Self>) {
        if page != self.modele.page_active() {
            self.agir(cx, |m| m.set_page_courante(page));
        }
    }

    /// « Reveal page in table of contents » : table des matières, barre ouverte, entrée de la page
    /// dépliée, active et centrée.
    pub fn reveler(&mut self, page: u32, cx: &mut Context<Self>) {
        let mut chemin = None;
        self.agir(cx, |m| {
            let (plan, c) = m.reveler(page);
            chemin = c;
            plan
        });
        self.a_centrer = chemin;
    }

    /// Restaure le mode et la largeur de l'état éphémère (l'ouverture se règle à part).
    pub fn restaurer(&mut self, mode: Option<&str>, largeur: Option<f64>, cx: &mut Context<Self>) {
        self.agir(cx, |m| m.restaurer(mode, largeur));
    }

    /// Reprend l'état de la barre d'outils : ouverture, contenu, et la page à révéler (consommée).
    pub fn synchroniser(&mut self, barre: &mut BarrePdf, cx: &mut Context<Self>) {
        let mode = match barre.mode_barre() {
            pdf_barre::ModeBarre::Vignettes => ModeBarre::Vignettes,
            pdf_barre::ModeBarre::Plan => ModeBarre::TableDesMatieres,
        };
        let ouverte = barre.barre_ouverte();
        self.agir(cx, |m| {
            let mut plan = m.set_mode(mode);
            let suite = m.set_ouverte(ouverte);
            plan.a_liberer.extend(suite.a_liberer);
            plan.a_peindre.extend(suite.a_peindre);
            plan
        });
        if let Some(page) = barre.prendre_revele() {
            self.reveler(u32::try_from(page).unwrap_or(u32::MAX), cx);
        }
    }

    /// Un clic sur la vignette `page` : la vue va à cette page.
    pub(super) fn cliquer_vignette(&mut self, page: usize, cx: &mut Context<Self>) {
        if let Some(d) = self.modele.cliquer_vignette(page) {
            cx.emit(EvenementLaterale::Aller {
                page: d.page,
                y: d.y,
            });
        }
    }

    /// Un clic sur une entrée de l'arbre (hors chevron) : active, et la vue va à sa page.
    pub(super) fn cliquer_entree(&mut self, chemin: &[usize], cx: &mut Context<Self>) {
        if let Some(d) = self.modele.cliquer_entree(chemin) {
            cx.emit(EvenementLaterale::Aller {
                page: d.page,
                y: d.y,
            });
        }
        cx.notify();
    }

    /// Le chevron : replie ou déplie.
    pub(super) fn basculer_entree(&mut self, chemin: &[usize], cx: &mut Context<Self>) {
        self.modele.basculer_entree(chemin);
        cx.notify();
    }

    // ── Poignée ──────────────────────────────────────────────────────────

    pub(super) fn poignee_saisir(&mut self, x: f64, cx: &mut Context<Self>) {
        self.modele.poignee_saisir(x);
        cx.notify();
    }

    pub(super) fn poignee_deplacer(&mut self, x: f64, cx: &mut Context<Self>) {
        if self.modele.en_glissement() {
            self.modele.poignee_deplacer(x);
            cx.notify();
        }
    }

    /// Relâchement : les vignettes peintes à l'ancienne largeur seraient floues, on les repeint.
    pub(super) fn poignee_relacher(&mut self, cx: &mut Context<Self>) {
        if !self.modele.en_glissement() {
            return;
        }
        let plan = self.modele.poignee_relacher();
        self.appliquer_plan(plan, cx);
        cx.emit(EvenementLaterale::LargeurChangee(self.modele.largeur()));
        cx.notify();
    }

    // ── Vignettes : plans, mesures, rendus ───────────────────────────────

    fn appliquer_plan(&mut self, plan: PlanVignettes, cx: &mut Context<Self>) {
        for page in plan.a_liberer {
            self.liberer(page);
        }
        self.reconcilier(cx);
    }

    /// Jette le canvas de `page` : son rendu est annulé, son image rendue à l'atlas.
    fn liberer(&mut self, page: usize) {
        self.livrees.remove(&page);
        let Some(v) = self.vignettes.remove(&page) else {
            return;
        };
        if let (Some(fil), Some((jeton, _))) = (self.fil.as_mut(), v.en_cours) {
            fil.annuler(jeton);
        }
        self.a_jeter
            .borrow_mut()
            .extend(v.image.map(|(image, _)| image));
    }

    /// Demande un rendu pour chaque vignette visible dont l'image n'a pas la taille physique de
    /// sa boîte mesurée. Rien pendant le glissement de la poignée : les vignettes sont étirées,
    /// puis repeintes d'un coup au relâchement (comme `up` côté TS).
    fn reconcilier(&mut self, cx: &mut Context<Self>) {
        if self.modele.en_glissement() {
            return;
        }
        let mut soumis = false;
        for page in self.modele.vignettes_peintes().to_vec() {
            let Some(mesure) = self.livrees.get(&page).copied() else {
                continue;
            };
            let v = self.vignettes.entry(page).or_default();
            let a_jour = v.image.as_ref().is_some_and(|(_, p)| *p == mesure);
            let vise = v.en_cours.is_some_and(|(_, p)| p == mesure);
            if a_jour || vise {
                continue;
            }
            let Some(fil) = self.fil.as_mut() else {
                continue;
            };
            if let Some((ancien, _)) = v.en_cours.take() {
                fil.annuler(ancien);
            }
            self.prochain_jeton += 1;
            let jeton = self.prochain_jeton;
            v.en_cours = Some((jeton, mesure));
            fil.soumettre(Demande {
                page,
                jeton,
                pixels: mesure,
            });
            soumis = true;
        }
        if soumis {
            self.lancer_recolte(cx);
        }
    }

    /// Après chaque image : la zone de contenu, puis les mesures des boîtes de vignettes.
    pub(super) fn apres_image(&mut self, zone: Bounds<PixelsGpui>, cx: &mut Context<Self>) {
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let largeur = f64::from(f32::from(zone.size.width)) / zoom;
        let hauteur = f64::from(f32::from(zone.size.height)) / zoom;
        let mut change = false;
        if (largeur, hauteur) != self.contenu {
            self.contenu = (largeur, hauteur);
            let plan = self.modele.definir_hauteur(hauteur);
            self.appliquer_plan(plan, cx);
            change = true;
        }
        let mesures: Vec<(usize, Pixels)> = self.mesures_image.borrow_mut().drain(..).collect();
        for (page, p) in mesures {
            // Une vignette qui n'est plus voulue (sortie depuis) n'a plus de canvas à servir.
            if self.modele.vignettes_peintes().contains(&page)
                && self.livrees.insert(page, p) != Some(p)
            {
                change = true;
            }
        }
        self.reconcilier(cx);
        if change {
            cx.notify();
        }
    }

    // ── Récolte des rendus ───────────────────────────────────────────────

    fn lancer_recolte(&mut self, cx: &mut Context<Self>) {
        if !self.rendus_en_cours() || self.recolte_active {
            return;
        }
        self.recolte_active = true;
        self.recolte = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(RECOLTE_MS))
                    .await;
                let encore = this.update(cx, |v, cx| v.recolter(cx)).unwrap_or(false);
                if !encore {
                    break;
                }
            }
        }));
    }

    /// Prend les réponses du fil. Rend vrai s'il faut continuer à récolter.
    fn recolter(&mut self, cx: &mut Context<Self>) -> bool {
        let reponses = self
            .fil
            .as_mut()
            .map(FilDeRendu::recolter)
            .unwrap_or_default();
        let mut change = false;
        for r in reponses {
            match r {
                Reponse::Ouvert(Ok(tailles)) => {
                    self.attend_ouverture = false;
                    self.ouvrir(&tailles, cx);
                    change = true;
                }
                Reponse::Ouvert(Err(e)) => {
                    self.attend_ouverture = false;
                    self.erreur = Some(e);
                    change = true;
                }
                Reponse::Rendu {
                    page,
                    jeton,
                    resultat,
                } => {
                    let Some(v) = self.vignettes.get_mut(&page) else {
                        continue;
                    };
                    if v.en_cours.is_none_or(|(j, _)| j != jeton) {
                        continue;
                    }
                    v.en_cours = None;
                    if let Ok((image, pixels)) = resultat {
                        let perimee = v.image.replace((image, pixels));
                        self.a_jeter
                            .borrow_mut()
                            .extend(perimee.map(|(image, _)| image));
                    }
                    change = true;
                }
            }
        }
        if change {
            // Une image reçue à une taille périmée (la boîte a changé depuis) est refaite.
            self.reconcilier(cx);
            cx.notify();
        }
        let encore = self.rendus_en_cours();
        self.recolte_active = encore;
        encore
    }

    /// Le document est ouvert : vignettes et arbre. Les pages qui n'ont pas le format de la
    /// page 1 corrigent leur vignette tout de suite (pdfium donne toutes les tailles sans rendre).
    fn ouvrir(&mut self, tailles: &[fragment_pdf::document::TaillePage], cx: &mut Context<Self>) {
        let Some(premiere) = tailles.first() else {
            return;
        };
        let ratio = |t: &fragment_pdf::document::TaillePage| {
            if t.largeur > 0.0 {
                t.hauteur / t.largeur
            } else {
                1.0
            }
        };
        let toc = std::mem::take(&mut self.toc_en_attente);
        let ratio1 = ratio(premiere);
        self.ratios = tailles.iter().map(ratio).collect();
        let n = tailles.len();
        self.agir(cx, |m| {
            let mut plan = m.charger(n, ratio1, toc);
            for (i, t) in tailles.iter().enumerate().skip(1) {
                if ratio(t) != ratio1 {
                    let suite = m.format_page_connu(i + 1, t.largeur, t.hauteur);
                    plan.a_liberer.extend(suite.a_liberer);
                    plan.a_peindre.extend(suite.a_peindre);
                }
            }
            plan
        });
    }
}
