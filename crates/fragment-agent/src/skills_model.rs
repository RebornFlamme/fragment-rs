// Port de app/src/ui/views/agent/skillsModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les skills pédagogiques de l'agent — la logique PURE.
//
//  Les skills (`skills/skills/<nom>/SKILL.md` + `references/`) et le manifeste
//  (`skills/.claude-plugin/plugin.json`) forment un PLUGIN LOCAL de Claude Code.
//  Ils sont embarqués dans le binaire, puis recopiés en miroir dans
//  `<userData>/agent/skills-plugin/` : le binaire natif de Claude Code ne lit
//  pas dans l'archive de l'app.
//
//  ★ Noms côté SDK : `<plugin>:<skill>`. La session reçoit une liste nominative
//    (`skills: [...]`) ; la valeur `'all'` ferait entrer les skills fournis avec
//    Claude Code (design, code-review…), vérifié sur le vrai SDK.
//  ★ Sans outils (`tools: []`, cas du « Ask » du lecteur), on n'ajoute ni plugin ni
//    skills. Avec une liste d'outils limitée, il faut y mettre `Skill` : sinon
//    cet outil manque à la session (vérifié).
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde_json::Value;

use crate::motif::Motif;

/// Un fichier du plugin : chemin RELATIF à sa racine (séparateur `/`) → contenu.
pub type PluginFiles = BTreeMap<String, String>;

pub const MANIFEST_PATH: &str = ".claude-plugin/plugin.json";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurSkills {
    #[error("plugin.json sans `name`")]
    ManifesteSansNom,
    #[error("plugin.json illisible : {0}")]
    ManifesteIllisible(String),
}

/// Les fichiers du plugin à partir des fichiers embarqués (clés du type
/// `./skills/skills/<nom>/SKILL.md`) et du manifeste.
///
/// `prefix` : le préfixe à retirer (la racine du plugin) ; `"./skills/"` par défaut côté TS.
pub fn plugin_files(glob: &BTreeMap<String, String>, manifest: &str, prefix: &str) -> PluginFiles {
    let mut files = PluginFiles::new();
    files.insert(MANIFEST_PATH.to_string(), manifest.to_string());
    for (cle, contenu) in glob {
        if let Some(reste) = cle.strip_prefix(prefix) {
            files.insert(reste.to_string(), contenu.clone());
        }
    }
    files
}

/// Le nom du plugin, lu dans son manifeste.
pub fn plugin_name(manifest: &str) -> Result<String, ErreurSkills> {
    let v: Value = serde_json::from_str(manifest)
        .map_err(|e| ErreurSkills::ManifesteIllisible(e.to_string()))?;
    match v.get("name").and_then(Value::as_str) {
        Some(n) if !n.is_empty() => Ok(n.to_string()),
        _ => Err(ErreurSkills::ManifesteSansNom),
    }
}

static CHEMIN_SKILL: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"^skills/([^/]+)/SKILL\.md$"));

/// Les noms QUALIFIÉS (`<plugin>:<skill>`) des skills : un `skills/<nom>/SKILL.md` chacun, triés.
pub fn skill_names(files: &PluginFiles, plugin: &str) -> Vec<String> {
    let mut noms: Vec<String> = files
        .keys()
        .filter_map(|c| {
            CHEMIN_SKILL
                .captures(c)
                .map(|m| format!("{plugin}:{}", &m[1]))
        })
        .collect();
    noms.sort();
    noms
}

/// Le fichier (à la racine du dossier) qui retient l'empreinte de ce que l'app a livré.
pub const LIVRE_PATH: &str = ".livre.json";

