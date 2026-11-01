// Port de app/src/ui/views/agent/actions/actionsModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LES ACTIONS PERSO de la barre de l'agent — la logique PURE.
//
//  L'utilisateur (ou l'agent, par la CLI `agent:action-add`) enregistre des
//  actions : un libellé, une icône, et SOIT une consigne (`prompt` → un Side Chat
//  sur la sélection / la zone, comme une action rapide), SOIT une commande de l'app
//  (`command`, ex. `editor:toggle-highlight`). `on` restreint aux déclencheurs
//  (texte sélectionné, zone entourée, zone encadrée), `only` aux extensions de
//  document (`md`, `pdf`… ; une conversation : `chat`). Elles vivent dans le
//  data.json de l'agent (réglage `customActions`) et s'ouvrent dans un menu ✨ de
//  la barre, section `actions` — avant les suggestions de l'IA.
// ═══════════════════════════════════════════════════════════════════════════

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::motif::Motif;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionTrigger {
    Selection,
    Lasso,
    LassoRect,
}

pub const ACTION_TRIGGERS: [ActionTrigger; 3] = [
    ActionTrigger::Selection,
    ActionTrigger::Lasso,
    ActionTrigger::LassoRect,
];
pub const ACTION_FALLBACK_ICON: &str = "sparkles";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAction {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// Une consigne : le premier message d'un Side Chat sur la sélection / la zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Ou une commande de l'app (son id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Où l'action apparaît ; absent = partout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Vec<ActionTrigger>>,
    /// Les extensions de document (`md`, `pdf`, `chat`…) ; absent = tous.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
}

/// Le contexte d'une barre : ce qui est sélectionné / entouré, et le document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContexteBarre<'a> {
    /// `selection` ou `lasso`.
    pub kind: &'a str,
    /// `lasso` ou `lasso-rect` (une zone seulement).
    pub shape: Option<&'a str>,
    pub doc: &'a str,
}

/// Le déclencheur d'une barre : texte sélectionné, zone entourée ou encadrée.
pub fn declencheur(kind: &str, shape: Option<&str>) -> ActionTrigger {
    if kind == "selection" {
        return ActionTrigger::Selection;
    }
    if shape == Some("lasso-rect") {
        ActionTrigger::LassoRect
    } else {
        ActionTrigger::Lasso
    }
}

static SCHEMA: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"(?i)^([a-z][a-z0-9+.-]*):"));

/// L'« extension » d'une clé de document : celle du fichier, ou le schéma (`chat:…` → `chat`).
pub fn extension_de(doc: &str) -> String {
    if let Some(m) = SCHEMA.captures(doc) {
        return m[1].to_lowercase();
    }
    let nom = doc.rsplit('/').next().unwrap_or("");
    match nom.rfind('.') {
        Some(i) if i > 0 => nom[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Les actions valables pour cette barre, dans l'ordre enregistré.
pub fn actions_pour(
    actions: &[CustomAction],
    ctx: &ContexteBarre<'_>,
    side_chat: bool,
) -> Vec<CustomAction> {
    let t = declencheur(ctx.kind, ctx.shape);
    let ext = extension_de(ctx.doc);
    actions
        .iter()
        .filter(|a| {
            a.on.as_ref().is_none_or(|o| o.is_empty() || o.contains(&t))
                && a.only
                    .as_ref()
                    .is_none_or(|o| o.is_empty() || o.contains(&ext))
                && (a.command.is_some()
                    || (side_chat && a.prompt.as_deref().is_some_and(|p| !p.is_empty())))
        })
        .cloned()
        .collect()
}

static NON_ALNUM: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"[^a-z0-9]+"));

/// Un id lisible tiré du libellé (sans accents, minuscules, tirets).
pub fn id_de(label: &str) -> String {
    // NFD puis on retire les diacritiques combinants (U+0300–U+036F).
    let sans: String = label
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect::<String>()
        .to_lowercase();
    let tirets = NON_ALNUM.replace_all(&sans, "-");
    let s = tirets.trim_matches('-');
    if s.is_empty() {
        "action".to_string()
    } else {
        s.to_string()
    }
}

/// Une liste séparée par des virgules → ses éléments (vides retirés).
pub fn liste(v: Option<&str>) -> Option<Vec<String>> {
    let l: Vec<String> = v
        .unwrap_or("")
        .split(',')
        .map(|x| {
            x.trim()
                .strip_prefix('.')
                .unwrap_or(x.trim())
                .to_lowercase()
        })
        .filter(|x| !x.is_empty())
        .collect();
    if l.is_empty() { None } else { Some(l) }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionBrute {
    pub id: Option<String>,
    pub label: Option<String>,
    pub icon: Option<String>,
    pub prompt: Option<String>,
    pub command: Option<String>,
    pub on: Option<Vec<String>>,
    pub only: Option<Vec<String>>,
}

/// Ce que l'app sait : une icône existe-t-elle, une commande existe-t-elle.
pub struct DepsValidation<'a> {
    pub icone_existe: &'a dyn Fn(&str) -> bool,
    pub commande_existe: &'a dyn Fn(&str) -> bool,
}

/// L'action validée, avec un éventuel avertissement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionValidee {
    pub action: CustomAction,
    pub avertissement: Option<String>,
}

