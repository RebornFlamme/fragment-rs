// Port de app/src/ui/views/agent/controls/controls.ts (listes, anneau d'usage, emplacements)
//! Les contrôles de l'agent dans la barre du champ — leur logique, sans rendu.
//!
//! Quatre listes (moteur, modèle, effort, mode de permission) et l'anneau d'usage s'inscrivent
//! dans l'emplacement `chat:composer-tools@1`. Chaque contrôle se relit d'une
//! [`SourceControles`] — le fournisseur agent, branché au montage — à chaque image : choisir une
//! valeur change donc d'un coup les listes sœurs (effort d'un autre modèle, mode d'un moteur qui
//! ne sait pas demander), sans le rafraîchissement manuel que le DOM exigeait.
//!
//! Les libellés, l'anneau et le choix du modèle courant viennent de
//! `fragment_agent::controls_model` (déjà porté).

use std::rc::Rc;

use fragment_agent::contrat::{HarnessModel, PermissionMode, TurnUsage};
use fragment_agent::controls_model::{current_model, effort_label, usage_ring};
use fragment_agent::policy_model::mode_label;
use fragment_chat::api::{ChatMode, ChatSlot, ChatSlotContext, SlotAlign};

/// L'id du fournisseur de l'agent : les contrôles ne s'appliquent qu'à lui.
pub const FOURNISSEUR_AGENT: &str = "agent";

/// Un moteur proposé dans la liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoteurPropose {
    pub id: String,
    pub label: String,
    /// Sait DEMANDER : sinon le mode de permission reste « accès complet ».
    pub peut_demander: bool,
}

/// Ce que la conversation a choisi (absent : le défaut du moteur ou de la politique).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfilConversation {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub mode: Option<PermissionMode>,
}

/// Ce que lisent et écrivent les contrôles : le fournisseur agent. Les écritures passent par
/// `&self` (état interne du fournisseur) ; c'est lui qui notifie la vue.
pub trait SourceControles {
    fn moteurs(&self) -> Vec<MoteurPropose>;
    /// Le moteur de la conversation.
    fn moteur_de(&self, conversation: &str) -> String;
    /// Faux dès que la conversation a un premier message.
    fn peut_changer_moteur(&self, conversation: &str) -> bool;
    fn definir_moteur(&self, conversation: &str, moteur: &str);
    /// Les modèles d'un moteur (vide s'il est indisponible).
    fn modeles(&self, moteur: &str) -> Vec<HarnessModel>;
    fn profil(&self, conversation: &str) -> ProfilConversation;
    fn definir_modele(&self, conversation: &str, modele: &str);
    fn definir_effort(&self, conversation: &str, effort: Option<&str>);
    fn definir_mode(&self, conversation: &str, mode: PermissionMode);
    /// Le mode de la politique de base, quand la conversation n'en a pas choisi.
    fn mode_de_base(&self) -> PermissionMode;
    /// Le dernier usage connu (anneau).
    fn usage(&self, conversation: &str) -> Option<TurnUsage>;
}

/// Les quatre listes de la barre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controle {
    Moteur,
    Modele,
    Effort,
    Mode,
}

impl Controle {
    /// Dans l'ordre d'inscription.
    pub const TOUS: [Controle; 4] = [
        Controle::Moteur,
        Controle::Modele,
        Controle::Effort,
        Controle::Mode,
    ];

    /// L'id de l'emplacement (`data-slot`).
    #[must_use]
    pub fn id_slot(self) -> &'static str {
        match self {
            Controle::Moteur => "agent-harness",
            Controle::Modele => "agent-model",
            Controle::Effort => "agent-effort",
            Controle::Mode => "agent-mode",
        }
    }

    /// La classe du bouton (`mod-harness`…), que les specs ciblent.
    #[must_use]
    pub fn classe(self) -> &'static str {
        match self {
            Controle::Moteur => "mod-harness",
            Controle::Modele => "mod-model",
            Controle::Effort => "mod-effort",
            Controle::Mode => "mod-mode",
        }
    }
}

/// Ce que montre le bouton d'une liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatListe {
    pub libelle: String,
    /// L'infobulle et le nom accessible.
    pub info: String,
    /// Grisé : pas de liste au clic, pas de chevron.
    pub desactive: bool,
    /// Caché (l'effort d'un modèle sans niveaux).
    pub masque: bool,
}

/// Une ligne de la liste ouverte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeListe {
    /// Ce que `choisir` reçoit (vide : « Par défaut » de l'effort).
    pub id: String,
    pub titre: String,
    pub coche: bool,
    pub desactive: bool,
}

/// Le palier de couleur de l'anneau (`data-level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiveauUsage {
    Bas,
    Moyen,
    Haut,
}

