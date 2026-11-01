// Port de app/src/ui/coreCommands/cliServer.ts (PARTIE PURE seulement)
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE SERVEUR DU CLI — ce qui se décide SANS socket.
//
//  Côté Fragment, `demarrerServeurCli` mêle trois choses : l'écoute du tuyau
//  nommé (transport, hors de ce port), la publication de l'index (voir
//  `instance_index`) et le TRAITEMENT d'une trame : version, jeton, sous-commande,
//  borne du timeout, plafond de sortie. Seul ce dernier est porté ici, en
//  fonction pure : trame en texte → réponse.
//
//  ★ LE JETON N'EST PAS UNE OPTION. N'importe quel processus de la session peut
//    se connecter au tuyau, et `eval` est de l'exécution de code arbitraire dans
//    le coffre. Le jeton vit dans le fichier d'index (ACLé), jamais dans le nom du
//    tuyau, et se compare à temps constant.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;
use std::error::Error;

use serde_json::Value;

use crate::protocol::{
    CLI_PROTOCOL_VERSION, CliRequest, CliResult, EXIT, OUTPUT_MAX_BYTES, TIMEOUT_MAX_MS,
    TIMEOUT_MIN_MS, echec, message_erreur,
};

/// Ce que le serveur délègue : exécuter une requête déjà validée. Une `Err` devient
/// `EXIT.HANDLER_ERROR` — son MESSAGE seul part au client (A21).
pub type Resolveur<'a> = &'a dyn Fn(&CliRequest) -> Result<CliResult, Box<dyn Error + Send + Sync>>;

/// Levée quand le tuyau du coffre est déjà tenu par une instance VIVANTE (A33).
/// Le nom du tuyau est dérivé du SEUL chemin du coffre (`pipe`), sans pid : deux
/// processus qui ouvrent le même coffre visent le même tuyau. On le DÉTECTE au bind
/// plutôt que de laisser un `EADDRINUSE` brut remonter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ce coffre est déjà ouvert dans une autre instance ({pipe})")]
pub struct ErreurCoffreDejaServi {
    pub pipe: String,
}

impl ErreurCoffreDejaServi {
    /// Le code stable de l'erreur (lu par l'app au démarrage).
    pub const CODE: &'static str = "FRAGMENT_VAULT_ALREADY_SERVED";

    pub fn new(pipe: impl Into<String>) -> Self {
        Self { pipe: pipe.into() }
    }
}

