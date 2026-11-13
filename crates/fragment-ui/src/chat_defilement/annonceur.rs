// Port de app/src/ui/views/chat/scroll/announcer.ts (et `.chat-announcer`, scroll.css).
//! L'ANNONCEUR (lecteurs d'écran) : le fil n'est pas une région vive (chaque delta serait lu) ;
//! une région à part, masquée, dit les ÉVÉNEMENTS — « Claude répond », « Réponse terminée »,
//! « Erreur : … » — au plus un toutes les [`ECART_ANNONCES_MS`], le dernier remplaçant ceux qui
//! attendent (on dit l'état, pas chaque étape).

use std::time::{Duration, Instant};

use fragment_chat::follow_model::announce_at;
use gpui_kit::{
    Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, px,
};

/// L'écart minimal entre deux annonces (ms).
pub const ECART_ANNONCES_MS: f64 = 2000.0;

/// L'identifiant de la région.
pub const ID_ANNONCEUR: &str = "chat-announcer";

/// Le calendrier PUR de l'annonceur ; le temps en ms.
#[derive(Clone, Debug, PartialEq)]
pub struct Annonces {
    dernier: f64,
    en_attente: Option<String>,
    /// Une annonce est programmée à cet instant.
    echeance: Option<f64>,
    texte: String,
}

impl Default for Annonces {
    fn default() -> Self {
        Annonces {
            dernier: f64::NEG_INFINITY,
            en_attente: None,
            echeance: None,
            texte: String::new(),
        }
    }
}

impl Annonces {
    /// Le texte annoncé (le contenu de la région).
    #[must_use]
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Dire `texte` : rend le délai (ms) d'un minuteur à armer, ou `None` si un minuteur attend
    /// déjà (le texte remplace alors celui qui attendait).
    pub fn dire(&mut self, texte: &str, maintenant: f64) -> Option<f64> {
        self.en_attente = Some(texte.to_owned());
        if self.echeance.is_some() {
            return None;
        }
        let quand = announce_at(maintenant, self.dernier, ECART_ANNONCES_MS);
        self.echeance = Some(quand);
        Some((quand - maintenant).max(0.0))
    }

    /// Le minuteur a sonné : le texte en attente devient l'annonce. Vrai si elle a changé.
    pub fn echoir(&mut self, maintenant: f64) -> bool {
        self.echeance = None;
        let Some(t) = self.en_attente.take() else {
            return false;
        };
        self.texte = t;
        self.dernier = maintenant;
        true
    }
}

/// La région annoncée (une entité) : `role=status`, masquée (1 × 1 px, rognée).
pub struct Annonceur {
    annonces: Annonces,
    origine: Instant,
    minuteur: Option<Task<()>>,
}

impl Annonceur {
    #[must_use]
    pub fn new(cx: &Context<Self>) -> Self {
        Annonceur {
            annonces: Annonces::default(),
            origine: cx.background_executor().now(),
            minuteur: None,
        }
    }

    fn maintenant(&self, cx: &Context<Self>) -> f64 {
        cx.background_executor()
            .now()
            .saturating_duration_since(self.origine)
            .as_secs_f64()
            * 1000.0
    }

    /// Le texte annoncé.
    #[must_use]
    pub fn texte(&self) -> &str {
        self.annonces.texte()
    }

    /// Dire un événement (au plus un toutes les 2 s, le dernier l'emporte).
    pub fn dire(&mut self, texte: &str, cx: &mut Context<Self>) {
        let maintenant = self.maintenant(cx);
        let Some(delai) = self.annonces.dire(texte, maintenant) else {
            return;
        };
        // Le temps de l'exécuteur : virtuel en test (`advance_clock`).
        let attente = cx
            .background_executor()
            .timer(Duration::from_secs_f64(delai / 1000.0));
        self.minuteur = Some(cx.spawn(async move |this, cx| {
            attente.await;
            let _ = this.update(cx, |a, cx| {
                let maintenant = a.maintenant(cx);
                if a.annonces.echoir(maintenant) {
                    cx.notify();
                }
            });
        }));
    }
}

impl Render for Annonceur {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(ElementId::from(ID_ANNONCEUR))
            .role(Role::Status)
            .absolute()
            .w(px(1.0))
            .h(px(1.0))
            .overflow_hidden()
            .child(self.annonces.texte().to_owned())
    }
}

/// UNE mise à jour du fil par image : les changements de la session se regroupent ; le premier
/// programme la passe (à l'image suivante, repli minuteur [`REPLI_IMAGE_MS`] pour une fenêtre sans
/// images), les suivants n'ajoutent rien.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UneParImage {
    planifie: bool,
}

/// Le repli quand aucune image ne vient (fenêtre en arrière-plan), ms.
pub const REPLI_IMAGE_MS: u64 = 100;

impl UneParImage {
    /// Un changement : vrai s'il faut programmer la passe.
    pub fn demander(&mut self) -> bool {
        !std::mem::replace(&mut self.planifie, true)
    }

    /// L'image (ou le repli) est venue : vrai s'il faut faire la passe (une seule fois).
    pub fn executer(&mut self) -> bool {
        std::mem::take(&mut self.planifie)
    }
}
