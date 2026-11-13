// Port de app/src/ui/views/agent/AgentPlugin.ts (`AgentSettingTab.permissions`)
//
//! Les permissions de l'agent, réglées en déclaratif : le mode par défaut, trois préréglages qui
//! remplissent la matrice, un accès par famille d'outils, les commandes permises, les domaines
//! web, l'arbre des dossiers du coffre, les dossiers hors coffre, les « toujours » retenus.
//!
//! ★ Chaque changement relit la politique COMPLÈTE, y change une pièce et la réécrit : les
//!   éléments que cet onglet n'affiche pas ne se perdent pas.
//! ★ Un préréglage ne remplace que le mode et la matrice des outils : dossiers, commandes,
//!   domaines et « toujours » de l'utilisateur restent.
//! ★ En tête, ce que la configuration ne protège PAS (`incoherences`) : honnêteté, pas alarme.

use std::rc::Rc;

use fragment_agent::contrat::{
    AgentPolicy, DossierExterne, FolderAccess, PermanentRule, PermissionMode, ToolAccess,
    ToolFamily, WebDomains,
};
use fragment_agent::policy_model::{
    PolicyPreset, TOOL_FAMILIES, family_label, incoherences, mode_label,
};
use fragment_core::{
    SettingControl, SettingDefinition, SettingDefinitionAction, SettingDefinitionControl,
    SettingDefinitionEmpty, SettingDefinitionGroup, SettingDefinitionItem, SettingDefinitionList,
    SettingDefinitionTree, SettingDropdownControl, SettingExtraButtonControl, SettingGroupItem,
    SettingTextControl, SettingToggleControl, SettingTreeNode,
};

use super::{ContexteAgent, base};

/// Les trois préréglages, dans l'ordre du menu, avec leur description.
pub const PREREGLAGES: [(PolicyPreset, &str, &str); 3] = [
    (
        PolicyPreset::Open,
        "open",
        "Tout le coffre, le terminal et le CLI ; ni le web ni les sous-agents.",
    ),
    (
        PolicyPreset::Assistant,
        "assistant",
        "Lire et chercher librement ; écrire, le terminal et le web sont demandés.",
    ),
    (
        PolicyPreset::Strict,
        "strict",
        "Ni terminal ni JavaScript ni web ; écrire est demandé.",
    ),
];

/// Les modes, dans l'ordre du menu.
const MODES: [PermissionMode; 4] = [
    PermissionMode::Full,
    PermissionMode::Ask,
    PermissionMode::AcceptEdits,
    PermissionMode::Plan,
];

/// L'écriture d'une valeur de la politique dans le `data.json` (`allow`, `acceptEdits`…).
///
/// ★ Un trait local plutôt que `serde` : fragment-ui ne dépend pas de `serde`, et les valeurs
///   sont quatre énumérations fermées dont l'écriture est celle du contrat de l'agent.
trait Texte: Sized + Copy + 'static {
    const TOUS: &'static [Self];
    fn texte(self) -> &'static str;
}

impl Texte for PermissionMode {
    const TOUS: &'static [Self] = &[Self::Full, Self::Ask, Self::AcceptEdits, Self::Plan];
    fn texte(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Ask => "ask",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
        }
    }
}