/// L'anneau d'usage : la fenêtre de contexte remplie.
#[derive(Debug, Clone, PartialEq)]
pub struct EtatAnneau {
    pub fraction: f64,
    pub niveau: NiveauUsage,
    pub label: String,
}

/// Le nom d'un mode dans les réglages et les profils (`acceptEdits`…).
#[must_use]
pub fn mode_id(m: PermissionMode) -> &'static str {
    match m {
        PermissionMode::Full => "full",
        PermissionMode::Ask => "ask",
        PermissionMode::AcceptEdits => "acceptEdits",
        PermissionMode::Plan => "plan",
    }
}

#[must_use]
pub fn mode_depuis_id(id: &str) -> Option<PermissionMode> {
    [
        PermissionMode::Full,
        PermissionMode::Ask,
        PermissionMode::AcceptEdits,
        PermissionMode::Plan,
    ]
    .into_iter()
    .find(|m| mode_id(*m) == id)
}

/// Les contrôles d'UNE conversation, lus de la source.
pub struct Controles {
    source: Rc<dyn SourceControles>,
    conversation: String,
}

impl Controles {
    #[must_use]
    pub fn new(source: Rc<dyn SourceControles>, conversation: impl Into<String>) -> Self {
        Self {
            source,
            conversation: conversation.into(),
        }
    }

    fn moteur(&self) -> Option<MoteurPropose> {
        let id = self.source.moteur_de(&self.conversation);
        self.source.moteurs().into_iter().find(|m| m.id == id)
    }

    fn peut_demander(&self) -> bool {
        self.moteur().is_some_and(|m| m.peut_demander)
    }

    fn modeles(&self) -> Vec<HarnessModel> {
        self.source
            .modeles(&self.source.moteur_de(&self.conversation))
    }

    fn mode_courant(&self) -> PermissionMode {
        if self.peut_demander() {
            self.source
                .profil(&self.conversation)
                .mode
                .unwrap_or_else(|| self.source.mode_de_base())
        } else {
            PermissionMode::Full
        }
    }

    /// Ce que montre le bouton du contrôle.
    #[must_use]
    pub fn etat(&self, c: Controle) -> EtatListe {
        let profil = self.source.profil(&self.conversation);
        match c {
            Controle::Moteur => {
                let id = self.source.moteur_de(&self.conversation);
                let nom = self.moteur().map_or(id, |m| m.label);
                let fige = !self.source.peut_changer_moteur(&self.conversation);
                EtatListe {
                    info: if fige {
                        format!("Moteur : {nom} (figé après le premier message)")
                    } else {
                        "Moteur de la conversation".to_string()
                    },
                    libelle: nom,
                    desactive: fige,
                    masque: false,
                }
            }
            Controle::Modele => {
                if self.moteur().is_none() {
                    return EtatListe {
                        libelle: "—".into(),
                        info: "Moteur indisponible".into(),
                        desactive: true,
                        masque: false,
                    };
                }
                let modeles = self.modeles();
                let m = current_model(&modeles, profil.model.as_deref());
                EtatListe {
                    libelle: m.map_or_else(|| "Modèle".to_string(), |m| m.label.clone()),
                    info: format!("Modèle : {}", m.map_or("—", |m| m.label.as_str())),
                    desactive: modeles.is_empty(),
                    masque: false,
                }
            }
            Controle::Effort => {
                let modeles = self.modeles();
                let niveaux = current_model(&modeles, profil.model.as_deref())
                    .and_then(|m| m.efforts.as_ref())
                    .is_some_and(|e| !e.is_empty());
                let nom = effort_label(profil.effort.as_deref());
                EtatListe {
                    info: format!("Effort : {nom}"),
                    libelle: nom,
                    desactive: false,
                    masque: !niveaux,
                }
            }
            Controle::Mode => {
                let nom = mode_label(self.mode_courant()).to_string();
                EtatListe {
                    info: if self.peut_demander() {
                        format!("Permissions : {nom}")
                    } else {
                        "Permissions : accès complet (ce moteur ne sait pas demander)".to_string()
                    },
                    libelle: nom,
                    desactive: false,
                    masque: false,
                }
            }
        }
    }

    /// Le mode effectif (`data-mode`).
    #[must_use]
    pub fn mode(&self) -> PermissionMode {
        self.mode_courant()
    }

