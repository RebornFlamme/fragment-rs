// Port de app/src/ui/views/agent/controls/{controls.ts,controlsModel.ts} et
// app/src/ui/views/agent/policy/permissionCard.ts
//! Contrôles du champ de l'agent et carte de permission (tâche `fr-t7-controles`).
//!
//! - [`modele`] : les listes (moteur, modèle, effort, mode), l'anneau d'usage et les emplacements
//!   `chat:composer-tools@1` qui les accueillent — purs, lus d'une [`SourceControles`] ;
//! - [`contexte`] : la puce de CONTEXTE (extension `agent-context` du champ, barre seulement) ;
//! - [`carte`] : le part `agent-permission` — ce que dit la carte, ses boutons, la décision
//!   renvoyée par un rappel (le fournisseur est branché au montage) ;
//! - [`vue`] : le rendu GPUI de tout cela, branché dans le registre du fil.

pub mod carte;
pub mod contexte;
pub mod modele;
pub mod vue;

pub use carte::{
    ActionCarte, BoutonCarte, CarteModele, MESSAGE_TOUR_ARRETE, PART_PERMISSION, VarianteBouton,
    refus_a_larret, repondre,
};
pub use contexte::{
    DocumentVu, EXTENSION_CONTEXTE, ExtensionContexte, PuceContexte, VuePuce, contexte_vers_options,
};
pub use modele::{
    Controle, Controles, EntreeListe, EtatAnneau, EtatListe, FOURNISSEUR_AGENT, MoteurPropose,
    NiveauUsage, ProfilConversation, SourceControles, mode_depuis_id, mode_id, slots_controles,
};
pub use vue::{
    CarteVue, PartPermission, Rafraichir, RenduPermission, RenduPlan, Reponse, commandes_de_scene,
    slots_gpui, zone_contexte,
};
