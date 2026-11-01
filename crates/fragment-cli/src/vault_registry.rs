// Port de app/src/shared/cli/vaultRegistry.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LES COFFRES CONNUS, lus à froid — « quels coffres existent ? »
//
//  C'est la seconde question du CLI, après « qui tourne ? » (`instance_index`).
//  Quand aucune instance ne tourne, le client doit savoir QUEL coffre lancer :
//  celui que `vault=` nomme, ou le dernier ouvert.
//
//  La source est le fichier de configuration du processus principal,
//  `<userData>/config.json` : `vaultRoot` (le courant) et
//  `vaults: [{ path, lastOpened }]`. C'est du JSON lisible, sans chiffrement.
//
//  ★ LECTURE SEULE. Le registre de l'app reste le seul écrivain : ce module ne
//    fait que déclarer la forme une seconde fois, pour un processus qui n'a pas
//    l'app. Si la forme change là-bas, le test unitaire d'ici le dira.
// ═══════════════════════════════════════════════════════════════════════════

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::nom_du_dossier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoffreConnu {
    pub path: String,
    /// Le nom du dossier — jamais stocké, toujours dérivé (même règle que le registre de l'app).
    pub name: String,
    pub last_opened: u64,
    pub current: bool,
}

/// Lit `<userData>/config.json`. Fichier absent ou illisible → liste vide ; les
/// entrées malformées sont ignorées ; tri du plus récent au plus ancien.
pub fn lire_coffres_connus(user_data: &Path) -> Vec<CoffreConnu> {
    let Ok(texte) = fs::read_to_string(user_data.join("config.json")) else {
        return Vec::new();
    };
    let Ok(brut) = serde_json::from_str::<Value>(&texte) else {
        return Vec::new();
    };
    let courant = brut.get("vaultRoot").and_then(Value::as_str);
    let Some(entrees) = brut.get("vaults").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut coffres: Vec<CoffreConnu> = entrees
        .iter()
        .filter_map(|e| {
            let path = e.get("path")?.as_str()?;
            // `typeof lastOpened === 'number'` ; sinon 0. Un négatif n'a pas de sens : 0.
            let last_opened = e
                .get("lastOpened")
                .and_then(Value::as_f64)
                .map_or(0, |n| n.max(0.0) as u64);
            Some(CoffreConnu {
                path: path.to_string(),
                name: nom_du_dossier(path),
                last_opened,
                current: Some(path) == courant,
            })
        })
        .collect();
    // ★ tri STABLE (comme Array.sort moderne) : à date égale, l'ordre du fichier reste.
    coffres.sort_by_key(|c| std::cmp::Reverse(c.last_opened));
    coffres
}

/// Même comparaison que pour les instances : nom du dossier ou chemin, casse ignorée.
pub fn meme_coffre(path: &str, name: &str, voulu: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    name.to_lowercase() == voulu.to_lowercase() || norm(path) == norm(voulu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::DossierTemp;

    fn user_data_avec(config: &serde_json::Value) -> DossierTemp {
        let dir = DossierTemp::new("vr-");
        fs::write(dir.chemin().join("config.json"), config.to_string()).unwrap();
        dir
    }

    // ── lire_coffres_connus — la forme du fichier de config, lue à froid ──

    #[test]
    fn lire_coffres_connus_lit_vault_root_et_vaults_derive_le_nom_trie_du_plus_recent_au_plus_ancien()
     {
        let ud = user_data_avec(&serde_json::json!({
            "vaultRoot": "C:\\coffres\\B",
            "vaults": [
                { "path": "C:\\coffres\\A", "lastOpened": 1 },
                { "path": "C:\\coffres\\B", "lastOpened": 2 },
            ],
        }));
        let c = lire_coffres_connus(ud.chemin());
        let noms: Vec<_> = c.iter().map(|k| k.name.as_str()).collect();
        assert_eq!(noms, ["B", "A"]);
        assert!(c[0].current);
        assert!(!c[1].current);
    }

    #[test]
    fn lire_coffres_connus_ignore_les_entrees_malformees_et_survit_a_un_fichier_absent() {
        let ud = user_data_avec(&serde_json::json!({
            "vaults": [{ "path": "C:\\ok" }, { "nope": 1 }, "x", null],
        }));
        let noms: Vec<_> = lire_coffres_connus(ud.chemin())
            .into_iter()
            .map(|k| k.name)
            .collect();
        assert_eq!(noms, ["ok"]);
        assert!(lire_coffres_connus(&ud.chemin().join("inexistant")).is_empty());
    }

    // ── meme_coffre ──────────────────────────────────────────────────────

    #[test]
    fn meme_coffre_compare_par_nom_ou_par_chemin_casse_et_separateurs_ignores() {
        let (path, name) = (r"C:\Users\x\Mes Notes", "Mes Notes");
        assert!(meme_coffre(path, name, "mes notes"));
        assert!(meme_coffre(path, name, "c:/users/x/mes notes/"));
        assert!(!meme_coffre(path, name, "Notes"));
    }

    // ── Ajouts du port ───────────────────────────────────────────────────

    #[test]
    fn un_json_illisible_rend_une_liste_vide() {
        let ud = DossierTemp::new("vr-");
        fs::write(ud.chemin().join("config.json"), "{pas du json").unwrap();
        assert!(lire_coffres_connus(ud.chemin()).is_empty());
    }
}
