// Port de app/src/shared/cli/protocol.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE CONTRAT DU CLI — les deux moitiés dans le même fichier.
//
//  Le client, le serveur et l'app l'importent tous : ce qui est encodé d'un côté
//  et décodé de l'autre vit à un seul endroit, pour que la symétrie se relise
//  d'un coup d'œil.
//
//  ★ Ce module n'a besoin de RIEN d'autre que serde : le hash du tuyau (qui
//    demandait Node côté TS) vit dans `pipe.rs`.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;
use std::fmt::Display;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Incrémenté à chaque changement incompatible de trame. Le serveur refuse un autre numéro.
pub const CLI_PROTOCOL_VERSION: u32 = 1;

/// Ce que le client envoie. Une requête par connexion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliRequest {
    pub v: u32,
    /// Le secret de l'instance — lu dans son fichier d'index, jamais dans le nom du tuyau.
    pub token: String,
    /// `command`, `eval`, `templater:create-from-template`…
    pub subcommand: String,
    /// Les `clé=valeur` de la ligne de commande.
    pub params: BTreeMap<String, String>,
    /// Les `--drapeaux`, sans les tirets.
    pub flags: Vec<String>,
    /// Au-delà, le serveur abandonne et rend `EXIT.TIMEOUT`. Borné par le serveur.
    pub timeout_ms: u64,
}

/// Ce que le serveur répond.
///
/// Sur le fil : `{ "ok": true, "output": … }` ou `{ "ok": false, "error": …, "code": … }`
/// (le `ok` booléen n'est pas un tag serde standard, d'où l'impl à la main).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliResult {
    Succes { output: String },
    Echec { code: i32, error: String },
}

#[derive(Serialize, Deserialize)]
struct CliResultFil {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
}

impl Serialize for CliResult {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let fil = match self {
            CliResult::Succes { output } => CliResultFil {
                ok: true,
                output: Some(output.clone()),
                error: None,
                code: None,
            },
            CliResult::Echec { code, error } => CliResultFil {
                ok: false,
                output: None,
                error: Some(error.clone()),
                code: Some(*code),
            },
        };
        fil.serialize(s)
    }
}

impl<'de> Deserialize<'de> for CliResult {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let fil = CliResultFil::deserialize(d)?;
        if fil.ok {
            let output = fil
                .output
                .ok_or_else(|| D::Error::missing_field("output"))?;
            Ok(CliResult::Succes { output })
        } else {
            let error = fil.error.ok_or_else(|| D::Error::missing_field("error"))?;
            let code = fil.code.ok_or_else(|| D::Error::missing_field("code"))?;
            Ok(CliResult::Echec { code, error })
        }
    }
}

/// Les DEUX seules façons de construire un `CliResult` (A21). Passer par ces
/// fabriques garantit la forme unique — fini le `{ ok:false, code, error }`
/// recopié à la main à treize endroits, avec un `code` libre et des champs
/// oubliables.
pub fn succes(output: impl Into<String>) -> CliResult {
    CliResult::Succes {
        output: output.into(),
    }
}

/// Voir [`succes`].
pub fn echec(code: i32, error: impl Into<String>) -> CliResult {
    CliResult::Echec {
        code,
        error: error.into(),
    }
}

/// Le message d'une erreur — JAMAIS sa pile (A21). `err.stack` porte les chemins
/// absolus de l'installation : le renvoyer au client par la socket est une fuite.
/// La pile se journalise en LOCAL ; seul le message part au CLI.
///
/// ★ En Rust il n'y a pas de pile dans `Display` : la fonction prend n'importe quel
///   `Display` (une erreur, une chaîne, un nombre) comme `String(err)` côté TS.
pub fn message_erreur(err: &dyn Display) -> String {
    err.to_string()
}

/// Un `connect()` au tuyau qui échoue ainsi = PERSONNE n'écoute : instance morte,
/// index orphelin. Partagé par le client (code de sortie) et la purge des index.
pub const CODES_TUYAU_MORT: [&str; 3] = ["ECONNREFUSED", "ENOENT", "EPIPE"];

/// Ce code d'erreur de connexion signifie-t-il « tuyau mort » ?
pub fn est_tuyau_mort(code: &str) -> bool {
    CODES_TUYAU_MORT.contains(&code)
}

