// Port de app/src/shared/vaultPath.ts
//
// Le garde-fou des chemins du coffre. Il vit dans fragment-cli (program.md §4 : le
// groupe p1-cli porte `vaultPath.ts`) mais ne dépend de rien d'autre ici.
//
// CONVENTION DU PROJET : un chemin de coffre est TOUJOURS relatif à la racine
// et en style posix (`/`), sur toutes les plateformes — y compris Windows.
// La racine elle-même vaut `""`.
//
// ★ Tout est LEXICAL, comme `path.resolve` de Node : aucun accès disque, aucune
//   résolution de lien symbolique. Ce qui borne le chemin, c'est la normalisation
//   des `..` puis le test de préfixe.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Pourquoi un chemin demandé est refusé. Les messages sont ceux de Fragment.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurChemin {
    #[error("chemin invalide (octet nul) : {0:?}")]
    OctetNul(String),
    #[error("chemin absolu refusé : {0}")]
    Absolu(String),
    #[error("chemin relatif à un lecteur refusé : {0}")]
    Lecteur(String),
    #[error("hors du {etiquette} : {demande}")]
    HorsRacine { etiquette: String, demande: String },
}

/// `path.resolve(racine)` : absolu, `.` et `..` résolus, sans toucher au disque.
fn resoudre(chemin: &Path) -> PathBuf {
    let absolu = std::path::absolute(chemin).unwrap_or_else(|_| chemin.to_path_buf());
    normaliser(&absolu)
}

fn normaliser(chemin: &Path) -> PathBuf {
    let mut sortie = PathBuf::new();
    for c in chemin.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => sortie.push(c.as_os_str()),
            Component::CurDir => {}
            // `pop` refuse de remonter au-dessus de la racine : comme Node, on y reste.
            Component::ParentDir => {
                sortie.pop();
            }
            Component::Normal(nom) => sortie.push(nom),
        }
    }
    sortie
}

fn composants(chemin: &Path) -> Vec<OsString> {
    chemin
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect()
}

