// Port de app/src/ui/views/agent/AgentPlugin.ts (`AgentSettingTab`, lignes hors permissions)
// et de app/src/ui/views/agent/harness/claudeSetup.ts (`apiKeySetting`)
//
//! Les réglages généraux de l'onglet Agent : moteur, Claude Code, modèle, nom, orbe, barre, clé.

use std::rc::Rc;

use fragment_agent::harness_claude::MODELE_PAR_DEFAUT;
use fragment_agent::harness_registry::CLAUDE_HARNESS_ID;
use fragment_agent::service::reglages::nom_utilisateur;
use fragment_core::{
    SettingControl, SettingDefinition, SettingDefinitionControl, SettingDropdownControl,
    SettingSecretControl, SettingTextControl,
};
use serde_json::{Value, json};

use super::{ContexteAgent, ORBES, base};

/// La clef de la clé d'API dans le stockage des secrets (la même que celle de Fragment).
pub const CLE_SECRET_API: &str = "agent-anthropic-api-key";

fn ligne(nom: &str, description: &str, control: SettingControl) -> SettingDefinition {
    SettingDefinition::Control(SettingDefinitionControl {
        base: base(None),
        name: nom.into(),
        description: Some(description.into()),
        control,
    })
}

fn liste(
    valeur: &str,
    options: &[(&str, &str)],
    ecrire: impl Fn(&str) + 'static,
) -> SettingControl {
    SettingControl::Dropdown(SettingDropdownControl {
        value: valeur.into(),
        options: options
            .iter()
            .map(|(v, l)| ((*v).to_string(), (*l).to_string()))
            .collect(),
        on_change: Some(Rc::new(move |v: String| ecrire(&v))),
    })
}

/// Pose (ou retire, avec `null`) une clé du `data.json`. Un échec d'écriture n'est pas rendu : le
/// réglage reste affiché tel que relu au rendu suivant, donc l'utilisateur voit qu'il n'a pas pris.
fn poser(ctx: &ContexteAgent, cle: &'static str) -> impl Fn(Value) + 'static {
    let r = ctx.reglages.clone();
    move |v| {
        let _ = r.poser(cle, v);
    }
}

/// `valeur` si elle diffère du défaut, sinon `null` (la clé disparaît du fichier).
fn sauf_defaut(v: &str, defaut: &str) -> Value {
    if v == defaut { Value::Null } else { json!(v) }
}

pub(crate) fn definitions(ctx: &ContexteAgent) -> Vec<SettingDefinition> {
    let r = &ctx.reglages;

    // Le moteur choisi ailleurs (plugin tiers non chargé ici) reste affiché par son id.
    let actuel = r.harness().unwrap_or_else(|| CLAUDE_HARNESS_ID.to_string());
    let mut moteurs = ctx.moteurs.clone();
    if !moteurs.iter().any(|(id, _)| *id == actuel) {
        moteurs.push((actuel.clone(), actuel.clone()));
    }
    let options_moteurs: Vec<(&str, &str)> = moteurs
        .iter()
        .map(|(i, l)| (i.as_str(), l.as_str()))
        .collect();

    let (moteur_ecrit, engine_ecrit) = (poser(ctx, "harness"), poser(ctx, "engine"));
    let (modele_ecrit, nom_ecrit) = (poser(ctx, "model"), poser(ctx, "userName"));
    let (orbe_ecrite, barre_ecrite) = (poser(ctx, "orb"), poser(ctx, "barStart"));

    let mut options_orbe = vec![("auto", "Automatique")];
    options_orbe.extend(ORBES.iter().map(|(_, n, l)| (*n, *l)));

    let secrets = ctx.secrets.clone();
    let nom_affiche = nom_utilisateur(None, ctx.compte.as_deref());
    let barre = r
        .tout()
        .get("barStart")
        .and_then(Value::as_str)
        .unwrap_or("collapsed")
        .to_string();

    vec![
        ligne(
            "Moteur",
            "Le moteur d’une NOUVELLE conversation (Claude, ou celui d’un plugin). Une conversation garde le sien.",
            liste(&actuel, &options_moteurs, move |v| {
                moteur_ecrit(sauf_defaut(v, CLAUDE_HARNESS_ID));
            }),
        ),
        ligne(
            "Claude Code",
            "Le programme qui fait tourner l’agent Claude. Automatique : ton Claude Code s’il est installé et à jour, sinon Fragment télécharge le sien au premier message (une fois).",
            liste(
                &r.moteur().unwrap_or_else(|| "auto".into()),
                &[
                    ("auto", "Automatique"),
                    ("fragment", "Toujours celui de Fragment"),
                ],
                move |v| {
                    engine_ecrit(if v == "fragment" {
                        json!(v)
                    } else {
                        Value::Null
                    })
                },
            ),
        ),
        ligne(
            "Modèle",
            "Le modèle Claude utilisé par l’agent.",
            SettingControl::Text(SettingTextControl {
                value: r.modele(),
                placeholder: Some(MODELE_PAR_DEFAUT.into()),
                on_change: Some(Rc::new(move |v: String| {
                    let v = v.trim();
                    modele_ecrit(json!(if v.is_empty() { MODELE_PAR_DEFAUT } else { v }));
                })),
            }),
        ),
        ligne(
            "Ton nom",
            "Ton nom dans les chats (ton avatar). Vide : celui de ton compte.",
            SettingControl::Text(SettingTextControl {
                value: r.nom_saisi().unwrap_or_default(),
                placeholder: Some(nom_affiche),
                on_change: Some(Rc::new(move |v: String| {
                    let v = v.trim();
                    nom_ecrit(if v.is_empty() { Value::Null } else { json!(v) });
                })),
            }),
        ),
        ligne(
            "Orbe",
            "L’animation qui montre que l’agent travaille. Automatique : elle change avec ce qu’il fait (réfléchir, chercher, écrire, agir).",
            liste(
                &r.orbe().unwrap_or_else(|| "auto".into()),
                &options_orbe,
                move |v| orbe_ecrite(sauf_defaut(v, "auto")),
            ),
        ),
        ligne(
            "Barre de l’agent",
            "La barre de chat en bas des notes, PDF et pages : repliée en pastille, ou ouverte, à l’ouverture d’une vue.",
            liste(
                &barre,
                &[("collapsed", "Repliée"), ("open", "Ouverte")],
                move |v| barre_ecrite(if v == "open" { json!(v) } else { Value::Null }),
            ),
        ),
        // ★ Le rendu d'un secret n'écrit rien de lui-même : c'est ce rappel qui met la clé au
        //   trousseau. Un trousseau indisponible laisse le champ vide au rendu suivant.
        ligne(
            "Clé API Anthropic",
            "Facultative : sans clé, l’agent utilise la connexion de Claude Code ou la variable ANTHROPIC_API_KEY.",
            SettingControl::Secret(SettingSecretControl {
                key: CLE_SECRET_API.into(),
                on_change: Some(Rc::new(move |v: String| {
                    let _ = secrets.set(CLE_SECRET_API, v.trim());
                })),
            }),
        ),
    ]
}