    /// Les lignes de la liste ouverte.
    #[must_use]
    pub fn entrees(&self, c: Controle) -> Vec<EntreeListe> {
        let profil = self.source.profil(&self.conversation);
        match c {
            Controle::Moteur => {
                let actuel = self.source.moteur_de(&self.conversation);
                self.source
                    .moteurs()
                    .into_iter()
                    .map(|m| EntreeListe {
                        coche: m.id == actuel,
                        id: m.id,
                        titre: m.label,
                        desactive: false,
                    })
                    .collect()
            }
            Controle::Modele => {
                let modeles = self.modeles();
                let actuel = current_model(&modeles, profil.model.as_deref()).map(|m| m.id.clone());
                modeles
                    .iter()
                    .map(|m| EntreeListe {
                        id: m.id.clone(),
                        titre: match m.description.as_deref().filter(|d| !d.is_empty()) {
                            Some(d) => format!("{} — {d}", m.label),
                            None => m.label.clone(),
                        },
                        coche: actuel.as_deref() == Some(m.id.as_str()),
                        desactive: false,
                    })
                    .collect()
            }
            Controle::Effort => {
                let modeles = self.modeles();
                let niveaux = current_model(&modeles, profil.model.as_deref())
                    .and_then(|m| m.efforts.clone())
                    .unwrap_or_default();
                let actuel = profil.effort.filter(|e| !e.is_empty());
                let mut v = vec![EntreeListe {
                    id: String::new(),
                    titre: "Par défaut".into(),
                    coche: actuel.is_none(),
                    desactive: false,
                }];
                v.extend(niveaux.into_iter().map(|n| EntreeListe {
                    coche: actuel.as_deref() == Some(n.as_str()),
                    titre: effort_label(Some(&n)),
                    id: n,
                    desactive: false,
                }));
                v
            }
            Controle::Mode => {
                let actuel = profil.mode.unwrap_or_else(|| self.source.mode_de_base());
                let demande = self.peut_demander();
                [
                    PermissionMode::Full,
                    PermissionMode::Ask,
                    PermissionMode::AcceptEdits,
                    PermissionMode::Plan,
                ]
                .into_iter()
                .map(|m| EntreeListe {
                    id: mode_id(m).into(),
                    titre: mode_label(m).into(),
                    coche: m == actuel,
                    desactive: m != PermissionMode::Full && !demande,
                })
                .collect()
            }
        }
    }

    /// Une ligne choisie. `false` : refusée (liste grisée, ligne grisée, valeur inconnue).
    pub fn choisir(&self, c: Controle, id: &str) -> bool {
        let conv = &self.conversation;
        match c {
            Controle::Moteur => {
                if !self.source.peut_changer_moteur(conv) {
                    return false;
                }
                self.source.definir_moteur(conv, id);
            }
            Controle::Modele => {
                if !self.modeles().iter().any(|m| m.id == id) {
                    return false;
                }
                self.source.definir_modele(conv, id);
            }
            Controle::Effort => self
                .source
                .definir_effort(conv, Some(id).filter(|i| !i.is_empty())),
            Controle::Mode => {
                let Some(m) = mode_depuis_id(id) else {
                    return false;
                };
                if m != PermissionMode::Full && !self.peut_demander() {
                    return false;
                }
                self.source.definir_mode(conv, m);
            }
        }
        true
    }

    /// L'anneau d'usage, ou `None` (rien à montrer : l'emplacement se cache).
    #[must_use]
    pub fn anneau(&self) -> Option<EtatAnneau> {
        let r = usage_ring(self.source.usage(&self.conversation).as_ref())?;
        let niveau = if r.fraction > 0.85 {
            NiveauUsage::Haut
        } else if r.fraction > 0.6 {
            NiveauUsage::Moyen
        } else {
            NiveauUsage::Bas
        };
        Some(EtatAnneau {
            fraction: r.fraction,
            niveau,
            label: r.label,
        })
    }
}

/// Les emplacements `chat:composer-tools@1` des contrôles, dans l'ordre d'inscription : moteur,
/// modèle, effort, mode, usage. Ils ne s'appliquent qu'au fournisseur agent ; le moteur
/// disparaît d'une conversation ANCRÉE, l'anneau de la BARRE d'un document (elle lance des
/// conversations, aucune n'y tourne).
#[must_use]
pub fn slots_controles() -> Vec<ChatSlot> {
    fn agent(ctx: &ChatSlotContext) -> bool {
        ctx.provider == FOURNISSEUR_AGENT
    }
    vec![
        ChatSlot::new("agent-harness")
            .with_applies_to(|c| agent(c) && c.mode != ChatMode::Anchored),
        ChatSlot::new("agent-model").with_applies_to(agent),
        ChatSlot::new("agent-effort").with_applies_to(agent),
        ChatSlot::new("agent-mode").with_applies_to(agent),
        ChatSlot::new("agent-usage")
            .with_align(SlotAlign::End)
            .with_applies_to(|c| agent(c) && c.mode != ChatMode::Bar),
    ]
}