fn memes_composants(a: &OsString, b: &OsString) -> bool {
    if cfg!(windows) {
        // NTFS ignore la casse (comme `path.relative` de Node sous Windows).
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// L'opération INVERSE de [`resolve_in_vault`] : un chemin absolu du disque redevient
/// un chemin de coffre.
///
/// Deux raisons de ne jamais s'en passer sur ce qui sort du main :
///  - fuite d'information — `C:\Users\julie\Documents\Fragment\note.md` livre le
///    nom d'utilisateur et l'arborescence de la machine au code de plugin tiers ;
///  - correction — les clefs du fileMap sont des chemins de coffre. Un chemin
///    absolu, ou un chemin avec des `\`, ne s'apparie avec rien.
pub fn to_vault_path(vault_root: &Path, absolute_path: &Path) -> String {
    let racine = composants(&resoudre(vault_root));
    let cible = composants(&resoudre(absolute_path));
    let commun = racine
        .iter()
        .zip(&cible)
        .take_while(|(a, b)| memes_composants(a, b))
        .count();
    // Pas même la racine en commun (autre lecteur) : `path.relative` rend la cible.
    let reste: Vec<String> = if commun == 0 {
        cible
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .collect()
    } else {
        std::iter::repeat_n("..".to_string(), racine.len() - commun)
            .chain(
                cible[commun..]
                    .iter()
                    .map(|c| c.to_string_lossy().into_owned()),
            )
            .collect()
    };
    reste.join("/")
}

/// C'est une fonction de sécurité.
pub fn resolve_in_vault(vault_root: &Path, requested: &str) -> Result<PathBuf, ErreurChemin> {
    resolve_inside(vault_root, requested, "coffre")
}

/// Le MÊME garde-fou, pour une autre racine que le coffre.
///
/// Le corps n'a jamais rien eu de spécifique au coffre — il borne un chemin
/// relatif à une racine, point. Ce qui change avec le protocole de ressources,
/// c'est qu'il existe plusieurs racines (le coffre de l'utilisateur, les assets
/// livrés avec l'app) et qu'aucune ne doit pouvoir servir de porte vers l'autre.
/// `etiquette` ne sert qu'aux messages : un « hors du coffre » sur une police
/// de pdf.js enverrait le lecteur chercher au mauvais endroit.
pub fn resolve_inside(
    root: &Path,
    requested: &str,
    etiquette: &str,
) -> Result<PathBuf, ErreurChemin> {
    // 1. Octet nul — faille historique.
    if requested.contains('\0') {
        return Err(ErreurChemin::OctetNul(requested.to_string()));
    }

    // 2. ★ AUCUNE normalisation Unicode. Ce fut une ligne `normalize('NFC')` côté
    //    Fragment, et elle rendait INACCESSIBLE une partie entière d'un vrai coffre.
    //
    //    Un nom de fichier n'est pas du texte à afficher : c'est un IDENTIFIANT,
    //    et sur NTFS comme sur ext4 il est comparé point de code par point de
    //    code. Un dossier « Algèbre » créé sur macOS (ou synchronisé par Drive,
    //    ou par Dropbox) est stocké en NFD — « Alge » + U+0300 + « bre ». Le
    //    recomposer en NFC fabrique une chaîne qui ne désigne plus rien : ENOENT.
    //
    //    Et la normalisation ne protégeait de rien : les gardes portent sur de
    //    l'ASCII (`\0`, `..`, `C:`, le séparateur), que NFC ne peut ni créer ni
    //    effacer. Ce qui borne réellement le chemin, c'est la normalisation des
    //    `..` + le préfixe vérifiés plus bas. Corollaire : la forme du DISQUE
    //    traverse intacte, dans les deux sens (`to_vault_path` ne normalise pas non plus).

    // 3. Chemin absolu refusé. ★ `has_root` : sous Windows, `/etc/passwd` n'est pas
    //    « absolu » pour Rust (pas de lecteur) mais l'est pour Node — on refuse les deux.
    let demande = Path::new(requested);
    if demande.is_absolute() || demande.has_root() {
        return Err(ErreurChemin::Absolu(requested.to_string()));
    }

    // 4. Windows : `D:notes.md` (relatif au dossier courant du lecteur D).
    let octets = requested.as_bytes();
    if octets.len() >= 2 && octets[0].is_ascii_alphabetic() && octets[1] == b':' {
        return Err(ErreurChemin::Lecteur(requested.to_string()));
    }

    let racine = resoudre(root);
    let complet = normaliser(&racine.join(demande));

    // 5. Doit rester sous la racine (comparaison par composants : `/coffre2` n'est
    //    pas sous `/coffre`).
    if !complet.starts_with(&racine) {
        return Err(ErreurChemin::HorsRacine {
            etiquette: etiquette.to_string(),
            demande: requested.to_string(),
        });
    }
    Ok(complet)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Le garde-fou des chemins — et surtout ce qu'il ne doit PAS faire.
    //
    // Ce fichier existe à cause d'un vrai coffre : ses dossiers, créés sur macOS
    // puis synchronisés, portent des noms en NFD (« Algèbre » = « Alge » + U+0300 +
    // « bre »). Une normalisation NFC en entrée les rendait introuvables sur NTFS, et
    // comme la descente échouait, l'app entière restait sur un écran gris.

    fn racine() -> PathBuf {
        resoudre(Path::new(if cfg!(windows) {
            "C:/coffre"
        } else {
            "/coffre"
        }))
    }

    // Les deux orthographes Unicode du MÊME mot pour un lecteur humain, et deux
    // chaînes différentes pour un système de fichiers.
    const NFD: &str = "Alge\u{0300}bre 1";
    const NFC: &str = "Alg\u{00e8}bre 1";

    // ── resolveInside — la forme du disque est préservée ─────────────────

    #[test]
    fn resolve_inside_ne_recompose_pas_un_nom_decompose() {
        // L'assertion qui compte : le segment ressort tel qu'il est entré.
        let r = racine();
        assert_eq!(resolve_inside(&r, NFD, "coffre").unwrap(), r.join(NFD));
        assert_ne!(resolve_inside(&r, NFD, "coffre").unwrap(), r.join(NFC));
    }

    #[test]
    fn resolve_inside_ne_decompose_pas_davantage_un_nom_deja_compose() {
        let r = racine();
        assert_eq!(resolve_inside(&r, NFC, "coffre").unwrap(), r.join(NFC));
    }

    #[test]
    fn resolve_inside_traverse_les_sous_dossiers_sans_toucher_aux_accents() {
        let r = racine();
        let chemin = format!("Supe\u{0301}rieur/{NFD}/note.md");
        assert_eq!(
            resolve_inside(&r, &chemin, "coffre").unwrap(),
            r.join("Supe\u{0301}rieur").join(NFD).join("note.md")
        );
    }

    #[test]
    fn resolve_inside_fait_l_aller_retour_avec_to_vault_path() {
        // C'est le cycle réel : readdir → to_vault_path → IPC → resolve_in_vault.
        let r = racine();
        let absolu = r.join(NFD).join("chapitre.md");
        assert_eq!(
            resolve_inside(&r, &to_vault_path(&r, &absolu), "coffre").unwrap(),
            absolu
        );
    }

    // ── resolveInside — les gardes, eux, tiennent toujours ───────────────

    #[test]
    fn resolve_inside_refuse_l_echappee_par_deux_points() {
        let r = racine();
        for demande in ["../secrets.md", "notes/../../secrets.md"] {
            let e = resolve_inside(&r, demande, "coffre").unwrap_err();
            assert!(e.to_string().contains("hors du coffre"), "{e}");
        }
    }

    #[test]
    fn resolve_inside_refuse_l_octet_nul() {
        let e = resolve_inside(&racine(), "note\0.md", "coffre").unwrap_err();
        assert!(e.to_string().contains("octet nul"), "{e}");
    }

    #[test]
    fn resolve_inside_refuse_un_chemin_absolu_et_un_chemin_de_lecteur() {
        let r = racine();
        let e = resolve_inside(&r, "/etc/passwd", "coffre").unwrap_err();
        assert!(e.to_string().contains("absolu"), "{e}");
        let e = resolve_inside(&r, "D:notes.md", "coffre").unwrap_err();
        assert!(e.to_string().contains("lecteur"), "{e}");
    }

    #[test]
    fn resolve_inside_accepte_la_racine_elle_meme() {
        let r = racine();
        assert_eq!(resolve_inside(&r, "", "coffre").unwrap(), r);
    }

    #[test]
    fn resolve_inside_nomme_la_racine_dont_on_sort() {
        // `etiquette` n'est pas cosmétique : un « hors du coffre » sur une police
        // de pdf.js enverrait chercher au mauvais endroit.
        let e = resolve_inside(&racine(), "../x", "assets").unwrap_err();
        assert!(e.to_string().contains("hors du assets"), "{e}");
    }

    // ── Ajouts du port ───────────────────────────────────────────────────

    #[test]
    fn un_dossier_frere_au_prefixe_commun_n_est_pas_sous_la_racine() {
        let r = racine();
        let nom = r.file_name().unwrap().to_string_lossy().into_owned();
        let e = resolve_inside(&r, &format!("../{nom}2/x"), "coffre").unwrap_err();
        assert!(matches!(e, ErreurChemin::HorsRacine { .. }));
    }

    #[test]
    fn to_vault_path_est_relatif_en_style_posix_et_vaut_vide_pour_la_racine() {
        let r = racine();
        assert_eq!(to_vault_path(&r, &r), "");
        assert_eq!(to_vault_path(&r, &r.join("a").join("b.md")), "a/b.md");
    }
}