/// Comparaison à temps constant : la durée ne dépend pas du préfixe commun.
fn memes_octets(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Number(x) || 0` de JS, pour une valeur du fil.
fn nombre_ou_zero(v: Option<&Value>) -> f64 {
    let n = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };
    if n.is_finite() { n } else { 0.0 }
}

/// Traite UNE trame reçue : valide, borne, délègue à `resoudre`, plafonne la sortie.
///
/// Ordre des contrôles (celui de Fragment) : trame lisible → version → JETON →
/// sous-commande. ★ Le jeton est vérifié AVANT de regarder la sous-commande : un
/// inconnu n'apprend même pas quelles sous-commandes existent.
pub fn traiter_trame(trame: &str, jeton: &str, resoudre: Resolveur<'_>) -> CliResult {
    let Ok(brut) = serde_json::from_str::<Value>(trame) else {
        return echec(EXIT.USAGE, "trame illisible");
    };
    let Some(objet) = brut.as_object() else {
        return echec(EXIT.USAGE, "trame illisible");
    };

    if objet.get("v").and_then(Value::as_u64) != Some(u64::from(CLI_PROTOCOL_VERSION)) {
        let recu = objet
            .get("v")
            .map_or_else(|| "undefined".to_string(), Value::to_string);
        return echec(
            EXIT.REFUSED,
            format!("version de protocole {recu}, attendue {CLI_PROTOCOL_VERSION}"),
        );
    }
    let jeton_ok = objet
        .get("token")
        .and_then(Value::as_str)
        .is_some_and(|t| memes_octets(t.as_bytes(), jeton.as_bytes()));
    if !jeton_ok {
        return echec(EXIT.REFUSED, "jeton refusé");
    }
    let subcommand = match objet.get("subcommand").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return echec(EXIT.USAGE, "sous-commande manquante"),
    };

    // Le timeout est BORNÉ ici, pas côté client : un client ne doit pas pouvoir
    // demander « attends pour toujours » à l'app.
    let timeout_ms = nombre_ou_zero(objet.get("timeoutMs"))
        .clamp(TIMEOUT_MIN_MS as f64, TIMEOUT_MAX_MS as f64) as u64;
    // ★ Plus strict que le TS : seules les valeurs CHAÎNES des params survivent.
    let params: BTreeMap<String, String> = objet
        .get("params")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let flags: Vec<String> = objet
        .get("flags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| f.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let req = CliRequest {
        v: CLI_PROTOCOL_VERSION,
        token: jeton.to_string(),
        subcommand,
        params,
        flags,
        timeout_ms,
    };

    // La pile reste dans la console de l'hôte ; le client ne voit que le message (A21).
    let res = match resoudre(&req) {
        Ok(res) => res,
        Err(err) => echec(EXIT.HANDLER_ERROR, message_erreur(&err)),
    };

    // Plafond de sortie : une erreur explicite vaut mieux qu'une socket qui
    // s'étrangle sur 200 Mo de `eval`.
    if let CliResult::Succes { output } = &res
        && output.len() > OUTPUT_MAX_BYTES
    {
        return echec(
            EXIT.HANDLER_ERROR,
            format!(
                "sortie de {} octets — au-delà du plafond de {OUTPUT_MAX_BYTES}",
                output.len()
            ),
        );
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::succes;
    use serde_json::json;

    // Les 3 tests de cliServer.test.ts (collision de coffre A33) exigent de VRAIES
    // sockets : non portés (voir parite.toml). Ceux-ci couvrent la partie pure.

    fn ok(_: &CliRequest) -> Result<CliResult, Box<dyn Error + Send + Sync>> {
        Ok(succes("pong"))
    }

    fn requete(v: serde_json::Value) -> String {
        v.to_string()
    }

    fn valide() -> serde_json::Value {
        json!({ "v": 1, "token": "secret", "subcommand": "eval", "params": {"code": "1"}, "flags": ["copy"], "timeoutMs": 5000 })
    }

    #[test]
    fn une_trame_valide_atteint_le_resolveur_avec_la_requete_typee() {
        let vu = std::cell::RefCell::new(None);
        let res = traiter_trame(&requete(valide()), "secret", &|r| {
            *vu.borrow_mut() = Some(r.clone());
            Ok(succes("pong"))
        });
        assert_eq!(res, succes("pong"));
        let r = vu.into_inner().unwrap();
        assert_eq!(r.subcommand, "eval");
        assert_eq!(r.params["code"], "1");
        assert_eq!(r.flags, ["copy"]);
        assert_eq!(r.timeout_ms, 5000);
    }

    #[test]
    fn une_trame_illisible_est_un_usage() {
        assert_eq!(
            traiter_trame("{pas du json", "secret", &ok),
            echec(EXIT.USAGE, "trame illisible")
        );
        assert_eq!(
            traiter_trame("[1]", "secret", &ok),
            echec(EXIT.USAGE, "trame illisible")
        );
    }

    #[test]
    fn une_autre_version_est_refusee() {
        let mut t = valide();
        t["v"] = json!(2);
        assert_eq!(
            traiter_trame(&requete(t), "secret", &ok),
            echec(EXIT.REFUSED, "version de protocole 2, attendue 1")
        );
    }

    #[test]
    fn le_jeton_est_verifie_avant_la_sous_commande() {
        let mut t = valide();
        t["token"] = json!("faux");
        t["subcommand"] = json!("");
        assert_eq!(
            traiter_trame(&requete(t), "secret", &ok),
            echec(EXIT.REFUSED, "jeton refusé")
        );
        let mut t = valide();
        t.as_object_mut().unwrap().remove("token");
        assert_eq!(
            traiter_trame(&requete(t), "secret", &ok),
            echec(EXIT.REFUSED, "jeton refusé")
        );
    }

    #[test]
    fn une_sous_commande_vide_ou_absente_est_un_usage() {
        let mut t = valide();
        t["subcommand"] = json!("");
        assert_eq!(
            traiter_trame(&requete(t), "secret", &ok),
            echec(EXIT.USAGE, "sous-commande manquante")
        );
    }

    #[test]
    fn le_timeout_est_borne_par_le_serveur() {
        let timeout = |valeur: serde_json::Value| {
            let mut t = valide();
            t["timeoutMs"] = valeur;
            let vu = std::cell::Cell::new(0);
            traiter_trame(&requete(t), "secret", &|r| {
                vu.set(r.timeout_ms);
                Ok(succes(""))
            });
            vu.get()
        };
        assert_eq!(timeout(json!(1)), TIMEOUT_MIN_MS);
        assert_eq!(timeout(json!(999_999_999)), TIMEOUT_MAX_MS);
        assert_eq!(timeout(json!("pas un nombre")), TIMEOUT_MIN_MS);
        assert_eq!(timeout(json!(20_000)), 20_000);
    }

    #[test]
    fn une_erreur_du_resolveur_ne_renvoie_que_son_message() {
        let res = traiter_trame(&requete(valide()), "secret", &|_| {
            Err("boum".to_string().into())
        });
        assert_eq!(res, echec(EXIT.HANDLER_ERROR, "boum"));
    }

    #[test]
    fn une_sortie_trop_grosse_devient_une_erreur_explicite() {
        let res = traiter_trame(&requete(valide()), "secret", &|_| {
            Ok(succes("x".repeat(OUTPUT_MAX_BYTES + 1)))
        });
        match res {
            CliResult::Echec { code, error } => {
                assert_eq!(code, EXIT.HANDLER_ERROR);
                assert!(error.contains("au-delà du plafond"));
            }
            CliResult::Succes { .. } => panic!("la sortie aurait dû être refusée"),
        }
    }

    #[test]
    fn erreur_coffre_deja_servi_nomme_le_tuyau() {
        let e = ErreurCoffreDejaServi::new("p");
        assert!(e.to_string().contains("déjà ouvert"));
        assert_eq!(ErreurCoffreDejaServi::CODE, "FRAGMENT_VAULT_ALREADY_SERVED");
    }
}