/// Les codes de sortie — stables, documentés, testables.
///
/// ★ Obsidian rend 0 même en cas d'erreur (« The CLI exits 0 even on errors, so
///   read what it printed »). C'est le seul point de sa CLI qu'on ne copie PAS :
///   un script shell qui ne peut pas tester `$?` n'est pas scriptable.
#[allow(non_snake_case)] // ★ champs en MAJUSCULES : `EXIT.OK`, comme côté TS (contrats.md).
#[derive(Debug, Clone, Copy)]
pub struct Exit {
    pub OK: i32,
    /// Le handler a levé. Le message est dans `error`.
    pub HANDLER_ERROR: i32,
    /// `vault=` ne désigne aucune instance connue.
    pub UNKNOWN_VAULT: i32,
    /// Sous-commande inconnue du serveur.
    pub UNKNOWN_SUBCOMMAND: i32,
    /// Pas d'instance qui tourne, et impossible d'en lancer une.
    pub NO_INSTANCE: i32,
    /// Plusieurs instances, et pas de `vault=` pour choisir.
    pub AMBIGUOUS_VAULT: i32,
    /// `command id=` désigne une commande qui n'existe pas dans le registre.
    pub UNKNOWN_COMMAND: i32,
    /// `command id=` désigne une commande connue mais INDISPONIBLE ici
    /// (p. ex. aucun éditeur actif).
    pub UNAVAILABLE: i32,
    /// Ligne de commande mal formée.
    pub USAGE: i32,
    /// Le serveur n'a pas répondu à temps — convention de GNU timeout.
    pub TIMEOUT: i32,
    /// La fenêtre a disparu pendant la requête.
    pub RENDERER_GONE: i32,
    /// Jeton absent ou faux.
    pub REFUSED: i32,
}

pub const EXIT: Exit = Exit {
    OK: 0,
    HANDLER_ERROR: 1,
    UNKNOWN_VAULT: 2,
    UNKNOWN_SUBCOMMAND: 4,
    NO_INSTANCE: 5,
    AMBIGUOUS_VAULT: 6,
    UNKNOWN_COMMAND: 7,
    UNAVAILABLE: 8,
    USAGE: 64,
    TIMEOUT: 124,
    RENDERER_GONE: 125,
    REFUSED: 126,
};

/// Bornes du timeout par requête : un `eval` qui boucle fige l'app mono-thread.
pub const TIMEOUT_MIN_MS: u64 = 1_000;
pub const TIMEOUT_MAX_MS: u64 = 300_000;
pub const TIMEOUT_DEFAULT_MS: u64 = 30_000;

/// Au-delà, la sortie est tronquée avec une erreur explicite — pas une socket épuisée.
pub const OUTPUT_MAX_BYTES: usize = 8 * 1024 * 1024;

/// Une instance qui tourne, telle qu'elle se décrit dans
/// `<userData>/cli-instances/<id>.json`. Un fichier PAR instance : deux coffres
/// ouverts n'écrivent jamais le même fichier, donc aucune course.
///
/// ★ Les noms JSON (`vaultPath`, `startedAt`…) sont ceux de Fragment : un index écrit
///   par l'un est lisible par l'autre pendant la migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstanceRecord {
    pub v: u32,
    pub vault_path: String,
    /// Le nom du dossier — ce que `vault=` compare, sans tenir compte de la casse.
    pub vault_name: String,
    pub pipe: String,
    pub token: String,
    pub pid: u32,
    pub version: String,
    pub started_at: u64,
}

// ── La trame ─────────────────────────────────────────────────────────────
//  JSON sur une ligne, terminé par '\n'. Une requête, une réponse, la
//  connexion se ferme. Assez simple pour être lue avec `nc`, assez stricte
//  pour ne jamais confondre deux messages.

/// Encode un message en trame : JSON sur une ligne + `\n`.
///
/// ★ `serde_json` n'émet jamais de saut de ligne brut dans une chaîne (il l'échappe
///   en `\n`), donc la trame reste sur une seule ligne.
pub fn encoder_trame<T: Serialize>(message: &T) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string(message)?;
    json.push('\n');
    Ok(json)
}

