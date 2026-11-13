// Port de app/src/ui/views/agent/engine/diagnostic.ts
//
//! Le diagnostic d'une erreur du moteur : une erreur BRUTE (Claude Code, le téléchargement) devient
//! ce qui manque ([`EngineNeed`]) et un message en français montrable tel quel. Un message inconnu
//! n'est pas deviné : pas de `need`, le message brut.
//!
//! ★ Les motifs viennent de messages relevés sur le vrai Claude Code ; les tests en gardent la
//!   réécriture. Ajout propre au Rust : les formules de `ureq` (DNS, connexion, délai) rejoignent
//!   celles de Node dans le cas « réseau ».

use std::sync::LazyLock;

use crate::motif::Motif;

/// Ce qu'il manque pour que l'agent tourne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineNeed {
    Install,
    Login,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDiagnostic {
    pub need: Option<EngineNeed>,
    pub message: String,
}

static PREFIXE_SDK: LazyLock<Motif> =
    LazyLock::new(|| Motif::new(r"^Claude Code returned an error result:\s*"));
static PREFIXE_ERROR: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"^Error:\s*"));
static CLE_INVALIDE: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"(?i)invalid api key"));
static NON_CONNECTE: LazyLock<Motif> =
    LazyLock::new(|| Motif::new(r"(?i)not logged in|please run /login"));
static BINAIRE_ABSENT: LazyLock<Motif> = LazyLock::new(|| {
    Motif::new(r"(?i)native binary not found|absent de l'archive|tronqué|empreinte invalide")
});
static RESEAU: LazyLock<Motif> = LazyLock::new(|| {
    Motif::new(
        r"(?i)ENOTFOUND|ECONNREFUSED|ECONNRESET|ETIMEDOUT|EAI_AGAIN|can't reach the api server|connection refused|délai dépassé|dns error|connection failed|timed out",
    )
});

/// Le texte utile d'une erreur : sans « Error: » ni préfixe du SDK.
pub fn raw_message(brut: &str) -> String {
    let sans_error = PREFIXE_ERROR.replace_all(brut, "");
    PREFIXE_SDK.replace_all(&sans_error, "").trim().to_string()
}

/// Ce qui manque, et le message à montrer.
pub fn diagnose(brut: &str) -> EngineDiagnostic {
    let m = raw_message(brut);
    let avec = |need, message: &str| EngineDiagnostic {
        need: Some(need),
        message: message.to_string(),
    };
    if CLE_INVALIDE.is_match(&m) {
        return avec(
            EngineNeed::Login,
            "La clé API Anthropic est refusée. Corrige-la ci-dessous, ou efface-la pour utiliser la connexion de Claude Code.",
        );
    }
    if NON_CONNECTE.is_match(&m) {
        return avec(
            EngineNeed::Login,
            "Claude Code n’est pas connecté. Connecte-le, ou donne une clé API Anthropic.",
        );
    }
    if BINAIRE_ABSENT.is_match(&m) {
        return avec(
            EngineNeed::Install,
            "Le moteur de l’agent (Claude Code) est introuvable ou abîmé : il faut l’installer.",
        );
    }
    if RESEAU.is_match(&m) {
        return avec(
            EngineNeed::Network,
            "Impossible de joindre le service : vérifie ta connexion à Internet (ou un pare-feu, un proxy).",
        );
    }
    EngineDiagnostic {
        need: None,
        message: if m.is_empty() {
            "Erreur inconnue.".to_string()
        } else {
            m
        },
    }
}