/// L'empreinte d'un contenu (FNV-1a 32 bits, hexadécimal) — reconnaître une version LIVRÉE.
/// ★ Calculée sur les unités UTF-16 (`charCodeAt`), comme en TS : les empreintes déjà
///   écrites dans `.livre.json` par Fragment restent valables.
pub fn empreinte(s: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for u in s.encode_utf16() {
        h ^= u32::from(u);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// Ce que l'app doit écrire dans le dossier de l'agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanMiroir {
    /// Les chemins à écrire (triés).
    pub write: Vec<String>,
    /// Les empreintes de cette livraison.
    pub livre: BTreeMap<String, String>,
}

/// Ce que l'app doit écrire dans le dossier de l'agent — SON dossier : il y
/// corrige ses consignes et ses skills (décision utilisateur). Une version
/// LIVRÉE écrase, mais seulement quand elle CHANGE (mise à jour de l'app) : un
/// fichier livré est écrit s'il manque ou si sa version livrée diffère de la
/// dernière livrée (`livre`, empreintes) ; sinon la version de l'agent reste.
/// Rien n'est jamais retiré : un skill AJOUTÉ par l'agent vit.
pub fn plan_mirror(
    files: &PluginFiles,
    existing: &PluginFiles,
    livre: &BTreeMap<String, String>,
) -> PlanMiroir {
    let mut nouveau = BTreeMap::new();
    let mut write = Vec::new();
    // BTreeMap itère déjà trié (le `.sort()` du TS).
    for (p, contenu) in files {
        let h = empreinte(contenu);
        let manque = !existing.contains_key(p);
        if manque || (livre.get(p) != Some(&h) && existing.get(p) != Some(contenu)) {
            write.push(p.clone());
        }
        nouveau.insert(p.clone(), h);
    }
    PlanMiroir {
        write,
        livre: nouveau,
    }
}

static VARIABLE: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"\{\{([A-Za-z0-9_]+)\}\}"));

/// Le prompt d'un fichier (`prompt.md`) : ses variables `{{nom}}` remplacées. Pur.
pub fn render_prompt(template: &str, vars: &BTreeMap<String, String>) -> String {
    let rendu = match VARIABLE.regex() {
        Some(re) => re
            .replace_all(template, |c: &regex::Captures<'_>| match vars.get(&c[1]) {
                Some(v) => v.clone(),
                None => c[0].to_string(),
            })
            .into_owned(),
        None => template.to_string(),
    };
    rendu.trim().to_string()
}

/// Les skills accompagnent-ils ce tour ? Oui, sauf un profil explicitement SANS outils.
pub fn skills_enabled(tools: Option<&[String]>) -> bool {
    tools.is_none_or(|t| !t.is_empty())
}

/// Une liste d'outils restreinte, complétée de `Skill` (sans doublon).
pub fn with_skill_tool(tools: &[String]) -> Vec<String> {
    let mut v = tools.to_vec();
    if !v.iter().any(|t| t == "Skill") {
        v.push("Skill".to_string());
    }
    v
}

/// Le prompt du chat, suivi du préprompt pédagogique en section « Pédagogie ».
pub fn with_pedagogy(base: &str, preprompt: &str) -> String {
    let p = preprompt.trim();
    if p.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n# Pédagogie\n\n{p}")
    }
}

