// Port de app/src/ui/views/agent/policy/permissionCard.ts (la carte : ses états, ses boutons, sa décision)
//! La carte d'autorisation du part `agent-permission` — ce qu'elle montre et ce qu'elle renvoie.
//!
//! Trois états :
//! - EN ATTENTE : l'icône de l'outil et un verbe, la commande ou le chemin en bloc, la raison du
//!   moteur ; Autoriser une fois, Toujours dans cette conversation, Toujours dans ce coffre (si la
//!   demande offre une règle), Refuser, et un « pourquoi » facultatif que l'agent lira ;
//! - PLAN (mode plan) : le plan et deux boutons, Lancer et Modifier ;
//! - RÉPONDUE : une ligne repliée (« Autorisé : git status »), gardée dans l'historique du tour.
//!
//! Le part ne porte que des DONNÉES ([`PermissionCardData`]) ; la réponse sort par un rappel que
//! le fournisseur branche au montage, et c'est lui qui tient la demande en attente (une vue
//! reconstruite la retrouve).

use fragment_agent::contrat::{PermissionDecision, Remember, RememberScope};
use fragment_agent::permission_model::{
    DescriptionPermission, EtatCarte, PermissionCardData, answer_note, describe_permission,
};
use fragment_chat::api::ChatPart;

/// Le type du part (`chat:part@1`).
pub const PART_PERMISSION: &str = "agent-permission";

/// Ce que l'agent lit quand un tour est arrêté pendant qu'une carte attend.
pub const MESSAGE_TOUR_ARRETE: &str = "Tour arrêté.";

/// Ce que l'agent lit d'un plan renvoyé sans commentaire.
const PLAN_A_MODIFIER: &str = "Modifie le plan avant de le lancer.";

/// L'aspect d'un bouton (`mod-cta`, `mod-deny`, neutre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarianteBouton {
    Principal,
    Normal,
    Refus,
}

/// Ce qu'un bouton fait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCarte {
    AutoriserUneFois,
    ToujoursConversation,
    ToujoursCoffre,
    Refuser,
    /// Mode plan : valider le plan.
    Lancer,
    /// Mode plan : le renvoyer avec ce qu'il faut changer.
    Modifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoutonCarte {
    pub action: ActionCarte,
    pub libelle: &'static str,
    pub variante: VarianteBouton,
}

fn bouton(action: ActionCarte, libelle: &'static str, variante: VarianteBouton) -> BoutonCarte {
    BoutonCarte {
        action,
        libelle,
        variante,
    }
}

/// La marque d'une carte répondue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarqueReponse {
    Coche,
    Croix,
}

/// Une carte répondue, repliée en une ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneRepondue {
    pub marque: MarqueReponse,
    pub texte: String,
}

/// L'état d'une carte : ses données, ce qu'elle en dit, et le « pourquoi » en cours de saisie.
#[derive(Debug, Clone, PartialEq)]
pub struct CarteModele {
    pub data: PermissionCardData,
    pub description: DescriptionPermission,
    /// Le texte du champ « pourquoi » (vide tant que l'utilisateur n'a rien écrit).
    pub pourquoi: String,
}

impl CarteModele {
    #[must_use]
    pub fn new(data: PermissionCardData) -> Self {
        let description = describe_permission(&data.request);
        Self {
            data,
            description,
            pourquoi: String::new(),
        }
    }

    /// Les données d'un part `agent-permission`, ou `None` si elles sont illisibles.
    #[must_use]
    pub fn depuis_part(part: &ChatPart) -> Option<Self> {
        let valeur = part.data.clone()?;
        serde_json::from_value::<PermissionCardData>(valeur)
            .ok()
            .map(Self::new)
    }

    /// Ce qui fait reconstruire la carte : l'état, la demande et la note. Une nouvelle version du
    /// part qui n'y change rien laisse la carte (et le « pourquoi » tapé) telle quelle.
    #[must_use]
    pub fn signature(&self) -> (EtatCarte, &str, Option<&str>) {
        (
            self.data.state,
            self.data.id.as_str(),
            self.data.note.as_deref(),
        )
    }

    /// Une nouvelle version du part. `true` : la carte a changé (le « pourquoi » repart de zéro).
    pub fn mettre_a_jour(&mut self, data: PermissionCardData) -> bool {
        let avant = (
            self.data.state,
            self.data.id.clone(),
            self.data.note.clone(),
        );
        let apres = (data.state, data.id.clone(), data.note.clone());
        if avant == apres {
            return false;
        }
        *self = Self::new(data);
        true
    }

    /// Une demande de validation de plan ?
    #[must_use]
    pub fn est_plan(&self) -> bool {
        self.data.request.plan.is_some()
    }

    #[must_use]
    pub fn en_attente(&self) -> bool {
        self.data.state == EtatCarte::Pending
    }

