// Port de app/src/ui/core/workspace/ItemView.ts (reprendre : la reprise réessayée).
//! La reprise réessayée : rejouer un essai à chaque image jusqu'à ce qu'il réussisse.
//!
//! ★ Le contenu arrive tard (messages d'une conversation relus, page dont la hauteur se
//!   connaît après coup) : poser une fois ne suffit pas. On réessaie à chaque image, au plus
//!   10 s, et on LÂCHE au premier geste de l'utilisateur (molette, pointeur, clavier) : sa
//!   lecture prime sur la nôtre.

/// Le temps laissé à une reprise.
pub const DELAI_REPRISE_MS: u64 = 10_000;

/// Comment une reprise s'est terminée (ou non).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueReprise {
    /// L'essai n'a pas encore réussi : on rejoue à la prochaine image.
    EnCours,
    /// L'essai a réussi.
    Reussie,
    /// L'utilisateur a pris la main.
    Abandonnee,
    /// Dix secondes sans succès.
    Expiree,
}

impl IssueReprise {
    /// La reprise est-elle terminée ?
    pub fn est_finie(self) -> bool {
        self != Self::EnCours
    }
}

/// Une reprise : l'essai, son échéance, son issue.
pub struct Reprise {
    essai: Box<dyn FnMut() -> bool>,
    echeance_ms: u64,
    issue: IssueReprise,
}

impl Reprise {
    /// Lance la reprise à `maintenant_ms` : le PREMIER essai a lieu tout de suite.
    pub fn demarrer(maintenant_ms: u64, mut essai: Box<dyn FnMut() -> bool>) -> Self {
        let issue = if essai() {
            IssueReprise::Reussie
        } else {
            IssueReprise::EnCours
        };
        Self {
            essai,
            echeance_ms: maintenant_ms + DELAI_REPRISE_MS,
            issue,
        }
    }

    /// Une image : rejoue l'essai si la reprise court encore.
    pub fn image(&mut self, maintenant_ms: u64) -> IssueReprise {
        if self.issue != IssueReprise::EnCours {
            return self.issue;
        }
        if (self.essai)() {
            self.issue = IssueReprise::Reussie;
        } else if maintenant_ms > self.echeance_ms {
            self.issue = IssueReprise::Expiree;
        }
        self.issue
    }

    /// Un geste de l'utilisateur : la reprise est abandonnée (sans effet si elle est finie).
    pub fn geste(&mut self) {
        if self.issue == IssueReprise::EnCours {
            self.issue = IssueReprise::Abandonnee;
        }
    }

    /// L'issue actuelle.
    pub fn issue(&self) -> IssueReprise {
        self.issue
    }
}
