// Port de app/src/ui/coreCommands/WorkspaceCommandsPlugin.ts (le MODÈLE)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le plugin interne « Espace de travail » — les commandes `workspace:*`.
//
//  Elles n'apportent AUCUNE mécanique : split, épinglage, fermeture existent
//  déjà dans le Workspace. Ce module ne garde que ce qui se teste sans fenêtre :
//    - le TABLEAU des commandes (id, nom, raccourcis) ;
//    - le cycle des onglets (`aller_a_onglet_voisin`) ;
//    - la validation des paramètres de `workspace:open` (CLI).
//
//  ★ Crate choisie : fragment-plugins (un plugin cœur est un plugin) ; la vue, le
//    workspace et `revealPassage` (l'attente de stabilité du défilement de
//    `ouvrirCli`, boucle de `setTimeout` sur le DOM) restent à fragment-ui.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

use fragment_cli::CliError;
use fragment_core::keymap::types::{Hotkey, Modifier};

/// Une commande `workspace:*` (le `addCommand` du TS, sans callback).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub hotkeys: Vec<Hotkey>,
}

fn cmd(id: &'static str, name: &'static str, hotkeys: Vec<Hotkey>) -> CommandeSpec {
    CommandeSpec { id, name, hotkeys }
}

/// Les commandes du plugin (hors marge de lecture, voir `margin_mode`), dans l'ordre d'inscription.
pub fn commandes_workspace() -> Vec<CommandeSpec> {
    use Modifier::{Ctrl, Mod, Shift};
    vec![
        cmd("split-vertical", "Diviser à droite", vec![]),
        cmd("split-horizontal", "Diviser en bas", vec![]),
        cmd("new-tab", "Nouvel onglet", vec![Hotkey::new(&[Mod], "t")]),
        cmd(
            "next-tab",
            "Onglet suivant",
            vec![Hotkey::new(&[Ctrl], "Tab")],
        ),
        cmd(
            "previous-tab",
            "Onglet précédent",
            vec![Hotkey::new(&[Ctrl, Shift], "Tab")],
        ),
        cmd("close", "Fermer l’onglet", vec![Hotkey::new(&[Mod], "w")]),
        cmd("close-others", "Fermer les autres onglets", vec![]),
        cmd("toggle-pin", "Épingler / désépingler", vec![]),
    ]
}

/// Une pile d'onglets, vue par le cycle.
pub trait PileDOnglets {
    fn nombre(&self) -> usize;
    /// L'onglet RÉELLEMENT visible (`pile.getActiveLeaf()`).
    fn visible(&self) -> Option<usize>;
    /// Bascule visuellement (`selectTab`) ; ne touche PAS la feuille active du workspace.
    fn select_tab(&mut self, index: usize);
}

/// L'index voisin de `courant` (`delta` = +1 suivant, -1 précédent), modulo toujours positif.
pub fn onglet_voisin(courant: usize, delta: isize, n: usize) -> usize {
    let n = n as isize;
    (((courant as isize + delta) % n + n) % n) as usize
}

/// Va à l'onglet voisin (`delta` = +1 suivant, -1 précédent). `activer(index)` est le
/// `workspace.setActiveLeaf(pile.children[index])` de l'hôte.
///
/// ★ On repart de l'onglet RÉELLEMENT VISIBLE (`pile.visible()`), pas de la feuille
///   active du workspace : cette dernière peut avoir dérivé, car ni le clic d'onglet
///   ni `select_tab` ne la mettent à jour. Puis on bascule la pile ET on réaligne la
///   feuille active — les deux sources de vérité (onglet visible / feuille active)
///   restent en phase, ce qui débloque le défilement.
pub fn aller_a_onglet_voisin(
    pile: &mut dyn PileDOnglets,
    delta: isize,
    activer: &mut dyn FnMut(usize),
) {
    let n = pile.nombre();
    if n == 0 {
        return;
    }
    let courant = pile.visible().unwrap_or(0);
    let cible = onglet_voisin(courant, delta, n);
    pile.select_tab(cible); // bascule visuellement
    activer(cible); // et logiquement
}

// ── CLI : workspace:open ─────────────────────────────────────────────────────

/// Où ouvrir (`where=`) ; absent / `here` = dans la feuille courante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OuvrirOu {
    Split,
    Tab,
}

impl OuvrirOu {
    fn as_str(self) -> &'static str {
        match self {
            OuvrirOu::Split => "split",
            OuvrirOu::Tab => "tab",
        }
    }
}