    /// La ligne d'une carte répondue (`None` tant qu'elle attend).
    #[must_use]
    pub fn ligne_repondue(&self) -> Option<LigneRepondue> {
        let (marque, mot) = match self.data.state {
            EtatCarte::Pending => return None,
            EtatCarte::Allowed => (MarqueReponse::Coche, "Autorisé"),
            EtatCarte::Denied => (MarqueReponse::Croix, "Refusé"),
        };
        let note = self
            .data
            .note
            .as_deref()
            .filter(|n| !n.is_empty())
            .map(|n| format!(" — {n}"))
            .unwrap_or_default();
        Some(LigneRepondue {
            marque,
            texte: format!("{mot} : {}{note}", self.description.short),
        })
    }

    /// Le bloc de détail (commande, chemin) : absent d'un plan et d'une demande sans détail.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        if self.est_plan() {
            return None;
        }
        Some(self.description.detail.as_str()).filter(|d| !d.is_empty())
    }

    /// Le texte du plan.
    #[must_use]
    pub fn plan(&self) -> Option<&str> {
        self.data.request.plan.as_deref()
    }

    /// La raison que donne le moteur.
    #[must_use]
    pub fn raison(&self) -> Option<&str> {
        self.data
            .request
            .reason
            .as_deref()
            .filter(|r| !r.is_empty())
    }

    /// Le texte indicatif du champ « pourquoi ».
    #[must_use]
    pub fn invite_pourquoi(&self) -> &'static str {
        if self.est_plan() {
            "Ce qu’il faut changer…"
        } else {
            "Pourquoi (facultatif, l’agent le lira)"
        }
    }

    /// Les boutons d'une carte qui attend, dans l'ordre. Les « Toujours » n'existent que si la
    /// demande offre une règle à retenir.
    #[must_use]
    pub fn boutons(&self) -> Vec<BoutonCarte> {
        if !self.en_attente() {
            return Vec::new();
        }
        if self.est_plan() {
            return vec![
                bouton(ActionCarte::Lancer, "Lancer", VarianteBouton::Principal),
                bouton(ActionCarte::Modifier, "Modifier", VarianteBouton::Refus),
            ];
        }
        let mut v = vec![bouton(
            ActionCarte::AutoriserUneFois,
            "Autoriser une fois",
            VarianteBouton::Principal,
        )];
        if self.data.rule.is_some() {
            v.push(bouton(
                ActionCarte::ToujoursConversation,
                "Toujours dans cette conversation",
                VarianteBouton::Normal,
            ));
            v.push(bouton(
                ActionCarte::ToujoursCoffre,
                "Toujours dans ce coffre",
                VarianteBouton::Normal,
            ));
        }
        v.push(bouton(
            ActionCarte::Refuser,
            "Refuser",
            VarianteBouton::Refus,
        ));
        v
    }

    /// La décision qu'un bouton renvoie au fournisseur. `None` : action sans objet (un « Toujours »
    /// sans règle).
    #[must_use]
    pub fn decision(&self, action: ActionCarte) -> Option<PermissionDecision> {
        let pourquoi = self.pourquoi.trim();
        let message = Some(pourquoi).filter(|p| !p.is_empty()).map(str::to_owned);
        let toujours = |portee| {
            self.data
                .rule
                .clone()
                .map(|rule| PermissionDecision::Allow {
                    remember: Some(Remember {
                        scope: portee,
                        rule,
                    }),
                })
        };
        match action {
            ActionCarte::AutoriserUneFois | ActionCarte::Lancer => {
                Some(PermissionDecision::Allow { remember: None })
            }
            ActionCarte::ToujoursConversation => toujours(RememberScope::Conversation),
            ActionCarte::ToujoursCoffre => toujours(RememberScope::Vault),
            ActionCarte::Refuser => Some(PermissionDecision::Deny { message }),
            ActionCarte::Modifier => Some(PermissionDecision::Deny {
                message: Some(message.unwrap_or_else(|| PLAN_A_MODIFIER.to_owned())),
            }),
        }
    }
}

/// Les données de la carte APRÈS une décision : autorisée (avec la note d'un « toujours »), ou
/// refusée (avec le message). C'est ce que le fournisseur republie dans le part.
#[must_use]
pub fn repondre(data: &PermissionCardData, decision: &PermissionDecision) -> PermissionCardData {
    let mut suite = data.clone();
    match decision {
        PermissionDecision::Allow { remember } => {
            suite.state = EtatCarte::Allowed;
            suite.note = answer_note(remember.as_ref().map(|r| r.scope), None);
        }
        PermissionDecision::Deny { message } => {
            suite.state = EtatCarte::Denied;
            suite.note = answer_note(None, message.as_deref());
        }
    }
    suite
}

/// Le tour est arrêté pendant que la carte attend : elle est refusée, « Tour arrêté. »
#[must_use]
pub fn refus_a_larret(data: &PermissionCardData) -> PermissionCardData {
    repondre(
        data,
        &PermissionDecision::Deny {
            message: Some(MESSAGE_TOUR_ARRETE.to_owned()),
        },
    )
}