impl Texte for ToolAccess {
    const TOUS: &'static [Self] = &[Self::Allow, Self::Ask, Self::Deny];
    fn texte(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

impl Texte for FolderAccess {
    const TOUS: &'static [Self] = &[Self::Hidden, Self::Read, Self::Write];
    fn texte(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

fn en_texte<T: Texte>(v: &T) -> String {
    v.texte().to_owned()
}

fn depuis_texte<T: Texte>(s: &str) -> Option<T> {
    T::TOUS.iter().copied().find(|v| v.texte() == s)
}

/// Le texte d'une famille d'outils (`web-fetch`), celui du contrat.
fn famille_en_texte(f: ToolFamily) -> &'static str {
    match f {
        ToolFamily::Read => "read",
        ToolFamily::Search => "search",
        ToolFamily::Write => "write",
        ToolFamily::Edit => "edit",
        ToolFamily::Shell => "shell",
        ToolFamily::Eval => "eval",
        ToolFamily::FragmentCli => "fragment-cli",
        ToolFamily::WebFetch => "web-fetch",
        ToolFamily::WebSearch => "web-search",
        ToolFamily::Subagents => "subagents",
        ToolFamily::Skills => "skills",
    }
}

/// Relit la politique, y applique `f` et l'écrit.
fn modifier(ctx: &ContexteAgent, f: impl FnOnce(&mut AgentPolicy)) {
    let mut p = ctx.reglages.politique();
    f(&mut p);
    // Un échec d'écriture reste visible : le rendu suivant relit l'ancienne valeur.
    let _ = ctx.reglages.poser_politique(&p);
}

fn retirer(on_click: impl Fn() + 'static) -> SettingControl {
    SettingControl::ExtraButton(SettingExtraButtonControl {
        icon: "trash-2".into(),
        tooltip: Some("Retirer".into()),
        on_click: Rc::new(on_click),
    })
}

fn liste_deroulante(
    valeur: String,
    options: Vec<(String, String)>,
    ecrire: impl Fn(String) + 'static,
) -> SettingControl {
    SettingControl::Dropdown(SettingDropdownControl {
        value: valeur,
        options,
        on_change: Some(Rc::new(ecrire)),
    })
}

fn ligne_controle(
    id: Option<&str>,
    nom: &str,
    description: Option<&str>,
    control: SettingControl,
) -> SettingDefinitionControl {
    SettingDefinitionControl {
        base: base(id),
        name: nom.into(),
        description: description.map(str::to_owned),
        control,
    }
}

fn ligne_simple(nom: &str, controls: Vec<SettingControl>) -> SettingGroupItem {
    SettingGroupItem::Item(SettingDefinitionItem {
        base: base(None),
        name: nom.into(),
        description: None,
        controls,
    })
}

/// La ligne « Ajouter » : un champ dont le brouillon survit aux rendus, et un bouton `+`.
fn ligne_ajout(
    id: &str,
    nom: &str,
    description: Option<&str>,
    placeholder: &str,
    brouillon: Rc<dyn Fn(String)>,
    ajouter: Rc<dyn Fn()>,
) -> SettingGroupItem {
    SettingGroupItem::Item(SettingDefinitionItem {
        base: base(Some(id)),
        name: nom.into(),
        description: description.map(str::to_owned),
        controls: vec![
            SettingControl::Text(SettingTextControl {
                value: String::new(),
                placeholder: Some(placeholder.into()),
                on_change: Some(brouillon),
            }),
            SettingControl::ExtraButton(SettingExtraButtonControl {
                icon: "plus".into(),
                tooltip: Some("Ajouter".into()),
                on_click: ajouter,
            }),
        ],
    })
}

fn groupe(nom: &str, items: Vec<SettingDefinition>) -> SettingDefinition {
    SettingDefinition::Group(SettingDefinitionGroup {
        base: base(None),
        name: nom.into(),
        items,
    })
}

pub(crate) fn definitions(ctx: &ContexteAgent) -> Vec<SettingDefinition> {
    let p = ctx.reglages.politique();
    let mut defs = vec![groupe_permissions(ctx, &p), groupe_outils(ctx, &p)];
    defs.push(liste_commandes(ctx, &p));
    defs.push(liste_domaines(ctx, &p));
    defs.push(arbre_dossiers(ctx));
    defs.push(liste_dehors(ctx, &p));
    defs.push(groupe_toujours(ctx, &p));
    defs
}

fn groupe_permissions(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let mut items: Vec<SettingDefinition> = incoherences(p, &ctx.plateforme)
        .into_iter()
        .map(|m| {
            SettingDefinition::Empty(SettingDefinitionEmpty {
                base: base(None),
                message: format!("⚠ {m}"),
            })
        })
        .collect();

    let c = ctx.clone();
    items.push(SettingDefinition::Control(ligne_controle(
        Some("agent-mode"),
        "Mode par défaut",
        Some(
            "Accès complet : ce qui est « Demander » est permis sans carte (le comportement historique). Demander : une carte dans la réponse. Modifs auto : écrire sans demander. Plan : lecture seule, puis un plan à valider. Chaque conversation peut changer le sien.",
        ),
        liste_deroulante(
            en_texte(&p.mode),
            MODES
                .iter()
                .map(|m| (en_texte(m), mode_label(*m).to_owned()))
                .collect(),
            move |v| {
                if let Some(m) = depuis_texte::<PermissionMode>(&v) {
                    modifier(&c, |p| p.mode = m);
                }
            },
        ),
    )));

    for (preset, cle, description) in PREREGLAGES {
        let c = ctx.clone();
        items.push(SettingDefinition::Action(SettingDefinitionAction {
            base: base(Some(&format!("agent-preset-{cle}"))),
            name: format!("Préréglage « {} »", preset.label()),
            description: Some(description.into()),
            label: "Appliquer".into(),
            cta: false,
            warning: false,
            on_click: Rc::new(move || {
                let n = preset.make();
                modifier(&c, |p| {
                    p.mode = n.mode;
                    p.tools = n.tools;
                });
            }),
        }));
    }
    groupe("Permissions", items)
}

fn groupe_outils(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let acces = [
        (ToolAccess::Allow, "Autoriser"),
        (ToolAccess::Ask, "Demander"),
        (ToolAccess::Deny, "Interdire"),
    ];
    let items = TOOL_FAMILIES
        .iter()
        .map(|&f| {
            let c = ctx.clone();
            let valeur = p.tools.get(&f).copied().unwrap_or(ToolAccess::Deny);
            SettingDefinition::Control(ligne_controle(
                Some(&format!("agent-tool-{}", famille_en_texte(f))),
                family_label(f),
                None,
                liste_deroulante(
                    en_texte(&valeur),
                    acces
                        .iter()
                        .map(|(a, l)| (en_texte(a), (*l).to_owned()))
                        .collect(),
                    move |v| {
                        if let Some(a) = depuis_texte::<ToolAccess>(&v) {
                            modifier(&c, |p| {
                                p.tools.insert(f, a);
                            });
                        }
                    },
                ),
            ))
        })
        .collect();
    groupe("Outils", items)
}

fn liste_commandes(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let mut items: Vec<SettingGroupItem> = p
        .shell_rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let c = ctx.clone();
            ligne_simple(
                r,
                vec![retirer(move || {
                    modifier(&c, |p| {
                        if i < p.shell_rules.len() {
                            p.shell_rules.remove(i);
                        }
                    });
                })],
            )
        })
        .collect();
    let (b, c) = (ctx.clone(), ctx.clone());
    items.push(ligne_ajout(
        "agent-shell-add",
        "Ajouter",
        Some("Un motif, par exemple « git status* »."),
        "git status*",
        Rc::new(move |v| b.brouillons.borrow_mut().commande = v),
        Rc::new(move || {
            let v = std::mem::take(&mut c.brouillons.borrow_mut().commande);
            let v = v.trim().to_owned();
            if !v.is_empty() {
                modifier(&c, |p| p.shell_rules.push(v));
            }
        }),
    ));
    SettingDefinition::List(SettingDefinitionList {
        base: base(None),
        name: "Commandes permises (même si le terminal est « Demander »)".into(),
        items,
        add: None,
    })
}

fn liste_domaines(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let c = ctx.clone();
    let tous = matches!(p.web_domains, WebDomains::Tous);
    let mut items = vec![SettingGroupItem::Control(ligne_controle(
        Some("agent-web-all"),
        "Tous les domaines",
        None,
        SettingControl::Toggle(SettingToggleControl {
            value: tous,
            on_change: Some(Rc::new(move |v: bool| {
                modifier(&c, |p| {
                    p.web_domains = if v {
                        WebDomains::Tous
                    } else {
                        WebDomains::Liste(Vec::new())
                    };
                });
            })),
        }),
    ))];
    if let WebDomains::Liste(domaines) = &p.web_domains {
        for (i, d) in domaines.iter().enumerate() {
            let c = ctx.clone();
            items.push(ligne_simple(
                d,
                vec![retirer(move || {
                    modifier(&c, |p| {
                        if let WebDomains::Liste(l) = &mut p.web_domains
                            && i < l.len()
                        {
                            l.remove(i);
                        }
                    });
                })],
            ));
        }
        let (b, c) = (ctx.clone(), ctx.clone());
        items.push(ligne_ajout(
            "agent-web-add",
            "Ajouter un domaine",
            None,
            "wikipedia.org",
            Rc::new(move |v| b.brouillons.borrow_mut().domaine = v),
            Rc::new(move || {
                let v = std::mem::take(&mut c.brouillons.borrow_mut().domaine);
                let v = v.trim().to_owned();
                if !v.is_empty() {
                    modifier(&c, |p| {
                        if let WebDomains::Liste(l) = &mut p.web_domains {
                            l.push(v);
                        }
                    });
                }
            }),
        ));
    }
    SettingDefinition::List(SettingDefinitionList {
        base: base(None),
        name: "Domaines web".into(),
        items,
        add: None,
    })
}

/// L'arbre des dossiers du coffre : chaque nœud porte son accès (`inherit` : celui du parent).
fn noeuds_dossiers(ctx: &ContexteAgent, chemin: &str) -> Vec<SettingTreeNode> {
    let acces = [
        ("inherit", "Hérité"),
        ("hidden", "Masqué"),
        ("read", "Lecture"),
        ("write", "Lecture + écriture"),
    ];
    let mut dossiers = (ctx.dossiers)(chemin);
    // Les dossiers cachés (`.fragment`…) ne se règlent pas ; le tri est celui des noms.
    dossiers.retain(|d| !d.rsplit('/').next().unwrap_or(d).starts_with('.'));
    dossiers.sort();
    dossiers
        .into_iter()
        .map(|d| {
            let valeur = ctx
                .reglages
                .politique()
                .folders
                .get(&d)
                .map_or_else(|| "inherit".to_owned(), en_texte);
            let (c, enfants, chemin) = (ctx.clone(), ctx.clone(), d.clone());
            SettingTreeNode {
                id: d.clone(),
                label: d.rsplit('/').next().unwrap_or(&d).to_owned(),
                icon: Some("folder".into()),
                children: Some(Rc::new(move || noeuds_dossiers(&enfants, &chemin))),
                control: Some(liste_deroulante(
                    valeur,
                    acces
                        .iter()
                        .map(|(v, l)| ((*v).to_owned(), (*l).to_owned()))
                        .collect(),
                    move |v| {
                        modifier(&c, |p| {
                            if v == "inherit" {
                                p.folders.remove(&d);
                            } else if let Some(a) = depuis_texte::<FolderAccess>(&v) {
                                p.folders.insert(d.clone(), a);
                            }
                        });
                    },
                )),
            }
        })
        .collect()
}

fn arbre_dossiers(ctx: &ContexteAgent) -> SettingDefinition {
    let c = ctx.clone();
    SettingDefinition::Tree(SettingDefinitionTree {
        base: base(Some("agent-folders")),
        name: "Dossiers du coffre".into(),
        description: Some("Ce que l’agent voit de chaque dossier. Un dossier hérite de son parent ; la racine : lecture + écriture. Les chemins de Fragment (réglages de l’agent, secrets, journal) ne s’écrivent jamais.".into()),
        nodes: Rc::new(move || noeuds_dossiers(&c, "")),
    })
}

fn liste_dehors(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let mut items: Vec<SettingGroupItem> = p
        .outside
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let (c1, c2) = (ctx.clone(), ctx.clone());
            ligne_simple(
                &o.path,
                vec![
                    liste_deroulante(
                        en_texte(&o.access),
                        vec![
                            ("read".into(), "Lecture".into()),
                            ("write".into(), "Lecture + écriture".into()),
                        ],
                        move |v| {
                            if let Some(a) = depuis_texte::<FolderAccess>(&v) {
                                modifier(&c1, |p| {
                                    if let Some(x) = p.outside.get_mut(i) {
                                        x.access = a;
                                    }
                                });
                            }
                        },
                    ),
                    retirer(move || {
                        modifier(&c2, |p| {
                            if i < p.outside.len() {
                                p.outside.remove(i);
                            }
                        });
                    }),
                ],
            )
        })
        .collect();
    let (b, c) = (ctx.clone(), ctx.clone());
    items.push(ligne_ajout(
        "agent-outside-add",
        "Ajouter un dossier",
        Some("Un chemin absolu, lisible par l’agent."),
        "D:/Documents",
        Rc::new(move |v| b.brouillons.borrow_mut().dehors = v),
        Rc::new(move || {
            let v = std::mem::take(&mut c.brouillons.borrow_mut().dehors);
            let v = v.trim().to_owned();
            if !v.is_empty() {
                modifier(&c, |p| {
                    p.outside.push(DossierExterne {
                        path: v,
                        access: FolderAccess::Read,
                    });
                });
            }
        }),
    ));
    SettingDefinition::List(SettingDefinitionList {
        base: base(None),
        name: "Hors du coffre".into(),
        items,
        add: None,
    })
}

/// « Toujours autorisé dans ce coffre » : les règles retenues par les cartes.
fn groupe_toujours(ctx: &ContexteAgent, p: &AgentPolicy) -> SettingDefinition {
    let items = if p.permanent.is_empty() {
        vec![SettingDefinition::Empty(SettingDefinitionEmpty {
            base: base(None),
            message: "Aucune. Une carte « Toujours dans ce coffre » en ajoute une.".into(),
        })]
    } else {
        p.permanent
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let c = ctx.clone();
                SettingDefinition::Item(SettingDefinitionItem {
                    base: base(None),
                    name: nom_regle(r),
                    description: None,
                    controls: vec![retirer(move || {
                        modifier(&c, |p| {
                            if i < p.permanent.len() {
                                p.permanent.remove(i);
                            }
                        });
                    })],
                })
            })
            .collect()
    };
    groupe("Toujours autorisé dans ce coffre", items)
}

/// « Famille — motif » (« tout » sans motif).
pub fn nom_regle(r: &PermanentRule) -> String {
    format!(
        "{} — {}",
        family_label(r.family),
        r.pattern.as_deref().unwrap_or("tout")
    )
}
