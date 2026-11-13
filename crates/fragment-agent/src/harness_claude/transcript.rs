// Port de sdkProcess.mjs (`getSessionInfo`, `getSessionMessages`) et de AgentHost.ts
// (`getSession`), réécrits sans le SDK : le transcript se lit en Rust (ADR-016 §5, §6)
//
// Le binaire range chaque conversation dans `<config>/projects/<cwd encodé>/<id>.jsonl`. Ce
// fichier est LA source du fil (Fragment n'en garde pas de copie) : `history` le relit, et son
// existence décide entre `--resume` et `--session-id`.
//
// ★ Le dossier de config est `~/.claude` : le harness RETIRE `CLAUDE_CONFIG_DIR` de
//   l'environnement du binaire (comme tout `CLAUDE*`), donc il n'y a pas d'autre endroit où
//   chercher. Les chemins longs ou non ASCII ne sont pas mesurés (S9 §5) : si le nom encodé
//   n'existe pas, on cherche `<id>.jsonl` dans tous les projets.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::contrat::ChatEntry;
use crate::events::to_chat_entries;

/// Le dossier de projets du binaire sous ce dossier de config.
pub fn dossier_projets(config: &Path) -> PathBuf {
    config.join("projects")
}

/// Le nom d'un dossier de travail dans `projects/` : tout caractère non alphanumérique devient
/// `-` (`C:\dev\bac` → `C--dev-bac`).
pub fn encoder_cwd(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Le transcript de la session `id` du dossier `cwd`, s'il existe.
pub fn chemin_transcript(config: &Path, cwd: &str, id: &str) -> Option<PathBuf> {
    // Un id sert de nom de fichier : rien qui puisse sortir du dossier.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let racine = dossier_projets(config);
    let nom = format!("{id}.jsonl");
    let p = racine.join(encoder_cwd(cwd)).join(&nom);
    if p.is_file() {
        return Some(p);
    }
    fs::read_dir(&racine).ok()?.flatten().find_map(|d| {
        let q = d.path().join(&nom);
        q.is_file().then_some(q)
    })
}

/// Les messages du fil PRINCIPAL d'un transcript : la chaîne des `parentUuid` remontée depuis
/// la dernière ligne `user` ou `assistant` (une reprise à un point antérieur laisse des lignes
/// hors chaîne), sans les lignes des sous-agents (`isSidechain`).
pub fn messages_du_fil(texte: &str) -> Vec<Value> {
    let lignes: Vec<Value> = texte
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut par_uuid: HashMap<&str, &Value> = HashMap::new();
    for l in &lignes {
        if let Some(u) = l.get("uuid").and_then(Value::as_str) {
            par_uuid.insert(u, l);
        }
    }
    let conversation = |l: &Value| {
        matches!(
            l.get("type").and_then(Value::as_str),
            Some("user") | Some("assistant")
        ) && l.get("isSidechain") != Some(&Value::Bool(true))
    };
    let Some(feuille) = lignes.iter().rev().find(|l| conversation(l)) else {
        return Vec::new();
    };
    let mut fil = Vec::new();
    let mut courant = Some(feuille);
    let mut garde = 0usize;
    while let Some(l) = courant {
        garde += 1;
        if garde > lignes.len() {
            break; // une boucle dans la chaîne : on s'arrête
        }
        if conversation(l) {
            fil.push(l.clone());
        }
        courant = l
            .get("parentUuid")
            .and_then(Value::as_str)
            .and_then(|p| par_uuid.get(p).copied());
    }
    fil.reverse();
    fil
}

/// Le fil d'une conversation, en bulles. Vide si le transcript manque (autre appareil, session
/// jamais posée) ou s'il est illisible.
pub fn lire_historique(config: &Path, cwd: &str, id: &str) -> Vec<ChatEntry> {
    let Some(p) = chemin_transcript(config, cwd, id) else {
        return Vec::new();
    };
    match fs::read_to_string(p) {
        Ok(t) => to_chat_entries(&messages_du_fil(&t)),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_cwd_s_encode_comme_le_binaire() {
        assert_eq!(encoder_cwd("C:\\dev\\s8-bac"), "C--dev-s8-bac");
        assert_eq!(encoder_cwd("/home/a b/coffre"), "-home-a-b-coffre");
    }

    #[test]
    fn un_id_qui_sort_du_dossier_n_est_jamais_lu() {
        assert_eq!(chemin_transcript(Path::new("."), "c", "../x"), None);
        assert_eq!(chemin_transcript(Path::new("."), "c", ""), None);
    }

    #[test]
    fn le_fil_suit_la_chaine_des_parents_sans_les_sous_agents() {
        let l = |v: Value| v.to_string();
        let texte = [
            l(json!({"type":"user","uuid":"1","parentUuid":null,"message":{"role":"user","content":"a"}})),
            l(json!({"type":"attachment","uuid":"2","parentUuid":"1"})),
            l(json!({"type":"assistant","uuid":"3","parentUuid":"2","message":{"role":"assistant","content":[{"type":"text","text":"b"}]}})),
            // Une branche abandonnée (reprise à un point antérieur) : hors du fil.
            l(json!({"type":"assistant","uuid":"9","parentUuid":"1","message":{"role":"assistant","content":[{"type":"text","text":"x"}]}})),
            l(json!({"type":"assistant","uuid":"8","parentUuid":"3","isSidechain":true,"message":{"role":"assistant","content":[]}})),
            l(json!({"type":"user","uuid":"4","parentUuid":"3","message":{"role":"user","content":"c"}})),
            "pas du json".to_string(),
        ]
        .join("\n");
        let fil = messages_du_fil(&texte);
        let uuids: Vec<&str> = fil.iter().filter_map(|m| m["uuid"].as_str()).collect();
        assert_eq!(uuids, ["1", "3", "4"]);
    }
}
