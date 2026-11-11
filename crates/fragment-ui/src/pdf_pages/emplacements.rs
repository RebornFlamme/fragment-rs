// Port de app/src/ui/views/pdf/PdfView.ts (Emplacement, peindre, liberer, repeindrePage) et de
// app/src/ui/views/pdf/PdfEditor.ts (renderedRanges)
//
// Les images des pages, page par page, sans GPUI : ce que le plan du modèle (`PlanRendu`) fait
// aux images (en demander une, garder l'ancienne dessous, jeter) et ce que devient un rendu
// qui revient. Générique sur le type d'image, pour se tester sans fenêtre.
//
// Une demande de rendu naît SANS taille : elle attend la mesure physique de sa boîte (l'image
// suivante), comme le canvas de Fragment attendait son `ResizeObserver`. On ne devine pas
// l'accrochage à la grille de l'écran, on le lit.

use fragment_pdf::pdf_editor::{PAGE_STRIDE, pos_de_page};
use fragment_pdf::vue_modele::{Pixels, PlanRendu};
use fragment_text::EditorRange;

/// Une image de page et sa taille en pixels physiques.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagePage<I> {
    pub image: I,
    pub pixels: Pixels,
}

/// Une demande de rendu pour le fil : la page (à partir de 1), son jeton, sa taille physique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demande {
    pub page: usize,
    pub jeton: u64,
    pub pixels: Pixels,
}

/// Le rendu en cours d'une page. `pixels` vaut `None` tant que la boîte n'est pas mesurée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnCours {
    jeton: u64,
    pixels: Option<Pixels>,
}

#[derive(Debug, Clone)]
struct Emplacement<I> {
    /// L'image à l'échelle courante (absente pendant un rendu : rien ne s'affiche de neuf).
    courante: Option<ImagePage<I>>,
    /// L'image d'avant un repeint, étirée dessous jusqu'à ce que la nouvelle soit prête.
    perimee: Option<ImagePage<I>>,
    en_cours: Option<EnCours>,
}

impl<I> Default for Emplacement<I> {
    fn default() -> Self {
        Self {
            courante: None,
            perimee: None,
            en_cours: None,
        }
    }
}

impl<I> Emplacement<I> {
    /// Quelque chose est dessiné ou va l'être : la page compte comme peinte.
    fn occupe(&self) -> bool {
        self.courante.is_some() || self.perimee.is_some() || self.en_cours.is_some()
    }
}

/// Ce que la vue doit faire après un plan : rendus à lancer, à annuler, images à jeter (de
/// l'atlas de GPUI).
#[derive(Debug)]
pub struct Travail<I> {
    pub demandes: Vec<Demande>,
    pub annulees: Vec<u64>,
    pub jetees: Vec<I>,
}

impl<I> Default for Travail<I> {
    fn default() -> Self {
        Self {
            demandes: Vec::new(),
            annulees: Vec::new(),
            jetees: Vec::new(),
        }
    }
}

/// Ce que la vue voit d'une page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EtatImages {
    /// Taille de l'image courante (prête), s'il y en a une.
    pub courante: Option<Pixels>,
    /// Taille de l'image périmée gardée dessous, s'il y en a une.
    pub perimee: Option<Pixels>,
    /// Un rendu est demandé (ou attend sa mesure).
    pub en_cours: bool,
}

/// Les images de toutes les pages du document ouvert.
#[derive(Debug)]
pub struct Emplacements<I> {
    pages: Vec<Emplacement<I>>,
    prochain_jeton: u64,
}

impl<I> Default for Emplacements<I> {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            prochain_jeton: 1,
        }
    }
}

