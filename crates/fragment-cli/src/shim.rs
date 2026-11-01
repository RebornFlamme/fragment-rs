// Port de app/src/shared/cli/shim.ts
//
// Pour comprendre le shim, c'est très simple : le binaire de l'app peut être
// lancé de deux façons. Normalement, c'est l'application. Avec
// `ELECTRON_RUN_AS_NODE=1` (côté Fragment/Electron), seul Node démarre, et c'est ce
// Node qui exécute le client `fragment`. Le shim est le petit script (`fragment.cmd`
// ou `fragment`) posé dans le dossier bin de l'utilisateur qui fait ce lancement.
//
// ★ Ici on ne fait que le TEXTE du shim et le calcul du PATH : écrire le fichier et
//   modifier le PATH de l'utilisateur sont des effets de bord de l'hôte (`CliHost::install_shim`).

/// La plateforme du shim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateformeShim {
    Win32,
    Posix,
}

/// Le nom du fichier à écrire dans le dossier bin.
pub fn nom_du_shim(plateforme: PlateformeShim) -> &'static str {
    match plateforme {
        PlateformeShim::Win32 => "fragment.cmd",
        PlateformeShim::Posix => "fragment",
    }
}

/// Échappe un chemin destiné à être placé ENTRE GUILLEMETS dans un `.cmd`.
///
/// Le seul métacaractère encore actif entre guillemets dans un batch est `%`
/// (expansion de variable) : `%SystemRoot%`, ou même un `%` isolé, serait
/// interprété. On le double — `%%` est le littéral `%` en batch. Les autres
/// méta (`&`, `(`, `)`, `^`, `!`) sont inertes entre guillemets tant qu'on
/// n'active pas l'expansion différée ; `"` est illégal dans un chemin Windows,
/// donc les guillemets englobants ne peuvent pas être cassés de l'intérieur.
fn echapper_cmd(chemin: &str) -> String {
    chemin.replace('%', "%%")
}

/// Échappe un chemin destiné à être placé ENTRE GUILLEMETS DOUBLES dans `sh`.
///
/// Entre `"…"`, quatre caractères restent actifs : `\`, `$`, `` ` `` et `"`.
/// On préfixe chacun d'un antislash — un chemin contenant `wei$rd` ou une
/// `` `sous-commande` `` cesse d'être évalué par le shell. Un seul passage sur
/// les caractères : l'antislash littéral du chemin devient `\\` sans
/// double échappement des trois autres.
fn echapper_sh(chemin: &str) -> String {
    let mut sortie = String::with_capacity(chemin.len());
    for c in chemin.chars() {
        if matches!(c, '"' | '$' | '`' | '\\') {
            sortie.push('\\');
        }
        sortie.push(c);
    }
    sortie
}

/// Le texte du shim.
///
/// Windows : un `.cmd`. `%*` transmet tous les arguments tels quels ; les
/// guillemets protègent les espaces des chemins (« Program Files »). Les fins
/// de ligne sont celles de cmd (`\r\n`).
///
/// POSIX : un script sh qui `exec` — le shim ne reste pas en mémoire, le code
/// de sortie du client est celui du shell.
///
/// ★ `exec_path`/`script_path` sont ÉCHAPPÉS avant interpolation (A45) : un chemin
///   d'installation peut contenir `%` (Windows) ou `$`/`` ` ``/`\`/`"` (POSIX),
///   qui casseraient sinon la citation.
pub fn contenu_du_shim(exec_path: &str, script_path: &str, plateforme: PlateformeShim) -> String {
    match plateforme {
        PlateformeShim::Win32 => {
            let exe = echapper_cmd(exec_path);
            let script = echapper_cmd(script_path);
            [
                "@echo off".to_string(),
                "set ELECTRON_RUN_AS_NODE=1".to_string(),
                format!("\"{exe}\" \"{script}\" %*"),
                String::new(),
            ]
            .join("\r\n")
        }
        PlateformeShim::Posix => {
            let exe = echapper_sh(exec_path);
            let script = echapper_sh(script_path);
            [
                "#!/bin/sh".to_string(),
                format!("ELECTRON_RUN_AS_NODE=1 exec \"{exe}\" \"{script}\" \"$@\""),
                String::new(),
            ]
            .join("\n")
        }
    }
}

fn separateur_path(plateforme: PlateformeShim) -> char {
    match plateforme {
        PlateformeShim::Win32 => ';',
        PlateformeShim::Posix => ':',
    }
}

/// Normalise une entrée de PATH pour la comparaison : sans `\` ou `/` finaux,
/// et sans casse sous Windows.
fn normaliser(entree: &str, plateforme: PlateformeShim) -> String {
    let sans_fin = entree.trim_end_matches(['/', '\\']);
    match plateforme {
        PlateformeShim::Win32 => sans_fin.to_lowercase(),
        PlateformeShim::Posix => sans_fin.to_string(),
    }
}