/// Accumule les octets reçus jusqu'au premier `\n`. Rend `None` tant que la
/// trame n'est pas complète — l'appelant rappelle avec plus de données.
///
/// ★ Le tampon est en OCTETS, pas en texte : un caractère multi-octets coupé entre
///   deux morceaux (« 多 » = 3 octets) est recollé avant le décodage. Le TS décodait
///   chaque morceau seul et pouvait abîmer ce cas.
#[derive(Debug, Default)]
pub struct LecteurDeTrame {
    tampon: Vec<u8>,
}

impl LecteurDeTrame {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rend la trame complète (sans `\n`), ou `None` s'il en manque.
    pub fn pousser(&mut self, morceau: &[u8]) -> Option<String> {
        self.tampon.extend_from_slice(morceau);
        let fin = self.tampon.iter().position(|&o| o == b'\n')?;
        let reste = self.tampon.split_off(fin + 1);
        let mut trame = std::mem::replace(&mut self.tampon, reste);
        trame.pop(); // le '\n'
        Some(String::from_utf8_lossy(&trame).into_owned())
    }

    /// Octets en attente (le serveur coupe un client qui n'en finit pas).
    pub fn taille(&self) -> usize {
        self.tampon.len()
    }
}

/// Le résultat de [`analyser_argv`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argv {
    pub subcommand: Option<String>,
    pub params: BTreeMap<String, String>,
    pub flags: Vec<String>,
}