/// Une demande `workspace:open` validée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OuvrirDemande {
    pub doc: String,
    pub ou: Option<OuvrirOu>,
    /// À partir de 1 ; sur un PDF, la page.
    pub ligne: Option<u64>,
    pub passage: Option<String>,
}

/// `Number(s)` de JS, pour un entier : blancs rognés, vide = 0.
fn nombre_js(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// Valide les paramètres de `workspace:open`.
///
/// ★ `line=` est une LECTURE de la surface (`posToOffset`) : sur un PDF une ligne est
///   une page (la convention d'`annotation:strokes`) ; aucun format propre à une vue.
pub fn analyser_ouvrir(
    p: &HashMap<String, String>,
    existe: &dyn Fn(&str) -> bool,
) -> Result<OuvrirDemande, CliError> {
    let doc = p.get("doc").map(|d| d.trim()).unwrap_or("");
    if doc.is_empty() {
        return Err(CliError::new(
            1,
            "il manque doc=<chemin ou clé du document>",
        ));
    }
    if !existe(doc) {
        return Err(CliError::new(1, format!("document introuvable : {doc}")));
    }
    let where_ = p.get("where").map(String::as_str).filter(|w| !w.is_empty());
    let ou = match where_ {
        Some("split") => Some(OuvrirOu::Split),
        Some("tab") => Some(OuvrirOu::Tab),
        Some("here") | None => None,
        Some(_) => return Err(CliError::new(1, "where= vaut here, split ou tab")),
    };
    let ligne = match p.get("line") {
        None => None,
        Some(l) => {
            let n = nombre_js(l);
            if !(n.is_finite() && n.fract() == 0.0 && n >= 1.0) {
                return Err(CliError::new(1, "line= attend un entier ≥ 1"));
            }
            Some(n as u64)
        }
    };
    let passage = p
        .get("passage")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(OuvrirDemande {
        doc: doc.to_string(),
        ou,
        ligne,
        passage,
    })
}

/// Le message de succès de `workspace:open`.
pub fn message_ouvert(d: &OuvrirDemande) -> String {
    format!(
        "ouvert : {}{}{}",
        d.doc,
        d.ligne.map_or(String::new(), |l| format!(" (ligne {l})")),
        d.ou.map_or(String::new(), |o| format!(" [{}]", o.as_str())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══ next-tab — voir le bug AVANT de le corriger ═══
    //  On modélise le CONTRAT de défilement de WorkspaceTabs : `children` (l'ordre
    //  visuel), `active_index` (l'onglet visible), `select_tab(i)` — qui NE touche PAS
    //  `workspace.activeLeaf` (aucun focusin ne le resynchronise).

    struct FakePile {
        n: usize,
        active_index: usize,
    }

    impl PileDOnglets for FakePile {
        fn nombre(&self) -> usize {
            self.n
        }
        fn visible(&self) -> Option<usize> {
            (self.active_index < self.n).then_some(self.active_index)
        }
        fn select_tab(&mut self, i: usize) {
            if i < self.n {
                self.active_index = i;
            }
        }
    }

    /// La commande `next-tab` TELLE QU'EXPÉDIÉE : l'index de départ vient de
    /// `workspace.activeLeaf`, et AUCUN setActiveLeaf ne suit le selectTab.
    fn next_tab_ancien(pile: &mut FakePile, ws_active: usize) {
        let i = ws_active;
        pile.select_tab((i + 1) % pile.n);
        // ★ aucun workspace.setActiveLeaf(...) — activeLeaf reste figé : LE BUG.
    }

    #[test]
    fn rebondit_au_lieu_de_defiler_active_leaf_ne_suit_jamais_l_onglet_visible() {
        let mut pile = FakePile {
            n: 3,
            active_index: 0,
        };
        let ws_active = 0; // synchronisés au départ
        let mut visite = Vec::new();
        for _ in 0..3 {
            next_tab_ancien(&mut pile, ws_active);
            visite.push(pile.active_index);
        }
        // Le défilement ATTENDU serait [1, 2, 0]. Il est coincé sur B.
        assert_eq!(visite, vec![1, 1, 1]);
    }

    #[test]
    fn apres_un_clic_next_tab_part_du_mauvais_onglet() {
        let mut pile = FakePile {
            n: 3,
            active_index: 0,
        };
        let ws_active = 0;
        // L'utilisateur CLIQUE l'onglet C : selectTab le rend visible, mais le
        // handler de clic n'appelle pas setActiveLeaf → activeLeaf reste A.
        pile.select_tab(2);
        assert_eq!(pile.visible(), Some(2));
        assert_eq!(ws_active, 0); // la dérive
        next_tab_ancien(&mut pile, ws_active);
        assert_eq!(pile.active_index, 1); // B, au lieu de 0 (A) attendu
    }

    // ── Le fix : la VRAIE fonction ──

    #[test]
    fn defile_a_b_c_a_depuis_l_onglet_visible_et_realigne_active_leaf() {
        let mut pile = FakePile {
            n: 3,
            active_index: 0,
        };
        let mut ws_active: Option<usize> = None;
        let mut appels = 0;
        let mut visite = Vec::new();
        for _ in 0..4 {
            aller_a_onglet_voisin(&mut pile, 1, &mut |i| {
                ws_active = Some(i);
                appels += 1;
            });
            visite.push(pile.active_index);
        }
        assert_eq!(visite, vec![1, 2, 0, 1]); // vrai défilement cyclique
        assert_eq!(ws_active, Some(1)); // activeLeaf suit l'onglet visible
        assert_eq!(appels, 4);
    }

    #[test]
    fn previous_tab_defile_en_sens_inverse_a_c_b_a() {
        let mut pile = FakePile {
            n: 3,
            active_index: 0,
        };
        let mut visite = Vec::new();
        for _ in 0..3 {
            aller_a_onglet_voisin(&mut pile, -1, &mut |_| {});
            visite.push(pile.active_index);
        }
        assert_eq!(visite, vec![2, 1, 0]);
    }

    #[test]
    fn repart_de_l_onglet_visible_meme_si_active_leaf_a_derive_le_coeur_du_fix() {
        let mut pile = FakePile {
            n: 3,
            active_index: 0,
        };
        let mut ws_active = Some(0usize); // activeLeaf resté sur A…
        pile.select_tab(2); // …mais l'utilisateur a cliqué C (visible)
        aller_a_onglet_voisin(&mut pile, 1, &mut |i| ws_active = Some(i));
        assert_eq!(pile.active_index, 0); // depuis C VISIBLE → A, comme attendu
        assert_eq!(ws_active, Some(0));
    }

    // ── ajouts du port : tableau des commandes et workspace:open ──

    #[test]
    fn le_tableau_des_commandes_garde_les_ids_et_raccourcis_du_ts() {
        let c = commandes_workspace();
        let ids: Vec<&str> = c.iter().map(|x| x.id).collect();
        assert_eq!(
            ids,
            vec![
                "split-vertical",
                "split-horizontal",
                "new-tab",
                "next-tab",
                "previous-tab",
                "close",
                "close-others",
                "toggle-pin"
            ]
        );
        let prev = c
            .iter()
            .find(|x| x.id == "previous-tab")
            .expect("previous-tab");
        assert_eq!(
            prev.hotkeys,
            vec![Hotkey::new(&[Modifier::Ctrl, Modifier::Shift], "Tab")]
        );
    }

    fn params(paires: &[(&str, &str)]) -> HashMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn open_valide_ses_parametres_comme_le_ts() {
        let existe = |d: &str| d == "a.md";
        let e = |p: &[(&str, &str)]| {
            analyser_ouvrir(&params(p), &existe)
                .expect_err("erreur attendue")
                .message
        };
        assert_eq!(e(&[]), "il manque doc=<chemin ou clé du document>");
        assert_eq!(
            e(&[("doc", "  ")]),
            "il manque doc=<chemin ou clé du document>"
        );
        assert_eq!(e(&[("doc", "b.md")]), "document introuvable : b.md");
        assert_eq!(
            e(&[("doc", "a.md"), ("where", "ailleurs")]),
            "where= vaut here, split ou tab"
        );
        for l in ["0", "-1", "2.5", "abc"] {
            assert_eq!(
                e(&[("doc", "a.md"), ("line", l)]),
                "line= attend un entier ≥ 1",
                "line={l}"
            );
        }
        let d = analyser_ouvrir(
            &params(&[
                ("doc", " a.md "),
                ("where", "split"),
                ("line", " 3 "),
                ("passage", "  mot  "),
            ]),
            &existe,
        )
        .expect("valide");
        assert_eq!(
            d,
            OuvrirDemande {
                doc: "a.md".into(),
                ou: Some(OuvrirOu::Split),
                ligne: Some(3),
                passage: Some("mot".into())
            }
        );
        assert_eq!(message_ouvert(&d), "ouvert : a.md (ligne 3) [split]");
        let d = analyser_ouvrir(&params(&[("doc", "a.md"), ("where", "here")]), &existe)
            .expect("valide");
        assert_eq!(d.ou, None);
        assert_eq!(message_ouvert(&d), "ouvert : a.md");
    }
}
