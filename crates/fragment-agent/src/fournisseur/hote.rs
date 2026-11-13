// Port de app/src/ui/views/agent/AgentPlugin.ts (la part que `AgentChatProvider` en utilise)
//
// Ce que le fournisseur demande à SON HÔTE. En TS le fournisseur tient le plugin `agent` entier
// (`plugin.sessions`, `plugin.run`, `plugin.app.registry`…) ; ici il n'en voit que ce trait, que
// `fr-t7-service` implémente (sessions au journal, run commun, politique) et que
// [`FauxHote`](super::faux::FauxHote) double pour les tests.
//
// ★ Écart assumé : les `Promise` deviennent des appels SYNCHRONES (voir `contrat.rs`). `lancer`
//   rend la main à la FIN du tour ; ses événements et ses demandes d'autorisation passent par
//   [`SortieTour`], que le fournisseur fournit. Une carte de permission attend sa réponse en
//   « pompant » l'hôte ([`AgentHote::pomper`]) : l'hôte laisse avancer ce qui peut répondre
//   (boucle d'événements, autre fil) et dit s'il reste quelque chose à attendre.

use std::path::PathBuf;
use std::rc::Rc;

use crate::contrat::{
    AgentEvent, AgentHarness, Entree, ErreurHarness, PermanentRule, PermissionDecision,
    PermissionMode, PermissionRequest,
};
use crate::fournisseur::pieces::CodecImage;

/// Une session de l'index (`agent:session`) : son id EST celui de la conversation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionAgent {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// `meta` : profil, moteur, origine, branche, usage… (clés camelCase, comme le TS).
    pub meta: Entree,
}

/// Qui écrit dans la session (`touch(…, 'user' | 'plugin')`) : le journal le garde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auteur {
    Utilisateur,
    Plugin,
}

/// Un tour demandé à l'hôte (`AgentQuery` du TS, moins ce que le fournisseur traduit lui-même).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DemandeTour {
    pub prompt: String,
    pub session_id: String,
    /// Le moteur de la conversation (un id de `agent:harness@1`).
    pub harness: String,
    pub system: Option<String>,
    /// `Some(vec![])` : aucun outil (réponse directe).
    pub tools: Option<Vec<String>>,
    pub mode: Option<PermissionMode>,
    pub effort: Option<String>,
    pub model: Option<String>,
    /// Le titre d'une session NEUVE.
    pub title: Option<String>,
    /// Les pièces jointes, en blocs de contenu, envoyées avant le texte.
    pub content: Vec<Entree>,
    /// La `meta` d'une session NEUVE (profil choisi, métadonnées en attente).
    pub meta: Option<Entree>,
    /// Une session NEUVE née d'une autre : (id du parent, repère `ChatEntry::last` où couper).
    pub fork: Option<(String, String)>,
}

/// Ce que le fournisseur reçoit d'un tour (le `onEvent` et le `onPermission` de `AgentQuery`).
pub trait SortieTour {
    /// Un événement du moteur, normalisé.
    fn evenement(&mut self, e: AgentEvent);
    /// Une demande d'autorisation qui reste après la politique : rend la réponse de l'utilisateur
    /// (une carte dans la réponse). Toujours résolue (refus si personne ne répond).
    fn permission(&mut self, r: PermissionRequest) -> PermissionDecision;
}

/// L'hôte du fournisseur.
pub trait AgentHote {
    /// Le nom de l'utilisateur (participant `user`).
    fn nom_utilisateur(&self) -> String;
    /// L'heure (ms depuis l'epoch) : trace et chronos.
    fn maintenant(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }
    /// Un identifiant neuf (UUID de session, id de carte).
    fn nouvel_id(&self) -> String;

    // ── L'index des sessions ──
    fn session(&self, id: &str) -> Option<SessionAgent>;
    /// Les sessions, la plus récente d'abord.
    fn sessions(&self) -> Vec<SessionAgent>;
    /// Remplace la `meta` de la session (et date la modification) ; sans effet si elle n'existe pas.
    fn toucher(&self, id: &str, meta: Entree, auteur: Auteur);
    /// La session quitte l'index (le transcript du moteur reste).
    fn supprimer(&self, id: &str);

    // ── Les moteurs ──
    /// Le harness d'id `id`, relu à chaque usage (un plugin de harness va et vient).
    fn harness(&self, id: &str) -> Option<Rc<dyn AgentHarness>>;
    /// Le moteur par défaut des réglages ; `None` : Claude.
    fn harness_par_defaut(&self) -> Option<String> {
        None
    }

    // ── Le tour ──
    /// Le RUN COMMUN : crée la session si elle est neuve (titre, meta), compose les consignes,
    /// la politique et la garde, confie le tour au harness et rend la main à sa fin. Une erreur
    /// avant ou pendant le tour devient `Err` (le fournisseur la montre dans le tour).
    fn lancer(
        &self,
        demande: DemandeTour,
        sortie: &mut dyn SortieTour,
    ) -> Result<(), ErreurHarness>;
    /// Interrompt le tour en cours de cette conversation.
    fn interrompre(&self, conversation: &str);
    /// Attend une réponse de carte : laisse avancer ce qui peut répondre. Faux : plus rien ne
    /// peut répondre, la carte est refusée au lieu de bloquer.
    fn pomper(&self) -> bool {
        false
    }

    // ── Les services annexes ──
    /// Le dossier des pièces jointes de l'agent (que `Read` doit pouvoir lire).
    fn dossier_pieces(&self) -> PathBuf;
    /// Le décodeur / encodeur d'images ; absent : une image part telle quelle (comme un
    /// `createImageBitmap` qui échoue).
    fn codec_image(&self) -> Option<Rc<dyn CodecImage>> {
        None
    }
    /// La règle qu'un « toujours » retiendrait pour cette demande (la carte la propose).
    fn regle_toujours(&self, _demande: &PermissionRequest) -> Option<PermanentRule> {
        None
    }
}