/// L'analyse de la ligne de commande — la grammaire d'Obsidian.
///
/// ```text
/// fragment [vault=<nom>] <sous-commande> [clé=valeur…] [--drapeau…]
/// ```
///
/// ★ `vault=` est un paramètre comme les autres syntaxiquement ; c'est le
///   client qui le retire des `params` pour choisir l'instance. Il peut venir
///   avant OU après la sous-commande — Obsidian exige « en premier », on est
///   plus tolérant sans rien casser.
///
/// Une valeur peut contenir `=` (« code=a=b » → `a=b`). Les guillemets sont
/// déjà retirés par le shell.
pub fn analyser_argv<S: AsRef<str>>(argv: &[S]) -> Argv {
    let mut subcommand: Option<String> = None;
    let mut params = BTreeMap::new();
    let mut flags = Vec::new();

    for arg in argv {
        let arg = arg.as_ref();
        if let Some(drapeau) = arg.strip_prefix("--") {
            flags.push(drapeau.to_string());
            continue;
        }
        // `egal > 0` : un `=` en tête n'ouvre pas de paramètre.
        if let Some(egal) = arg.find('=').filter(|&i| i > 0) {
            params.insert(arg[..egal].to_string(), arg[egal + 1..].to_string());
            continue;
        }
        if subcommand.is_none() {
            subcommand = Some(arg.to_string());
        } else {
            // un mot nu après la sous-commande = un drapeau court (`total`, `open`)
            flags.push(arg.to_string());
        }
    }
    Argv {
        subcommand,
        params,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(paires: &[(&str, &str)]) -> BTreeMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // ── fabriques de résultat (A21) ──────────────────────────────────────

    #[test]
    fn succes_echec_construisent_la_forme_canonique_et_rien_d_autre() {
        assert_eq!(
            succes("ok"),
            CliResult::Succes {
                output: "ok".into()
            }
        );
        assert_eq!(
            echec(EXIT.USAGE, "oups"),
            CliResult::Echec {
                code: EXIT.USAGE,
                error: "oups".into()
            }
        );
        // La forme sur le fil est celle de Fragment, champ pour champ.
        let ok: serde_json::Value = serde_json::to_value(succes("ok")).unwrap();
        assert_eq!(ok, serde_json::json!({ "ok": true, "output": "ok" }));
        let ko: serde_json::Value = serde_json::to_value(echec(64, "oups")).unwrap();
        assert_eq!(
            ko,
            serde_json::json!({ "ok": false, "code": 64, "error": "oups" })
        );
        // Et elle se relit.
        let relu: CliResult = serde_json::from_str(r#"{"ok":false,"error":"x","code":7}"#).unwrap();
        assert_eq!(relu, echec(7, "x"));
    }

    #[test]
    fn message_erreur_rend_le_message_jamais_la_pile() {
        let err = std::io::Error::other("boum");
        assert_eq!(message_erreur(&err), "boum");
        assert!(!message_erreur(&err).contains('\n'));
        // Un non-erreur est stringifié proprement.
        assert_eq!(message_erreur(&"juste une string"), "juste une string");
        assert_eq!(message_erreur(&42), "42");
    }

    // ── analyser_argv — la grammaire d'Obsidian ──────────────────────────

    #[test]
    fn analyser_argv_sous_commande_cle_valeur_drapeaux() {
        assert_eq!(
            analyser_argv(&["command", "id=editor:toggle-bold", "--copy"]),
            Argv {
                subcommand: Some("command".into()),
                params: params(&[("id", "editor:toggle-bold")]),
                flags: vec!["copy".into()],
            }
        );
    }

    #[test]
    fn analyser_argv_vault_peut_venir_avant_la_sous_commande() {
        let r = analyser_argv(&["vault=Notes", "commands", "filter=editor"]);
        assert_eq!(r.subcommand.as_deref(), Some("commands"));
        assert_eq!(
            r.params,
            params(&[("vault", "Notes"), ("filter", "editor")])
        );
    }

    #[test]
    fn analyser_argv_une_valeur_peut_contenir_des_egal() {
        assert_eq!(
            analyser_argv(&["eval", "code=a=b==c"]).params.get("code"),
            Some(&"a=b==c".to_string())
        );
    }

    #[test]
    fn analyser_argv_un_mot_nu_apres_la_sous_commande_est_un_drapeau_court() {
        assert_eq!(
            analyser_argv(&["tasks", "total", "done"]).flags,
            vec!["total".to_string(), "done".to_string()]
        );
    }

    #[test]
    fn analyser_argv_rien_pas_de_sous_commande() {
        let vide: [&str; 0] = [];
        assert_eq!(analyser_argv(&vide).subcommand, None);
    }

    // ── LecteurDeTrame ───────────────────────────────────────────────────

    #[test]
    fn lecteur_de_trame_rend_none_tant_que_la_ligne_n_est_pas_complete_puis_la_trame() {
        let mut l = LecteurDeTrame::new();
        assert_eq!(l.pousser(b"{\"a\":"), None);
        assert_eq!(l.pousser(b"1}\n"), Some("{\"a\":1}".to_string()));
    }

    #[test]
    fn lecteur_de_trame_garde_ce_qui_suit_le_saut_de_ligne_pour_la_trame_suivante() {
        let mut l = LecteurDeTrame::new();
        assert_eq!(l.pousser(b"un\nde"), Some("un".to_string()));
        assert_eq!(l.pousser(b"ux\n"), Some("deux".to_string()));
    }

    #[test]
    fn lecteur_de_trame_fait_l_aller_retour_avec_encoder_trame() {
        let mut l = LecteurDeTrame::new();
        let trame = l
            .pousser(encoder_trame(&succes("é\n多")).unwrap().as_bytes())
            .unwrap();
        let relu: CliResult = serde_json::from_str(&trame).unwrap();
        assert_eq!(relu, succes("é\n多"));
    }

    // ── Ajouts du port (pas dans le TS) ──────────────────────────────────

    #[test]
    fn lecteur_de_trame_recolle_un_caractere_coupe_entre_deux_morceaux() {
        let octets = "多\n".as_bytes();
        let mut l = LecteurDeTrame::new();
        assert_eq!(l.pousser(&octets[..1]), None);
        assert_eq!(l.pousser(&octets[1..]), Some("多".to_string()));
        assert_eq!(l.taille(), 0);
    }

    #[test]
    fn requete_utilise_les_noms_json_de_fragment() {
        let r = CliRequest {
            v: CLI_PROTOCOL_VERSION,
            token: "t".into(),
            subcommand: "eval".into(),
            params: params(&[("code", "1")]),
            flags: vec![],
            timeout_ms: 5_000,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["timeoutMs"], 5_000);
        assert_eq!(serde_json::from_value::<CliRequest>(json).unwrap(), r);
    }
}