impl<I> Emplacements<I> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn nombre(&self) -> usize {
        self.pages.len()
    }

    /// Un autre document (ou aucun) : `n` emplacements vides. Rend le travail d'annulation.
    pub fn reinitialiser(&mut self, n: usize) -> Travail<I> {
        let mut travail = Travail::default();
        for i in 0..self.pages.len() {
            self.vider(i, &mut travail);
        }
        self.pages = (0..n).map(|_| Emplacement::default()).collect();
        travail
    }

    fn vider(&mut self, i: usize, travail: &mut Travail<I>) {
        let Some(e) = self.pages.get_mut(i) else {
            return;
        };
        if let Some(c) = e.en_cours.take() {
            travail.annulees.push(c.jeton);
        }
        travail
            .jetees
            .extend(e.courante.take().into_iter().map(|c| c.image));
        travail
            .jetees
            .extend(e.perimee.take().into_iter().map(|c| c.image));
    }

    /// Applique un plan du modèle : libère, puis (re)demande. Une page repeinte garde
    /// son image (la courante, sinon la périmée d'un repeint encore en cours) dessous
    /// quand le plan le dit ; sinon tout est jeté. Les demandes attendent leur mesure.
    pub fn appliquer(&mut self, plan: &PlanRendu) -> Travail<I> {
        let mut travail = Travail::default();
        for &page in &plan.liberer {
            if let Some(i) = page.checked_sub(1) {
                self.vider(i, &mut travail);
            }
        }
        for p in &plan.peindre {
            let Some(i) = p.page.checked_sub(1) else {
                continue;
            };
            let jeton = self.prochain_jeton;
            let Some(e) = self.pages.get_mut(i) else {
                continue;
            };
            self.prochain_jeton += 1;
            if let Some(c) = e.en_cours.take() {
                travail.annulees.push(c.jeton);
            }
            // L'image à garder : la courante si elle est peinte, sinon la périmée d'avant.
            let ancienne = match e.courante.take() {
                Some(c) => {
                    travail
                        .jetees
                        .extend(e.perimee.take().into_iter().map(|x| x.image));
                    Some(c)
                }
                None => e.perimee.take(),
            };
            match ancienne {
                Some(a) if p.garder_ancienne => e.perimee = Some(a),
                Some(a) => travail.jetees.push(a.image),
                None => {}
            }
            e.en_cours = Some(EnCours {
                jeton,
                pixels: None,
            });
        }
        travail
    }

    /// La boîte de `page` mesure `pixels` (pixels physiques où elle sera peinte). Si sa
    /// demande attendait cette mesure, elle part.
    pub fn fournir_mesure(&mut self, page: usize, pixels: Pixels) -> Option<Demande> {
        let e = self.pages.get_mut(page.checked_sub(1)?)?;
        let c = e.en_cours.as_mut()?;
        if c.pixels.is_some() {
            return None;
        }
        c.pixels = Some(pixels);
        Some(Demande {
            page,
            jeton: c.jeton,
            pixels,
        })
    }

    /// Un rendu revient. S'il est encore attendu (même jeton), son image remplace
    /// l'ancienne, rend sa taille (à signaler au modèle) et l'image périmée à jeter.
    /// `image` vaut `None` si le rendu a échoué : la page reste blanche, mais elle
    /// n'est plus en attente. Un rendu périmé est refusé : `Err(image)`.
    pub fn recevoir(
        &mut self,
        page: usize,
        jeton: u64,
        image: Option<ImagePage<I>>,
    ) -> Result<(Pixels, Option<I>), Option<ImagePage<I>>> {
        let Some(e) = page.checked_sub(1).and_then(|i| self.pages.get_mut(i)) else {
            return Err(image);
        };
        let attendu = matches!(e.en_cours, Some(c) if c.jeton == jeton && c.pixels.is_some());
        if !attendu {
            return Err(image);
        }
        let demande = e.en_cours.take().and_then(|c| c.pixels);
        let pixels = image
            .as_ref()
            .map(|i| i.pixels)
            .or(demande)
            .unwrap_or(Pixels { w: 0, h: 0 });
        let perimee = e.perimee.take().map(|p| p.image);
        e.courante = image;
        Ok((pixels, perimee))
    }

    /// Un rendu au moins est demandé ou en vol.
    pub fn en_cours(&self) -> bool {
        self.pages.iter().any(|e| e.en_cours.is_some())
    }

    /// Les pages occupées (image, image périmée ou rendu), à partir de 1.
    pub fn occupees(&self) -> Vec<usize> {
        self.pages
            .iter()
            .enumerate()
            .filter(|(_, e)| e.occupe())
            .map(|(i, _)| i + 1)
            .collect()
    }

    pub fn etat(&self, page: usize) -> EtatImages {
        page.checked_sub(1)
            .and_then(|i| self.pages.get(i))
            .map(|e| EtatImages {
                courante: e.courante.as_ref().map(|c| c.pixels),
                perimee: e.perimee.as_ref().map(|c| c.pixels),
                en_cours: e.en_cours.is_some(),
            })
            .unwrap_or_default()
    }

    /// Les images à peindre pour `page`, de dessous à dessus : (périmée, courante).
    pub fn images(&self, page: usize) -> (Option<&ImagePage<I>>, Option<&ImagePage<I>>) {
        match page.checked_sub(1).and_then(|i| self.pages.get(i)) {
            Some(e) => (e.perimee.as_ref(), e.courante.as_ref()),
            None => (None, None),
        }
    }

    /// Les plages de positions DESSINÉES (`renderedRanges`) : une page compte dès que son
    /// rendu est demandé, et pendant un repeint (son ancienne image reste affichée), sinon
    /// ce qui y est ancré disparaîtrait à chaque zoom. Une page libérée ne compte pas : ce
    /// qui y est ancré est masqué, comme un texte replié.
    pub fn plages_dessinees(&self) -> Vec<EditorRange> {
        let mut plages = Vec::new();
        let mut debut: Option<usize> = None;
        for i in 0..=self.pages.len() {
            let peinte = self.pages.get(i).is_some_and(Emplacement::occupe);
            match (peinte, debut) {
                (true, None) => debut = Some(i),
                (false, Some(d)) => {
                    // Pages d+1 ..= i (numéros) : de l'ancre de la première au dernier offset
                    // de la dernière.
                    let from = pos_de_page(d as i64 + 1);
                    let to = pos_de_page(i as i64) + PAGE_STRIDE - 1;
                    plages.push(EditorRange {
                        from: from.max(0) as usize,
                        to: to.max(0) as usize,
                    });
                    debut = None;
                }
                _ => {}
            }
        }
        plages
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use fragment_pdf::vue_modele::Peinture;

    use super::*;

    fn px(w: u32, h: u32) -> Pixels {
        Pixels { w, h }
    }

    fn plan(peindre: &[(usize, bool)], liberer: &[usize]) -> PlanRendu {
        PlanRendu {
            peindre: peindre
                .iter()
                .map(|&(page, garder_ancienne)| Peinture {
                    page,
                    garder_ancienne,
                })
                .collect(),
            liberer: liberer.to_vec(),
            notifier: true,
            en_zoom: false,
        }
    }

    /// Peint `page` jusqu'au bout avec l'image `nom`.
    fn peindre(e: &mut Emplacements<&'static str>, page: usize, nom: &'static str, t: Pixels) {
        let travail = e.appliquer(&plan(&[(page, false)], &[]));
        assert!(travail.demandes.is_empty(), "une demande attend sa mesure");
        let d = e.fournir_mesure(page, t).expect("demande partie");
        assert!(
            e.recevoir(
                page,
                d.jeton,
                Some(ImagePage {
                    image: nom,
                    pixels: t
                })
            )
            .is_ok()
        );
    }

    #[test]
    fn une_demande_attend_la_mesure_de_sa_boite_puis_part_une_seule_fois() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(3);
        let t = e.appliquer(&plan(&[(2, false)], &[]));
        assert!(t.demandes.is_empty());
        assert!(e.etat(2).en_cours);
        let d = e.fournir_mesure(2, px(612, 792)).unwrap();
        assert_eq!((d.page, d.pixels), (2, px(612, 792)));
        assert_eq!(e.fournir_mesure(2, px(613, 792)), None, "déjà partie");
        assert_eq!(e.fournir_mesure(1, px(1, 1)), None, "rien d'attendu");
    }

    #[test]
    fn repeint_garde_l_ancienne_image_dessous_jusqu_a_la_nouvelle() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(1);
        peindre(&mut e, 1, "a", px(100, 120));
        let t = e.appliquer(&plan(&[(1, true)], &[]));
        assert!(t.jetees.is_empty());
        assert_eq!(
            e.etat(1),
            EtatImages {
                courante: None,
                perimee: Some(px(100, 120)),
                en_cours: true
            }
        );
        // Un second repeint pendant le premier garde la MÊME image périmée et annule le rendu.
        let d1 = e.fournir_mesure(1, px(150, 180)).unwrap();
        let t = e.appliquer(&plan(&[(1, true)], &[]));
        assert_eq!(t.annulees, vec![d1.jeton]);
        assert_eq!(e.etat(1).perimee, Some(px(100, 120)));
        // Le rendu annulé revient quand même : refusé.
        assert!(
            e.recevoir(
                1,
                d1.jeton,
                Some(ImagePage {
                    image: "b",
                    pixels: px(150, 180)
                })
            )
            .is_err()
        );
        let d2 = e.fournir_mesure(1, px(200, 240)).unwrap();
        let (taille, jetee) = e
            .recevoir(
                1,
                d2.jeton,
                Some(ImagePage {
                    image: "c",
                    pixels: px(200, 240),
                }),
            )
            .unwrap();
        assert_eq!((taille, jetee), (px(200, 240), Some("a")));
        assert_eq!(
            e.etat(1),
            EtatImages {
                courante: Some(px(200, 240)),
                perimee: None,
                en_cours: false
            }
        );
    }

    #[test]
    fn sans_garder_l_ancienne_image_est_jetee() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(1);
        peindre(&mut e, 1, "a", px(10, 10));
        let t = e.appliquer(&plan(&[(1, false)], &[]));
        assert_eq!(t.jetees, vec!["a"]);
        assert_eq!(e.etat(1).perimee, None);
    }

    #[test]
    fn liberer_annule_le_rendu_et_jette_les_images() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(2);
        peindre(&mut e, 1, "a", px(10, 10));
        let _ = e.appliquer(&plan(&[(1, true)], &[]));
        let d = e.fournir_mesure(1, px(20, 20)).unwrap();
        let t = e.appliquer(&plan(&[], &[1]));
        assert_eq!(t.annulees, vec![d.jeton]);
        assert_eq!(t.jetees, vec!["a"]);
        assert_eq!(e.etat(1), EtatImages::default());
        assert!(!e.en_cours());
    }

    #[test]
    fn un_rendu_rate_libere_l_attente_sans_image() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(1);
        let _ = e.appliquer(&plan(&[(1, false)], &[]));
        let d = e.fournir_mesure(1, px(30, 40)).unwrap();
        let (taille, _) = e.recevoir(1, d.jeton, None).unwrap();
        assert_eq!(taille, px(30, 40));
        assert_eq!(e.etat(1), EtatImages::default());
    }

    #[test]
    fn rendered_ranges_une_page_en_repeint_compte_comme_dessinee() {
        let mut e: Emplacements<&str> = Emplacements::new();
        let _ = e.reinitialiser(5);
        peindre(&mut e, 2, "b", px(1, 1));
        peindre(&mut e, 3, "c", px(1, 1));
        // Page 3 en repeint (image périmée), page 5 en rendu : toutes deux comptent.
        let _ = e.appliquer(&plan(&[(3, true), (5, false)], &[]));
        let s = PAGE_STRIDE as usize;
        assert_eq!(
            e.plages_dessinees(),
            vec![
                EditorRange {
                    from: s,
                    to: 3 * s - 1
                },
                EditorRange {
                    from: 4 * s,
                    to: 5 * s - 1
                },
            ]
        );
        let _ = e.appliquer(&plan(&[], &[2, 3, 5]));
        assert!(e.plages_dessinees().is_empty());
    }
}