/// Valide une action à enregistrer. Une icône inconnue retombe sur `sparkles`
/// (signalé), une commande inconnue est une erreur (`Err(message)`).
pub fn valider(a: &ActionBrute, deps: &DepsValidation<'_>) -> Result<ActionValidee, String> {
    let label = a.label.as_deref().unwrap_or("").trim().to_string();
    if label.is_empty() {
        return Err("Il faut un libellé (`label=`).".into());
    }
    let prompt = a.prompt.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let command = a
        .command
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if prompt.is_some() == command.is_some() {
        return Err(
            "Il faut SOIT une consigne (`prompt=`), SOIT une commande (`command=`).".into(),
        );
    }
    if let Some(c) = command
        && !(deps.commande_existe)(c)
    {
        return Err(format!(
            "Commande inconnue : « {c} » (voir `fragment commands`)."
        ));
    }
    let on_brut = a.on.as_ref().filter(|o| !o.is_empty());
    let mut on: Option<Vec<ActionTrigger>> = None;
    if let Some(liste) = on_brut {
        let mut v = Vec::new();
        for t in liste {
            match ACTION_TRIGGERS.iter().find(|x| {
                serde_json::to_value(x)
                    .ok()
                    .and_then(|j| j.as_str().map(|s| s == t))
                    .unwrap_or(false)
            }) {
                Some(x) => v.push(*x),
                None => {
                    return Err(format!(
                        "Déclencheur inconnu : « {t} » (selection, lasso, lasso-rect)."
                    ));
                }
            }
        }
        on = Some(v);
    }
    let mut icon = a.icon.as_deref().unwrap_or("").trim().to_string();
    if icon.is_empty() {
        icon = ACTION_FALLBACK_ICON.into();
    }
    let mut avertissement = None;
    if !(deps.icone_existe)(&icon) {
        avertissement = Some(format!(
            "Icône inconnue : « {icon} » → « {ACTION_FALLBACK_ICON} » (noms Lucide : lightbulb, list, book-open, languages, highlighter, search, sigma, quote…)."
        ));
        icon = ACTION_FALLBACK_ICON.into();
    }
    let id = match a.id.as_deref() {
        Some(i) if !i.trim().is_empty() => id_de(i),
        _ => id_de(&label),
    };
    let action = CustomAction {
        id,
        label,
        icon,
        prompt: prompt.map(str::to_string),
        command: if prompt.is_some() {
            None
        } else {
            command.map(str::to_string)
        },
        on,
        only: a.only.clone().filter(|o| !o.is_empty()),
    };
    Ok(ActionValidee {
        action,
        avertissement,
    })
}

/// Ajoute, ou REMPLACE à sa place celle du même id.
pub fn ajouter(actions: &[CustomAction], a: CustomAction) -> Vec<CustomAction> {
    let mut l = actions.to_vec();
    match l.iter().position(|x| x.id == a.id) {
        Some(i) => l[i] = a,
        None => l.push(a),
    }
    l
}

pub fn retirer(actions: &[CustomAction], id: &str) -> Option<Vec<CustomAction>> {
    if actions.iter().any(|a| a.id == id) {
        Some(actions.iter().filter(|a| a.id != id).cloned().collect())
    } else {
        None
    }
}

/// Déplace une action à `index` (borné). `None` : id inconnu.
pub fn deplacer(actions: &[CustomAction], id: &str, index: i64) -> Option<Vec<CustomAction>> {
    let i = actions.iter().position(|a| a.id == id)?;
    let mut l = actions.to_vec();
    let a = l.remove(i);
    let cible = index.clamp(0, l.len() as i64) as usize;
    l.insert(cible, a);
    Some(l)
}

