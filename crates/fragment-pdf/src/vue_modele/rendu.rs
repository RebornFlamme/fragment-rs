// Port de app/src/ui/views/pdf/PdfView.ts (emplacements, peindre, liberer, repeindrePage)
//
// Ce que le modèle dit à la vue : quelles pages peindre, lesquelles libérer.

/// Où en est une page dans la virtualisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EtatPage {
    /// Une boîte vide de la bonne taille : rien en mémoire.
    #[default]
    Vide,
    /// Un rendu est demandé. `ancienne` : l'image périmée (avant zoom) reste affichée dessous.
    EnRendu { ancienne: bool },
    /// Le canvas est peint à l'échelle courante.
    Prete,
}

/// Une page à (re)peindre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peinture {
    /// Numéro de page, à partir de 1.
    pub page: usize,
    /// Vrai si la page avait déjà une image : on la garde étirée sous la nouvelle,
    /// jusqu'à ce que celle-ci soit prête (pas de flash blanc).
    pub garder_ancienne: bool,
}

/// L'ordre de travail rendu à la vue après un événement.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanRendu {
    /// Pages à peindre, par numéro croissant.
    pub peindre: Vec<Peinture>,
    /// Pages à libérer (rendu annulé, canvas jeté, boîte conservée), numéros à partir de 1.
    pub liberer: Vec<usize>,
    /// Les calques doivent relire la géométrie (pages dessinées ou déplacées).
    pub notifier: bool,
    /// Un geste de zoom différé est en cours : la vue marque la zone « en zoom »
    /// (canvas étirés, aucun repeint) jusqu'à la fin du geste.
    pub en_zoom: bool,
}

impl PlanRendu {
    pub fn vide() -> Self {
        Self::default()
    }

    /// Rien à faire (ni rendu, ni notification).
    pub fn est_vide(&self) -> bool {
        self.peindre.is_empty() && self.liberer.is_empty() && !self.notifier
    }

    /// Les numéros de page à peindre.
    pub fn pages_a_peindre(&self) -> Vec<usize> {
        self.peindre.iter().map(|p| p.page).collect()
    }

    /// Ajoute `autre` à la suite. Une page à peindre dans `autre` annule sa libération
    /// antérieure et inversement, pour que le plan reste cohérent.
    pub fn fusionner(&mut self, autre: PlanRendu) {
        for p in autre.peindre {
            self.liberer.retain(|l| *l != p.page);
            match self.peindre.iter_mut().find(|q| q.page == p.page) {
                Some(q) => q.garder_ancienne |= p.garder_ancienne,
                None => self.peindre.push(p),
            }
        }
        for l in autre.liberer {
            self.peindre.retain(|q| q.page != l);
            if !self.liberer.contains(&l) {
                self.liberer.push(l);
            }
        }
        self.peindre.sort_by_key(|p| p.page);
        self.liberer.sort_unstable();
        self.notifier |= autre.notifier;
        self.en_zoom = autre.en_zoom;
    }
}
