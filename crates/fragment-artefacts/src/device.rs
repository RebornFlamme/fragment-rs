// Port de app/src/ui/core/artefacts/device.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Identité locale (§13, §14.1), jamais synchronisée.
//
//  - `deviceId` : valeur aléatoire créée une seule fois dans userData/device.json,
//    en dehors du coffre ; chaque machine a donc son propre dossier de journal.
//  - session : numéro propre à chaque processus qui écrit (fenêtre principale,
//    réglages…). L'écrivain est le couple des deux, par exemple `k3f9-b71`.
//  - journal local : userData/artefacts/<coffre>/journal/, au format du journal du
//    coffre, réservé aux records `local: true` (§14.1).
//
//  ★ Sans userData (tests, headless), tout se replie en mémoire : un identifiant
//    de processus, un journal local en mémoire (fs.rs). Rien ne casse.
//    Les adaptateurs `LocalFsAdapter` / `MemoryAdapter` du TS sont dans fs.rs.
// ═══════════════════════════════════════════════════════════════════════════

use std::path::{Path, PathBuf};

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// `n` caractères base 36 tirés au hasard (`crypto.getRandomValues`, via le
/// générateur d'`uuid` : 16 octets aléatoires par tirage).
pub fn random_id(n: usize) -> String {
    let mut out = String::with_capacity(n);
    while out.len() < n {
        for b in uuid::Uuid::new_v4().as_bytes() {
            if out.len() == n {
                break;
            }
            out.push(ALPHABET[(*b as usize) % ALPHABET.len()] as char);
        }
    }
    out
}

/// `crypto.randomUUID()` : l'id d'un record créé sans id.
pub fn random_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Lit (ou crée) l'identifiant de l'appareil dans `<userData>/device.json`.
/// Synchrone : l'estampille en a besoin dès le premier `set`.
pub fn read_device_id(user_data_dir: Option<&Path>) -> String {
    let Some(dir) = user_data_dir else {
        return random_id(4);
    };
    let file = dir.join("device.json");
    if let Ok(text) = std::fs::read_to_string(&file)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(id) = v.get("deviceId").and_then(|d| d.as_str())
        && !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
    {
        return id.to_string();
    }
    // absent ou illisible : on le crée
    let device_id = random_id(4);
    let written = std::fs::create_dir_all(dir).and_then(|_| {
        std::fs::write(
            &file,
            serde_json::json!({ "deviceId": device_id }).to_string(),
        )
    });
    // device.json non écrit : identifiant de session seulement (le TS l'écrit en console)
    let _ = written;
    device_id
}

/// Une clé de dossier stable et lisible pour un coffre : `<nom>-<fnv1a>`.
/// ★ FNV-1a sur les unités UTF-16 du chemin (`charCodeAt`), comme le TS : la même
///   clé des deux côtés pour un même coffre.
pub fn vault_key(vault_path: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for unit in vault_path.encode_utf16() {
        h ^= u32::from(unit);
        h = h.wrapping_mul(0x0100_0193);
    }
    let name = vault_path
        .split(['\\', '/'])
        .rfind(|s| !s.is_empty())
        .unwrap_or("coffre");
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}-{h:08x}")
}

/// Le dossier du journal local d'un coffre (les records `local: true`), ou `None`
/// sans userData.
pub fn local_root(user_data_dir: Option<&Path>, vault_path: Option<&str>) -> Option<PathBuf> {
    let dir = user_data_dir?;
    let vault = vault_path?;
    Some(dir.join("artefacts").join(vault_key(vault)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_id_a_la_bonne_longueur_et_l_alphabet_base_36() {
        let id = random_id(4);
        assert_eq!(id.len(), 4);
        assert!(id.bytes().all(|b| ALPHABET.contains(&b)));
        assert_ne!(random_id(8), random_id(8));
    }

    #[test]
    fn vault_key_est_stable_et_lisible() {
        // FNV-1a de « abc » (32 bits) = 0x1a47e90b.
        assert_eq!(vault_key("abc"), "abc-1a47e90b");
        assert_eq!(
            vault_key("C:\\Users\\julie\\Mon coffre/"),
            format!(
                "Mon_coffre-{}",
                &vault_key("C:\\Users\\julie\\Mon coffre/")[11..]
            )
        );
        assert_eq!(
            local_root(Some(Path::new("/ud")), Some("/v/notes")),
            Some(
                PathBuf::from("/ud")
                    .join("artefacts")
                    .join(vault_key("/v/notes"))
            )
        );
        assert_eq!(local_root(None, Some("/v")), None);
    }

    #[test]
    fn read_device_id_cree_puis_relit_le_meme() {
        let dir =
            std::env::temp_dir().join(format!("fragment-artefacts-device-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = read_device_id(Some(&dir));
        let b = read_device_id(Some(&dir));
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
        std::fs::write(dir.join("device.json"), r#"{"deviceId":"K3F9"}"#).unwrap(); // majuscules : refusé, recréé
        assert_ne!(read_device_id(Some(&dir)), "K3F9");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
