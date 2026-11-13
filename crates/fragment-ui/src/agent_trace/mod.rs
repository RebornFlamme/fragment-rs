// Port de app/src/ui/views/agent/trace/{traceView.ts,trace.css}
//! Part agent-trace : repliée par défaut, étapes, pensée, chrono, résumé, gestes (tâche
//! `fr-t7-trace`).
//!
//! - [`animation`] : les gestes en machines d'états pures (fondu croisé, chrono qui roule,
//!   zones repliables, mode nerveux d'un clic) ;
//! - [`poignee`] : la poignée du part (état vivant, mis à jour à chaque version du part) et son
//!   instantané [`VueTrace`] ;
//! - [`vue`] : la vue GPUI ([`TraceView`]) qui peint l'instantané, et l'emplacement de l'orbe
//!   ([`FournisseurOrbe`], rempli par `fr-t7-orbe`).

pub mod animation;
pub mod poignee;
pub mod vue;

pub use poignee::{
    CibleVue, EtapeVue, EtatEtape, EtatPartage, EtatUi, EtatsTrace, Marque, OrbeChoix, Poignee,
    VueTrace, extrait, nom_orbe,
};
pub use vue::{FournisseurOrbe, TraceView, commandes_de_scene};
