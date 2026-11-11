// Port de app/src/ui/views/pdf/PdfSidebar.ts
//
// Modèle PUR de la barre latérale de la vue PDF : vignettes OU table des matières,
// largeur bornée, virtualisation des vignettes. Aucun DOM, aucun moteur : la vue
// montée (fr-t4-laterale-ui) mesure, peint et applique les plans rendus ici.

mod barre;
mod toc;

pub use barre::{
    BarreLaterale, EtatBarre, LARGEUR_DEFAUT, LARGEUR_MAX, LARGEUR_MIN, ModeBarre, PlanVignettes,
    RATIO_MAX_VUE,
};
pub use toc::{ArbreToc, Chemin, Destination, LigneToc, MESSAGE_SANS_TABLE};