/// Le prompt système d'un tour : TOUJOURS le prompt Fragment (+ la pédagogie quand
/// les skills sont là) ; le profil d'une session (Ask) s'y AJOUTE — il ne le
/// remplace plus (un fil Ask ignorait qu'il était dans Fragment, ses skills, ses
/// consignes de pages). Comme les outils : un fil est l'agent complet.
pub fn compose_system(base: &str, preprompt: Option<&str>, profile: Option<&str>) -> String {
    let avec_pedagogie = match preprompt {
        None => base.to_string(),
        Some(p) => with_pedagogy(base, p),
    };
    match profile.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => format!("{avec_pedagogie}\n\n# Cette conversation\n\n{p}"),
        None => avec_pedagogie,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{"name":"fragment-explain","version":"0.1.0"}"#;

    fn m(paires: &[(&str, &str)]) -> BTreeMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn livre(f: &[(&str, &str)]) -> BTreeMap<String, String> {
        f.iter()
            .map(|(k, v)| (k.to_string(), empreinte(v)))
            .collect()
    }

    // ── pluginFiles / skillNames ──

    #[test]
    fn ramene_les_chemins_vite_a_la_racine_du_plugin_et_ajoute_le_manifeste() {
        let files = plugin_files(
            &m(&[
                ("./skills/skills/a/SKILL.md", "A"),
                ("./skills/skills/a/references/r.md", "R"),
                ("./autre/x.md", "X"),
            ]),
            MANIFEST,
            "./skills/",
        );
        assert_eq!(
            files,
            m(&[
                (MANIFEST_PATH, MANIFEST),
                ("skills/a/SKILL.md", "A"),
                ("skills/a/references/r.md", "R")
            ])
        );
        assert_eq!(
            skill_names(&files, &plugin_name(MANIFEST).unwrap()),
            vec!["fragment-explain:a".to_string()]
        );
    }

    #[test]
    fn refuse_un_manifeste_sans_nom() {
        assert_eq!(plugin_name("{}"), Err(ErreurSkills::ManifesteSansNom));
    }

    // ── planMirror ──

    #[test]
    fn premiere_livraison_tout_ce_qui_manque_est_ecrit_les_empreintes_sont_rendues() {
        let r = plan_mirror(
            &m(&[("a", "1"), ("b", "2")]),
            &PluginFiles::new(),
            &BTreeMap::new(),
        );
        assert_eq!(r.write, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(r.livre, livre(&[("a", "1"), ("b", "2")]));
    }

    #[test]
    fn lagent_a_modifie_un_fichier_livre_la_livraison_na_pas_change_sa_version_reste() {
        let r = plan_mirror(
            &m(&[("a", "1")]),
            &m(&[("a", "amélioré par l’agent")]),
            &livre(&[("a", "1")]),
        );
        assert_eq!(r.write, Vec::<String>::new());
    }

    #[test]
    fn la_livraison_a_change_mise_a_jour_de_lapp_elle_ecrase() {
        let r = plan_mirror(
            &m(&[("a", "2")]),
            &m(&[("a", "amélioré par l’agent")]),
            &livre(&[("a", "1")]),
        );
        assert_eq!(r.write, vec!["a".to_string()]);
    }

    #[test]
    fn rien_nest_jamais_retire_un_skill_ajoute_par_lagent_vit() {
        let r = plan_mirror(
            &m(&[("a", "1")]),
            &m(&[("a", "1"), ("skills/neuf/SKILL.md", "à moi")]),
            &livre(&[("a", "1")]),
        );
        assert_eq!(r.write, Vec::<String>::new());
        assert_eq!(r.livre, livre(&[("a", "1")]));
    }

    #[test]
    fn sans_empreintes_avant_cette_regle_un_fichier_identique_nest_pas_reecrit() {
        assert_eq!(
            plan_mirror(&m(&[("a", "1")]), &m(&[("a", "1")]), &BTreeMap::new()).write,
            Vec::<String>::new()
        );
    }

    // ── renderPrompt ──

    #[test]
    fn remplace_les_variables_connues_garde_les_autres() {
        assert_eq!(
            render_prompt(
                "vault={{vault}} dir={{dir}} {{x}}\n",
                &m(&[("vault", "Coffre"), ("dir", "C:/a")])
            ),
            "vault=Coffre dir=C:/a {{x}}"
        );
    }

    // ── outils et prompt ──

    #[test]
    fn skills_sauf_pour_un_profil_sans_outils() {
        assert!(skills_enabled(None));
        assert!(skills_enabled(Some(&["Read".to_string()])));
        assert!(!skills_enabled(Some(&[])));
    }

    #[test]
    fn une_liste_restreinte_nomme_skill_sans_doublon() {
        assert_eq!(
            with_skill_tool(&["Read".into()]),
            vec!["Read".to_string(), "Skill".to_string()]
        );
        assert_eq!(
            with_skill_tool(&["Read".into(), "Skill".into()]),
            vec!["Read".to_string(), "Skill".to_string()]
        );
    }

    #[test]
    fn le_preprompt_suit_le_prompt_du_chat_en_section_pedagogie() {
        assert_eq!(
            with_pedagogy("BASE", "  # P\n"),
            "BASE\n\n# Pédagogie\n\n# P"
        );
        assert_eq!(with_pedagogy("BASE", "  "), "BASE");
    }

    #[test]
    fn le_profil_dune_session_ask_sajoute_au_prompt_fragment_pedagogie_il_ne_le_remplace_pas() {
        assert_eq!(
            compose_system("BASE", Some("# P"), Some("ASK")),
            "BASE\n\n# Pédagogie\n\n# P\n\n# Cette conversation\n\nASK"
        );
        assert_eq!(
            compose_system("BASE", None, Some("ASK")),
            "BASE\n\n# Cette conversation\n\nASK"
        );
        assert_eq!(
            compose_system("BASE", Some("# P"), None),
            "BASE\n\n# Pédagogie\n\n# P"
        );
        assert_eq!(compose_system("BASE", None, Some("  ")), "BASE");
    }

    #[test]
    fn fnv1a_valeur_connue() {
        // FNV-1a 32 bits de "a" = e40c292c ; de "" = 811c9dc5.
        assert_eq!(empreinte("a"), "e40c292c");
        assert_eq!(empreinte(""), "811c9dc5");
    }
}