/// Les AUTRES `fragment` que le shell trouverait AVANT le nôtre — le shim est
/// ajouté en FIN de PATH, donc tout `fragment` plus haut gagne. Cas réel : un
/// vieux `npm link` (paquet `app`) vers un dossier disparu dans `%APPDATA%\npm`,
/// qui répondait `Cannot find module …\node_modules\app\dist-cli\fragment.cjs`.
/// On ne les supprime pas (pas nos fichiers) : on les NOMME dans le message.
pub fn chemins_masquants(
    path_actuel: &str,
    bin: &str,
    plateforme: PlateformeShim,
    existe: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let slash = match plateforme {
        PlateformeShim::Win32 => '\\',
        PlateformeShim::Posix => '/',
    };
    // PowerShell préfère `.ps1` à `.cmd` dans un même dossier : tous comptent.
    let noms: &[&str] = match plateforme {
        PlateformeShim::Win32 => &[
            "fragment.ps1",
            "fragment.exe",
            "fragment.cmd",
            "fragment.bat",
        ],
        PlateformeShim::Posix => &["fragment"],
    };
    let bin_norm = normaliser(bin, plateforme);
    let mut trouves = Vec::new();
    for entree in path_actuel.split(separateur_path(plateforme)) {
        if entree.is_empty() {
            continue;
        }
        if normaliser(entree, plateforme) == bin_norm {
            break;
        }
        for nom in noms {
            let chemin = format!("{}{slash}{nom}", entree.trim_end_matches(['/', '\\']));
            if existe(&chemin) {
                trouves.push(chemin);
            }
        }
    }
    trouves
}

/// Le PATH utilisateur avec `bin` en plus — ou `None` s'il y est déjà.
///
/// ★ Comparaison insensible à la casse et aux `\` finaux : sous Windows,
///   `C:\X\bin` et `c:\x\bin\` sont le même dossier, et on ne veut pas l'ajouter
///   trois fois parce qu'il a été écrit trois façons.
///
/// Ajouté EN FIN : un shim ne doit jamais passer devant les outils de
/// l'utilisateur. Les entrées existantes sont conservées telles quelles —
/// y compris les vides et les bizarres, ce n'est pas notre PATH.
pub fn path_avec_bin(path_actuel: &str, bin: &str, plateforme: PlateformeShim) -> Option<String> {
    let sep = separateur_path(plateforme);
    let bin_norm = normaliser(bin, plateforme);
    if path_actuel
        .split(sep)
        .any(|e| !e.is_empty() && normaliser(e, plateforme) == bin_norm)
    {
        return None;
    }
    let base = path_actuel.trim_end_matches(sep);
    Some(if base.is_empty() {
        bin.to_string()
    } else {
        format!("{base}{sep}{bin}")
    })
}

#[cfg(test)]
mod tests {
    use super::PlateformeShim::{Posix, Win32};
    use super::*;

    // ── contenu_du_shim ──────────────────────────────────────────────────

    #[test]
    fn contenu_du_shim_windows_un_cmd_qui_pose_electron_run_as_node_cite_les_chemins_transmet_les_args()
     {
        let s = contenu_du_shim(
            r"C:\Program Files\Fragment\Fragment.exe",
            r"C:\Program Files\Fragment\resources\dist-cli\fragment.cjs",
            Win32,
        );
        assert!(s.contains("set ELECTRON_RUN_AS_NODE=1"));
        assert!(s.contains(
            r#""C:\Program Files\Fragment\Fragment.exe" "C:\Program Files\Fragment\resources\dist-cli\fragment.cjs" %*"#
        ));
        assert_eq!(s.split("\r\n").next(), Some("@echo off"));
        assert_eq!(nom_du_shim(Win32), "fragment.cmd");
    }

    #[test]
    fn contenu_du_shim_posix_un_script_sh_qui_exec_le_code_de_sortie_est_celui_du_client() {
        let s = contenu_du_shim(
            "/Applications/Fragment.app/Contents/MacOS/Fragment",
            "/x/fragment.cjs",
            Posix,
        );
        assert!(s.starts_with("#!/bin/sh\n"));
        assert!(s.contains(
            "ELECTRON_RUN_AS_NODE=1 exec \"/Applications/Fragment.app/Contents/MacOS/Fragment\" \"/x/fragment.cjs\" \"$@\""
        ));
        assert_eq!(nom_du_shim(Posix), "fragment");
    }