/// Une ligne lisible (CLI `agent:actions`).
pub fn decrire(a: &CustomAction) -> String {
    let quoi = match (&a.prompt, &a.command) {
        (Some(p), _) => format!("prompt « {p} »"),
        (None, c) => format!("commande {}", c.as_deref().unwrap_or("undefined")),
    };
    let sur = match &a.on {
        Some(o) if !o.is_empty() => {
            let noms: Vec<String> = o
                .iter()
                .filter_map(|t| {
                    serde_json::to_value(t)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                })
                .collect();
            format!("sur {}", noms.join(", "))
        }
        _ => "partout".to_string(),
    };
    let docs = match &a.only {
        Some(o) if !o.is_empty() => format!("docs {}", o.join(", ")),
        _ => String::new(),
    };
    let ou = [sur, docs]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    format!("{}\t{}\t[{}]\t{}\t({})", a.id, a.label, a.icon, quoi, ou)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icone_existe(n: &str) -> bool {
        ["lightbulb", "sparkles", "highlighter"].contains(&n)
    }
    fn commande_existe(id: &str) -> bool {
        id == "editor:toggle-highlight"
    }
    fn deps() -> DepsValidation<'static> {
        DepsValidation {
            icone_existe: &icone_existe,
            commande_existe: &commande_existe,
        }
    }
    fn a(id: &str) -> CustomAction {
        CustomAction {
            id: id.into(),
            label: id.into(),
            icon: "sparkles".into(),
            prompt: Some("p".into()),
            command: None,
            on: None,
            only: None,
        }
    }
    fn ids(l: &[CustomAction]) -> Vec<&str> {
        l.iter().map(|x| x.id.as_str()).collect()
    }
    fn brute(label: &str) -> ActionBrute {
        ActionBrute {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    // ── déclencheur et document ──

    #[test]
    fn selection_zone_entouree_zone_encadree() {
        assert_eq!(declencheur("selection", None), ActionTrigger::Selection);
        assert_eq!(declencheur("lasso", Some("lasso")), ActionTrigger::Lasso);
        assert_eq!(
            declencheur("lasso", Some("lasso-rect")),
            ActionTrigger::LassoRect
        );
    }

    #[test]
    fn extension_dune_cle_fichier_ou_le_schema() {
        assert_eq!(extension_de("Cours/Kakeya.PDF"), "pdf");
        assert_eq!(extension_de("note.md"), "md");
        assert_eq!(extension_de("chat:agent/abc"), "chat");
        assert_eq!(extension_de("sans-extension"), "");
    }

    // ── actionsPour ──

    fn jeu() -> Vec<CustomAction> {
        vec![
            a("expliquer"),
            CustomAction {
                prompt: None,
                command: Some("editor:toggle-highlight".into()),
                on: Some(vec![ActionTrigger::Selection]),
                only: Some(vec!["md".into()]),
                ..a("surligner")
            },
            CustomAction {
                on: Some(vec![ActionTrigger::LassoRect]),
                ..a("cadre")
            },
            CustomAction {
                only: Some(vec!["chat".into()]),
                ..a("chat")
            },
        ]
    }
    fn ctx<'a>(kind: &'a str, shape: Option<&'a str>, doc: &'a str) -> ContexteBarre<'a> {
        ContexteBarre { kind, shape, doc }
    }

    #[test]
    fn filtre_par_declencheur_et_par_document_dans_lordre() {
        let actions = jeu();
        assert_eq!(
            ids(&actions_pour(
                &actions,
                &ctx("selection", None, "a.md"),
                true
            )),
            vec!["expliquer", "surligner"]
        );
        assert_eq!(
            ids(&actions_pour(
                &actions,
                &ctx("selection", None, "a.pdf"),
                true
            )),
            vec!["expliquer"]
        );
        assert_eq!(
            ids(&actions_pour(
                &actions,
                &ctx("lasso", Some("lasso-rect"), "a.md"),
                true
            )),
            vec!["expliquer", "cadre"]
        );
        assert_eq!(
            ids(&actions_pour(
                &actions,
                &ctx("selection", None, "chat:agent/x"),
                true
            )),
            vec!["expliquer", "chat"]
        );
    }

    #[test]
    fn sans_side_chat_possible_seules_les_commandes() {
        assert_eq!(
            ids(&actions_pour(
                &jeu(),
                &ctx("selection", None, "a.md"),
                false
            )),
            vec!["surligner"]
        );
    }

    // ── valider ──

    #[test]
    fn une_consigne_id_tire_du_libelle() {
        let r = valider(
            &ActionBrute {
                icon: Some("lightbulb".into()),
                prompt: Some(" Réécris. ".into()),
                ..brute("Réécrire en français")
            },
            &deps(),
        );
        assert_eq!(
            r,
            Ok(ActionValidee {
                action: CustomAction {
                    id: "reecrire-en-francais".into(),
                    label: "Réécrire en français".into(),
                    icon: "lightbulb".into(),
                    prompt: Some("Réécris.".into()),
                    command: None,
                    on: None,
                    only: None
                },
                avertissement: None,
            })
        );
    }

    #[test]
    fn une_commande_des_declencheurs_des_extensions() {
        let r = valider(
            &ActionBrute {
                icon: Some("highlighter".into()),
                command: Some("editor:toggle-highlight".into()),
                on: Some(vec!["selection".into()]),
                only: Some(vec!["md".into()]),
                ..brute("Surligner")
            },
            &deps(),
        );
        assert_eq!(
            r.map(|v| v.action),
            Ok(CustomAction {
                id: "surligner".into(),
                label: "Surligner".into(),
                icon: "highlighter".into(),
                prompt: None,
                command: Some("editor:toggle-highlight".into()),
                on: Some(vec![ActionTrigger::Selection]),
                only: Some(vec!["md".into()]),
            })
        );
    }

    #[test]
    fn icone_inconnue_sparkles_signale() {
        let r = valider(
            &ActionBrute {
                icon: Some("nope".into()),
                prompt: Some("p".into()),
                ..brute("X")
            },
            &deps(),
        )
        .unwrap();
        assert_eq!(r.action.icon, "sparkles");
        assert!(
            r.avertissement
                .unwrap_or_default()
                .contains("Icône inconnue")
        );
    }

    #[test]
    fn les_erreurs() {
        let p = |x: &str| Some(x.to_string());
        assert!(
            valider(
                &ActionBrute {
                    prompt: p("p"),
                    ..brute("")
                },
                &deps()
            )
            .is_err()
        );
        assert!(valider(&brute("X"), &deps()).is_err());
        assert!(
            valider(
                &ActionBrute {
                    prompt: p("p"),
                    command: p("editor:toggle-highlight"),
                    ..brute("X")
                },
                &deps()
            )
            .is_err()
        );
        assert!(
            valider(
                &ActionBrute {
                    command: p("nope"),
                    ..brute("X")
                },
                &deps()
            )
            .is_err()
        );
        assert!(
            valider(
                &ActionBrute {
                    prompt: p("p"),
                    on: Some(vec!["partout".into()]),
                    ..brute("X")
                },
                &deps()
            )
            .is_err()
        );
    }

    // ── la liste ──

    #[test]
    fn ajouter_remplace_a_sa_place_le_meme_id() {
        let l = ajouter(
            &[a("a"), a("b")],
            CustomAction {
                label: "A2".into(),
                ..a("a")
            },
        );
        assert_eq!(
            l.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(),
            vec!["A2", "b"]
        );
        assert_eq!(ids(&ajouter(&l, a("c"))), vec!["a", "b", "c"]);
    }

    #[test]
    fn retirer_deplacer_borne_ids_inconnus() {
        let l = vec![a("a"), a("b"), a("c")];
        assert_eq!(ids(&retirer(&l, "b").unwrap()), vec!["a", "c"]);
        assert_eq!(retirer(&l, "z"), None);
        assert_eq!(ids(&deplacer(&l, "c", 0).unwrap()), vec!["c", "a", "b"]);
        assert_eq!(ids(&deplacer(&l, "a", 99).unwrap()), vec!["b", "c", "a"]);
        assert_eq!(deplacer(&l, "z", 0), None);
    }

    #[test]
    fn id_de_liste() {
        assert_eq!(id_de("  !!  "), "action");
        assert_eq!(
            liste(Some("md, .PDF,,")),
            Some(vec!["md".to_string(), "pdf".to_string()])
        );
        assert_eq!(liste(Some("")), None);
    }
}