    #[test]
    fn contenu_du_shim_windows_un_pourcent_dans_le_chemin_est_double_pas_expanse_a45() {
        let s = contenu_du_shim(
            r"C:\Program Files (x86)\Frag%ment\Fragment.exe",
            r"C:\d\fragment.cjs",
            Win32,
        );
        // Le chemin cité contient le littéral doublé, jamais un %ment% expansible.
        assert!(s.contains(r#""C:\Program Files (x86)\Frag%%ment\Fragment.exe""#));
        assert!(!s.contains("Frag%ment"));
        // `%*` (les arguments) reste intact : on n'échappe que les chemins.
        assert!(s.trim_end().ends_with("%*"));
    }

    #[test]
    fn contenu_du_shim_posix_dollar_backtick_guillemet_et_antislash_sont_echappes_a45() {
        let s = contenu_du_shim("/home/u/wei$rd \"app\"/Fragment", "/o/`sub`/f.cjs", Posix);
        // $, ", ` deviennent inertes entre guillemets doubles.
        assert!(s.contains(r#""/home/u/wei\$rd \"app\"/Fragment""#));
        assert!(s.contains(r#""/o/\`sub\`/f.cjs""#));
        // Aucun $ ni backtick nu ne subsiste dans le corps interpolé.
        assert!(!s.contains("wei$rd"));
        assert!(!s.contains("`sub`"));
    }

    #[test]
    fn contenu_du_shim_posix_un_antislash_litteral_du_chemin_devient_double_sans_sur_echapper_le_reste()
     {
        let s = contenu_du_shim(r"/a\b/Fragment", "/c/f.cjs", Posix);
        assert!(s.contains(r#""/a\\b/Fragment""#));
    }

    // ── path_avec_bin ────────────────────────────────────────────────────

    #[test]
    fn path_avec_bin_ajoute_en_fin_separateur_de_la_plateforme() {
        assert_eq!(
            path_avec_bin(r"C:\a;C:\b", r"C:\bin", Win32).as_deref(),
            Some(r"C:\a;C:\b;C:\bin")
        );
        assert_eq!(
            path_avec_bin("/a:/b", "/home/x/.local/bin", Posix).as_deref(),
            Some("/a:/b:/home/x/.local/bin")
        );
    }

    #[test]
    fn path_avec_bin_rend_none_s_il_y_est_deja_casse_et_antislash_finaux_ignores_sous_windows() {
        assert_eq!(path_avec_bin(r"C:\a;c:\BIN\", r"C:\bin", Win32), None);
        assert_eq!(path_avec_bin("/a:/bin", "/bin", Posix), None);
        // Sous POSIX la casse compte.
        assert!(path_avec_bin("/a:/BIN", "/bin", Posix).is_some());
    }

    #[test]
    fn path_avec_bin_ne_confond_pas_un_prefixe_avec_le_dossier() {
        assert_eq!(
            path_avec_bin(r"C:\binaire", r"C:\bin", Win32).as_deref(),
            Some(r"C:\binaire;C:\bin")
        );
    }

    #[test]
    fn path_avec_bin_un_path_vide_ou_termine_par_un_separateur_ne_produit_pas_de_doublon() {
        assert_eq!(
            path_avec_bin("", r"C:\bin", Win32).as_deref(),
            Some(r"C:\bin")
        );
        assert_eq!(
            path_avec_bin(r"C:\a;", r"C:\bin", Win32).as_deref(),
            Some(r"C:\a;C:\bin")
        );
    }

    #[test]
    fn path_avec_bin_conserve_les_entrees_existantes_telles_quelles_ce_n_est_pas_notre_path() {
        let bizarre = r#"C:\a;;"C:\avec espace";%SystemRoot%\x"#;
        assert_eq!(
            path_avec_bin(bizarre, r"C:\bin", Win32),
            Some(format!(r"{bizarre};C:\bin"))
        );
    }

    // ── chemins_masquants — un autre `fragment` avant le nôtre ───────────

    const BIN: &str = r"C:\Users\u\AppData\Roaming\Fragment\bin";

    #[test]
    fn chemins_masquants_un_vieux_npm_link_plus_haut_dans_le_path_est_nomme_ps1_compris() {
        let npm = r"C:\Users\u\AppData\Roaming\npm";
        let existe = |p: &str| p.starts_with(npm);
        assert_eq!(
            chemins_masquants(&format!(r"C:\Windows;{npm}\;{BIN}"), BIN, Win32, &existe),
            [
                format!(r"{npm}\fragment.ps1"),
                format!(r"{npm}\fragment.exe"),
                format!(r"{npm}\fragment.cmd"),
                format!(r"{npm}\fragment.bat"),
            ]
        );
    }

    #[test]
    fn chemins_masquants_ce_qui_est_apres_notre_dossier_ne_masque_rien_casse_ignoree() {
        let path = format!(r"{};C:\autre", BIN.to_uppercase());
        assert!(chemins_masquants(&path, BIN, Win32, &|_| true).is_empty());
    }

    #[test]
    fn chemins_masquants_notre_dossier_absent_du_path_tout_ce_qui_existe_masque() {
        assert_eq!(
            chemins_masquants("/usr/bin:/opt/x", "/home/u/.local/bin", Posix, &|p| p
                == "/opt/x/fragment"),
            ["/opt/x/fragment"]
        );
    }
}
