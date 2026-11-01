// Port de app/src/ui/snap/snapModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE SNAP DES VUES — la logique PURE (testée), sur un arbre abstrait.
//
//  Ctrl+Alt+flèche sur la vue active : elle S'ÉTEND dans la direction de la
//  flèche, et tout ce qui est de ce côté dans son GROUPE se replie en tranches.
//  Le groupe = le premier split, en remontant depuis la vue, dans l'AXE de la
//  flèche (← → : un split « vertical », enfants côte à côte ; ↑ ↓ : un split
//  « horizontal », enfants empilés) qui a encore des frères non repliés de ce
//  côté. [1 | (2 / 3)] : 2 actif ↓ → 3 se replie ; 1 actif → → le groupe (2 / 3)
//  entier. [1 | 2 | 3] (un split à 3 enfants) : 2 actif → → seul 3.
//
//  La FLÈCHE OPPOSÉE défait (comme Win+→ puis Win+←) : si la vue active (ou un
//  de ses groupes) a replié l'autre côté, elle le rouvre — le plus intérieur
//  d'abord ; sinon elle snappe.
//
//  ★ Transposition : l'arbre est donné par le trait [`Arbre`] (accesseurs) ; le
//    modèle ne connaît pas les classes du workspace. Un nœud `N` est une POIGNÉE
//    copiable et comparable (un id, un index d'arène) : le `===` du TS devient `==`.
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapDir {
    Left,
    Right,
    Up,
    Down,
}

/// `Row` = enfants côte à côte (split « vertical » d'Obsidian), `Column` = empilés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapAxis {
    Row,
    Column,
}

/// De quel côté de la branche étendue sont les repliés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapSide {
    Before,
    After,
}

/// L'arbre du workspace vu par le modèle.
pub trait Arbre<N: Copy + PartialEq> {
    fn parent(&self, n: N) -> Option<N>;
    /// Les enfants d'un nœud ; `None` pour une feuille.
    fn children(&self, n: N) -> Option<Vec<N>>;
    /// L'axe d'un split ; `None` pour tout autre nœud (pile d'onglets, feuille).
    fn axis(&self, n: N) -> Option<SnapAxis>;
}

/// Un snap : dans `split`, `expanded` s'étend et `snapped` (frères, d'un côté) sont repliés.
#[derive(Clone, Debug, PartialEq)]
pub struct Snap<N> {
    pub split: N,
    pub expanded: N,
    pub snapped: Vec<N>,
    pub axis: SnapAxis,
    pub side: SnapSide,
}

pub fn axe_de(d: SnapDir) -> SnapAxis {
    match d {
        SnapDir::Left | SnapDir::Right => SnapAxis::Row,
        SnapDir::Up | SnapDir::Down => SnapAxis::Column,
    }
}

pub fn cote_de(d: SnapDir) -> SnapSide {
    match d {
        SnapDir::Right | SnapDir::Down => SnapSide::After,
        SnapDir::Left | SnapDir::Up => SnapSide::Before,
    }
}

pub fn autre_cote(s: SnapSide) -> SnapSide {
    match s {
        SnapSide::After => SnapSide::Before,
        SnapSide::Before => SnapSide::After,
    }
}

/// `a` est `n` ou l'un de ses ancêtres.
pub fn contient<N: Copy + PartialEq>(t: &dyn Arbre<N>, a: N, n: N) -> bool {
    let mut x = Some(n);
    while let Some(c) = x {
        if c == a {
            return true;
        }
        x = t.parent(c);
    }
    false
}

/// Les feuilles d'un nœud, dans l'ordre de l'arbre.
pub fn feuilles<N: Copy + PartialEq>(t: &dyn Arbre<N>, n: N) -> Vec<N> {
    match t.children(n) {
        None => vec![n],
        Some(c) => c.into_iter().flat_map(|x| feuilles(t, x)).collect(),
    }
}

/// Le snap que demande la flèche `dir` depuis `depuis` (la feuille active) — UNE
/// règle (correction utilisateur : « il n'y a pas d'histoire de bord ») : le GROUPE
/// de la vue = le premier split de l'axe, en remontant, où sa branche a des frères
/// non repliés ; la flèche dit quel CÔTÉ du groupe se replie.
///   - des frères de ce côté → ils se replient, la branche s'étend ([1 | 2], 1 → : 2) ;
///   - sinon c'est la branche qui est de ce côté → ELLE se replie, son voisin direct
///     de l'autre côté s'étend ([1 | 2], 2 → : 2 en tranche à droite, 1 prend tout).
///
/// `None` : la vue n'a de frère dans aucun split de l'axe.
pub fn trouver_snap<N: Copy + PartialEq>(
    t: &dyn Arbre<N>,
    depuis: N,
    dir: SnapDir,
    est_replie: &dyn Fn(N) -> bool,
) -> Option<Snap<N>> {
    let axis = axe_de(dir);
    let side = cote_de(dir);
    let mut item = depuis;
    let mut parent = t.parent(item);
    while let Some(p) = parent {
        let passe = t.axis(p) != Some(axis) || est_replie(item);
        if !passe {
            let enfants = t.children(p).unwrap_or_default();
            if let Some(i) = enfants.iter().position(|c| *c == item) {
                let avant: Vec<N> = enfants[..i]
                    .iter()
                    .copied()
                    .filter(|c| !est_replie(*c))
                    .collect();
                let apres: Vec<N> = enfants[i + 1..]
                    .iter()
                    .copied()
                    .filter(|c| !est_replie(*c))
                    .collect();
                // Pas de groupe à ce niveau.
                if !(avant.is_empty() && apres.is_empty()) {
                    let cote = if side == SnapSide::After {
                        &apres
                    } else {
                        &avant
                    };
                    if !cote.is_empty() {
                        return Some(Snap {
                            split: p,
                            expanded: item,
                            snapped: cote.clone(),
                            axis,
                            side,
                        });
                    }
                    let voisin = if side == SnapSide::After {
                        avant.last().copied()
                    } else {
                        apres.first().copied()
                    };
                    if let Some(voisin) = voisin {
                        return Some(Snap {
                            split: p,
                            expanded: voisin,
                            snapped: vec![item],
                            axis,
                            side,
                        });
                    }
                }
            }
        }
        item = p;
        parent = t.parent(item);
    }
    None
}

/// Le snap que la flèche `dir` DÉFAIT : un snap dont la branche étendue contient
/// `depuis`, dans l'axe de la flèche, qui a replié le côté OPPOSÉ — le plus
/// intérieur (la branche la plus proche de la vue).
pub fn trouver_restauration<'a, N: Copy + PartialEq>(
    t: &dyn Arbre<N>,
    depuis: N,
    dir: SnapDir,
    snaps: &'a [Snap<N>],
) -> Option<&'a Snap<N>> {
    let axis = axe_de(dir);
    let side = autre_cote(cote_de(dir));
    let mut x = Some(depuis);
    while let Some(c) = x {
        if let Some(s) = snaps
            .iter()
            .find(|s| s.expanded == c && s.axis == axis && s.side == side)
        {
            return Some(s);
        }
        x = t.parent(c);
    }
    None
}

// ── Les proportions (flex-grow) ──────────────────────────────────────────────
//  Les repliés GARDENT leur part (leur taille d'avant) : seule leur PRÉSENTATION
//  change (CSS, une tranche de taille fixe). La branche étendue reçoit une part
//  de plus, CHOISIE pour que les AUTRES frères ne bougent pas d'un pixel ([1 | 2 |
//  3], 2 → : 1 garde sa largeur, comme dans [1 | (2 | 3)]) — sans elle, la place
//  des tranches serait prise à tous. Avant : la place `avant` (px) est partagée
//  par Σ toutes les parts ; après : `apres` (= avant − tranches) par les autres +
//  l'étendu. Un autre garde `g·avant/G` ⇔ autres + étendu' = G·apres/avant.
//
//  ★ Le surplus est retenu RELATIVEMENT aux parts repliées (`tau` = surplus ÷ Σ
//    repliées) : la renormalisation du cœur multiplie toutes les parts d'un split
//    par un même facteur, une poignée tirée ailleurs ne touche pas les repliées,
//    la fenêtre redimensionnée ne change aucune part — `tau` reste juste, et le
//    dé-snap rend `tau × Σ repliées`.

const MIN_GROW: f64 = 0.05;

fn somme(xs: &[f64]) -> f64 {
    xs.iter().sum()
}

/// Les parts d'un split au moment du snap.
#[derive(Clone, Debug)]
pub struct PartsDuSplit {
    /// La part de la branche étendue.
    pub etendu: f64,
    /// Les parts des repliés.
    pub replies: Vec<f64>,
    /// Les parts des autres frères (qui ne doivent pas bouger).
    pub autres: Vec<f64>,
    /// La place partagée avant le snap (px, hors poignées) ; `apres` = avant − tranches.
    pub avant: f64,
    pub apres: f64,
}

/// La part de l'étendu après le snap, et `tau` (surplus ÷ Σ repliées).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrowApresSnap {
    pub grow: f64,
    pub tau: f64,
}

pub fn grow_apres_snap(p: &PartsDuSplit) -> GrowApresSnap {
    let sr = somme(&p.replies);
    let g = p.etendu + sr + somme(&p.autres);
    let voulu = if p.avant > 0.0 && p.apres > 0.0 {
        g * p.apres / p.avant - somme(&p.autres)
    } else {
        f64::NAN
    };
    // Hors mise en page (mesures nulles) : l'étendu prend simplement les parts repliées.
    let grow = if voulu.is_finite() && voulu > MIN_GROW {
        voulu
    } else {
        p.etendu + sr
    };
    GrowApresSnap {
        grow,
        tau: if sr > 0.0 {
            (grow - p.etendu) / sr
        } else {
            0.0
        },
    }
}

/// La part de l'étendu au dé-snap : il rend `tau × Σ repliées encore là`.
pub fn grow_apres_restauration(etendu: f64, tau: f64, replies: &[f64]) -> f64 {
    f64::max(MIN_GROW, etendu - tau * somme(replies))
}

// ── La persistance (par ids de feuilles) ─────────────────────────────────────

/// Un snap tel qu'enregistré dans `workspace.json` (clés JSON en camelCase côté TS).
#[derive(Clone, Debug, PartialEq)]
pub struct SnapEnregistre {
    pub axis: SnapAxis,
    pub side: SnapSide,
    /// Des feuilles de la branche étendue (une suffit pour la retrouver).
    pub expanded: Vec<String>,
    /// Les feuilles de chaque nœud replié.
    pub snapped: Vec<Vec<String>>,
    /// Le surplus de l'étendu ÷ Σ parts repliées (voir [`grow_apres_snap`]).
    pub tau: f64,
    /// Part de l'étendu ÷ Σ parts repliées, au moment d'enregistrer (surplus
    /// compris) : au redémarrage, dit si workspace.json a été écrit AVEC le surplus
    /// (cas normal) ou avant lui (fermeture brutale).
    pub ratio: f64,
}

/// L'ancêtre commun le plus bas.
pub fn ancetre_commun<N: Copy + PartialEq>(t: &dyn Arbre<N>, ns: &[N]) -> Option<N> {
    if ns.is_empty() {
        return None;
    }
    let chemin = |n: N| -> Vec<N> {
        let mut c = Vec::new();
        let mut x = Some(n);
        while let Some(v) = x {
            c.insert(0, v);
            x = t.parent(v);
        }
        c
    };
    let chemins: Vec<Vec<N>> = ns.iter().map(|n| chemin(*n)).collect();
    let mut commun = None;
    let mut i = 0;
    loop {
        let Some(x) = chemins[0].get(i).copied() else {
            return commun;
        };
        if chemins.iter().any(|c| c.get(i) != Some(&x)) {
            return commun;
        }
        commun = Some(x);
        i += 1;
    }
}

/// Retrouve un snap enregistré dans l'arbre restauré : chaque groupe de feuilles
/// repliées donne son nœud (ancêtre commun, remonté jusqu'à l'enfant d'un split
/// de l'axe), leur parent commun est le split (de l'axe), la branche étendue est
/// l'enfant du split qui contient une feuille étendue, et les repliés sont bien du
/// côté enregistré. Sinon `None` (abandonné).
pub fn reidentifier<N: Copy + PartialEq>(
    t: &dyn Arbre<N>,
    rec: &SnapEnregistre,
    feuille: &dyn Fn(&str) -> Option<N>,
) -> Option<Snap<N>> {
    let mut snapped: Vec<N> = Vec::new();
    for ids in &rec.snapped {
        let fs: Vec<N> = ids.iter().filter_map(|id| feuille(id)).collect();
        // Le nœud replié = l'ancêtre commun, remonté jusqu'à l'enfant d'un split de
        // l'axe (une seule feuille : sa PILE, pas la feuille).
        let mut n = ancetre_commun(t, &fs);
        while let Some(cur) = n {
            let Some(p) = t.parent(cur) else { break };
            if t.axis(p) == Some(rec.axis) {
                break;
            }
            n = Some(p);
        }
        let n = n?;
        t.parent(n)?;
        snapped.push(n);
    }
    if snapped.is_empty() {
        return None;
    }
    let split = t.parent(snapped[0])?;
    if t.axis(split) != Some(rec.axis) || snapped.iter().any(|s| t.parent(*s) != Some(split)) {
        return None;
    }
    let enfants = t.children(split).unwrap_or_default();
    let mut expanded: Option<N> = None;
    for id in &rec.expanded {
        let Some(f) = feuille(id) else { continue };
        expanded = enfants.iter().copied().find(|c| contient(t, *c, f));
        if expanded.is_some() {
            break;
        }
    }
    let expanded = expanded?;
    if snapped.contains(&expanded) {
        return None;
    }
    let index = |n: N| enfants.iter().position(|c| *c == n);
    let ie = index(expanded)?;
    let bon_cote = snapped.iter().all(|s| match (index(*s), rec.side) {
        (Some(i), SnapSide::After) => i > ie,
        (Some(i), SnapSide::Before) => i < ie,
        (None, _) => false,
    });
    bon_cote.then_some(Snap {
        split,
        expanded,
        snapped,
        axis: rec.axis,
        side: rec.side,
    })
}

/// workspace.json porte-t-il le surplus ? `ratio` = étendu ÷ Σ repliées à
/// l'enregistrement (surplus compris) ; SANS lui, le ratio vaut `ratio − tau`.
/// On prend le plus proche du ratio relu.
pub fn transfert_present(ratio_relu: f64, ratio_enregistre: f64, tau: f64) -> bool {
    (ratio_relu - ratio_enregistre).abs() <= (ratio_relu - (ratio_enregistre - tau)).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Un arbre de jeu : split(axe, …) / pile(…feuilles) / feuille(id). Arène d'indices.
    struct Noeud {
        id: String,
        axis: Option<SnapAxis>,
        children: Option<Vec<usize>>,
        parent: Option<usize>,
    }

    #[derive(Default)]
    struct Arene {
        noeuds: Vec<Noeud>,
    }

    impl Arene {
        fn ajouter(
            &mut self,
            id: &str,
            axis: Option<SnapAxis>,
            children: Option<&[usize]>,
        ) -> usize {
            let i = self.noeuds.len();
            self.noeuds.push(Noeud {
                id: id.into(),
                axis,
                children: children.map(<[usize]>::to_vec),
                parent: None,
            });
            for c in children.unwrap_or(&[]) {
                self.noeuds[*c].parent = Some(i);
            }
            i
        }
        fn split(&mut self, axis: SnapAxis, c: &[usize]) -> usize {
            self.ajouter("", Some(axis), Some(c))
        }
        fn pile(&mut self, c: &[usize]) -> usize {
            self.ajouter("", None, Some(c))
        }
        fn f(&mut self, id: &str) -> usize {
            self.ajouter(id, None, None)
        }
        fn ids(&self, ns: &[usize]) -> Vec<String> {
            ns.iter()
                .flat_map(|n| feuilles(self, *n))
                .map(|x| self.noeuds[x].id.clone())
                .collect()
        }
        fn par_id(&self, id: &str) -> Option<usize> {
            self.noeuds
                .iter()
                .position(|n| n.children.is_none() && n.id == id)
        }
    }

    impl Arbre<usize> for Arene {
        fn parent(&self, n: usize) -> Option<usize> {
            self.noeuds[n].parent
        }
        fn children(&self, n: usize) -> Option<Vec<usize>> {
            self.noeuds[n].children.clone()
        }
        fn axis(&self, n: usize) -> Option<SnapAxis> {
            self.noeuds[n].axis
        }
    }

    fn jamais(_: usize) -> bool {
        false
    }

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    use SnapAxis::{Column, Row};

    // ── trouverSnap — le schéma ──
    #[test]
    fn un_deux_1_actif_droite_2_se_replie_1_s_etend() {
        let mut t = Arene::default();
        let (a, b) = (t.f("1"), t.f("2"));
        let (pa, pb) = (t.pile(&[a]), t.pile(&[b]));
        let root = t.split(Row, &[pa, pb]);
        let s = trouver_snap(&t, a, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(s.split, root);
        assert_eq!(t.ids(&[s.expanded]), v(&["1"]));
        assert_eq!(t.ids(&s.snapped), v(&["2"]));
        assert_eq!(s.side, SnapSide::After);
    }

    #[test]
    fn un_puis_deux_sur_trois_2_actif_bas_3_se_replie_dans_le_groupe() {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        t.split(Row, &[pa, g]);
        let s = trouver_snap(&t, b, SnapDir::Down, &jamais).expect("snap");
        assert_eq!(s.split, g);
        assert_eq!(t.ids(&s.snapped), v(&["3"]));
    }

    #[test]
    fn meme_layout_1_actif_droite_le_groupe_entier_se_replie() {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        t.split(Row, &[pa, g]);
        let s = trouver_snap(&t, a, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(s.snapped, vec![g]);
    }

    #[test]
    fn trois_enfants_2_actif_droite_seul_3_1_actif_droite_2_et_3() {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        t.split(Row, &[pa, pb, pc]);
        let ids = |t: &Arene, n, d| t.ids(&trouver_snap(t, n, d, &jamais).expect("snap").snapped);
        assert_eq!(ids(&t, b, SnapDir::Right), v(&["3"]));
        assert_eq!(ids(&t, a, SnapDir::Right), v(&["2", "3"]));
        assert_eq!(ids(&t, c, SnapDir::Left), v(&["1", "2"]));
    }

    #[test]
    fn le_groupe_le_premier_split_de_l_axe_ou_la_vue_a_des_freres_la_fleche_dit_le_cote_qui_se_replie()
     {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        t.split(Row, &[pa, g]);
        // 2 actif, ← : son groupe (colonne) n'est pas de l'axe, le root a 1 à gauche.
        let s = trouver_snap(&t, b, SnapDir::Left, &jamais).expect("snap");
        assert_eq!(t.ids(&s.snapped), v(&["1"]));
        // → : (2 / 3) est du côté de la flèche → il se replie.
        let s = trouver_snap(&t, b, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(s.snapped, vec![g]);
        // ↑ : dans (2 / 3), 2 est en haut → 2 se replie, 3 s'étend.
        let h = trouver_snap(&t, b, SnapDir::Up, &jamais).expect("snap");
        assert_eq!(t.ids(&h.snapped), v(&["2"]));
        assert_eq!(t.ids(&[h.expanded]), v(&["3"]));
    }

    #[test]
    fn les_freres_deja_replies_ne_comptent_pas_tous_replies_on_remonte() {
        let mut t = Arene::default();
        let (a, b, c, d) = (t.f("1"), t.f("2"), t.f("3"), t.f("4"));
        let pa = t.pile(&[a]);
        let pb = t.pile(&[b]);
        let pc = t.pile(&[c]);
        let pd = t.pile(&[d]);
        let g = t.split(Row, &[pb, pc]);
        let ra = t.split(Row, &[pa]);
        t.split(Column, &[ra, g, pd]);
        // 2 → : 3 est déjà replié, rien d'autre à droite dans g ni plus haut (axe colonne).
        assert_eq!(trouver_snap(&t, b, SnapDir::Right, &|n| n == pc), None);
    }

    // ── la vue du côté de la flèche : ELLE se replie ──
    #[test]
    fn un_deux_2_actif_droite_rien_a_droite_2_se_replie_1_s_etend_gauche_depuis_1_le_defait() {
        let mut t = Arene::default();
        let (a, b) = (t.f("1"), t.f("2"));
        let (pa, pb) = (t.pile(&[a]), t.pile(&[b]));
        let root = t.split(Row, &[pa, pb]);
        let s = trouver_snap(&t, b, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(
            s,
            Snap {
                split: root,
                expanded: pa,
                snapped: vec![pb],
                axis: Row,
                side: SnapSide::After
            }
        );
        let snaps = [s];
        let r = trouver_restauration(&t, a, SnapDir::Left, &snaps);
        assert!(r.is_some_and(|r| std::ptr::eq(r, &snaps[0])));
    }

    #[test]
    fn trois_enfants_3_actif_droite_le_voisin_direct_2_s_etend() {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        t.split(Row, &[pa, pb, pc]);
        let s = trouver_snap(&t, c, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(t.ids(&[s.expanded]), v(&["2"]));
        assert_eq!(t.ids(&s.snapped), v(&["3"]));
    }

    #[test]
    fn un_puis_deux_sur_trois_2_actif_droite_le_groupe_se_replie_a_droite_1_s_etend() {
        let mut t = Arene::default();
        let (a, b, c) = (t.f("1"), t.f("2"), t.f("3"));
        let (pa, pb, pc) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        t.split(Row, &[pa, g]);
        let s = trouver_snap(&t, b, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(s.snapped, vec![g]);
        assert_eq!(t.ids(&[s.expanded]), v(&["1"]));
    }

    #[test]
    fn une_seule_pile_ou_un_axe_absent_null() {
        let mut t = Arene::default();
        let a = t.f("1");
        let pa = t.pile(&[a]);
        t.split(Row, &[pa]);
        assert_eq!(trouver_snap(&t, a, SnapDir::Right, &jamais), None);
        let (b, c) = (t.f("2"), t.f("3"));
        let (pb, pc) = (t.pile(&[b]), t.pile(&[c]));
        t.split(Row, &[pb, pc]);
        assert_eq!(trouver_snap(&t, c, SnapDir::Down, &jamais), None);
    }

    // ── trouverRestauration ──
    #[test]
    fn un_a_replie_2_droite_gauche_le_rouvre_droite_ne_le_rouvre_pas() {
        let mut t = Arene::default();
        let (a, b) = (t.f("1"), t.f("2"));
        let (pa, pb) = (t.pile(&[a]), t.pile(&[b]));
        t.split(Row, &[pa, pb]);
        let snaps = [trouver_snap(&t, a, SnapDir::Right, &jamais).expect("snap")];
        let r = trouver_restauration(&t, a, SnapDir::Left, &snaps);
        assert!(r.is_some_and(|r| std::ptr::eq(r, &snaps[0])));
        assert_eq!(trouver_restauration(&t, a, SnapDir::Right, &snaps), None);
        assert_eq!(trouver_restauration(&t, a, SnapDir::Up, &snaps), None);
    }

    #[test]
    fn le_plus_interieur_d_abord() {
        let mut t = Arene::default();
        let (a, b, c, d) = (t.f("1"), t.f("2"), t.f("3"), t.f("4"));
        let (pa, pb, pc, pd) = (t.pile(&[a]), t.pile(&[b]), t.pile(&[c]), t.pile(&[d]));
        let g = t.split(Row, &[pa, pb]);
        // (même axe imbriqué : possible après restauration)
        let root = t.split(Row, &[g, pc, pd]);
        let interieur = trouver_snap(&t, a, SnapDir::Right, &jamais).expect("snap");
        assert_eq!(interieur.split, g);
        let dehors = Snap {
            split: root,
            expanded: g,
            snapped: t.children(root).expect("enfants")[1..].to_vec(),
            axis: Row,
            side: SnapSide::After,
        };
        let snaps = [dehors, interieur];
        let r = trouver_restauration(&t, a, SnapDir::Left, &snaps);
        assert!(r.is_some_and(|r| std::ptr::eq(r, &snaps[1])));
    }

    // ── les proportions ──
    // Largeur d'un frère = sa part ÷ Σ parts × place partagée.
    fn largeur(g: f64, toutes: &[f64], place: f64) -> f64 {
        g / toutes.iter().sum::<f64>() * place
    }

    fn proche(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn un_deux_trois_2_droite_1_garde_sa_largeur_au_pixel_2_prend_le_reste() {
        let (avant, tranche) = (900.0, 36.0);
        let GrowApresSnap { grow, .. } = grow_apres_snap(&PartsDuSplit {
            etendu: 1.0,
            replies: vec![1.0],
            autres: vec![1.0],
            avant,
            apres: avant - tranche,
        });
        let l1_avant = largeur(1.0, &[1.0, 1.0, 1.0], avant);
        let l1_apres = largeur(1.0, &[1.0, grow], avant - tranche);
        assert!(proche(l1_apres, l1_avant));
        assert!(proche(
            largeur(grow, &[1.0, grow], avant - tranche),
            avant - tranche - l1_avant
        ));
    }

    #[test]
    fn le_de_snap_rend_exactement_la_part_d_avant_meme_apres_une_renormalisation() {
        let p = PartsDuSplit {
            etendu: 0.8,
            replies: vec![1.2, 0.5],
            autres: vec![1.5],
            avant: 1000.0,
            apres: 928.0,
        };
        let GrowApresSnap { grow, tau } = grow_apres_snap(&p);
        assert!(proche(grow_apres_restauration(grow, tau, &p.replies), 0.8));
        let k = 1.37;
        let replies: Vec<f64> = p.replies.iter().map(|r| r * k).collect();
        assert!(proche(
            grow_apres_restauration(grow * k, tau, &replies),
            0.8 * k
        ));
    }

    #[test]
    fn hors_mise_en_page_mesures_nulles_l_etendu_prend_simplement_les_parts_replies() {
        let r = grow_apres_snap(&PartsDuSplit {
            etendu: 1.0,
            replies: vec![0.6, 0.4],
            autres: vec![],
            avant: 0.0,
            apres: 0.0,
        });
        assert!(proche(r.grow, 2.0));
        assert!(proche(r.tau, 1.0));
    }

    #[test]
    fn le_rendu_ne_descend_jamais_a_0() {
        assert!(grow_apres_restauration(0.5, 1.0, &[1.0]) > 0.0);
    }

    #[test]
    fn transfert_present_le_ratio_relu_le_plus_proche_avec_ou_sans_le_surplus() {
        assert!(transfert_present(3.0, 3.0, 1.5));
        assert!(!transfert_present(1.5, 3.0, 1.5));
        assert!(transfert_present(2.8, 3.0, 1.5));
    }

    // ── la persistance ──
    #[test]
    fn ancetre_commun_cas() {
        let mut t = Arene::default();
        let (b, c) = (t.f("2"), t.f("3"));
        let (pb, pc) = (t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        assert_eq!(ancetre_commun(&t, &[b, c]), Some(g));
        assert_eq!(ancetre_commun(&t, &[b]), Some(b));
    }

    fn rec(
        axis: SnapAxis,
        side: SnapSide,
        expanded: &[&str],
        snapped: &[&[&str]],
    ) -> SnapEnregistre {
        SnapEnregistre {
            axis,
            side,
            expanded: v(expanded),
            snapped: snapped.iter().map(|s| v(s)).collect(),
            tau: 1.0,
            ratio: 2.0,
        }
    }

    #[test]
    fn reidentifier_retrouve_split_etendu_et_replies_abandonne_s_ils_ne_correspondent_plus() {
        let mut t = Arene::default();
        let (a, b, c, x) = (t.f("1"), t.f("2"), t.f("3"), t.f("x"));
        let (pb, pc) = (t.pile(&[b]), t.pile(&[c]));
        let g = t.split(Column, &[pb, pc]);
        let pax = t.pile(&[a, x]);
        let root = t.split(Row, &[pax, g]);
        let par_id = |id: &str| t.par_id(id);
        let s = reidentifier(
            &t,
            &rec(Row, SnapSide::After, &["gone", "1"], &[&["2", "3"]]),
            &par_id,
        )
        .expect("snap");
        assert_eq!(s.split, root);
        assert_eq!(s.snapped, vec![g]);
        assert_eq!(t.ids(&[s.expanded]), v(&["1", "x"]));
        // Mauvais côté, mauvais axe, feuilles disparues : abandonné.
        let abandonne = |r: SnapEnregistre| reidentifier(&t, &r, &par_id).is_none();
        assert!(abandonne(rec(
            Row,
            SnapSide::Before,
            &["1"],
            &[&["2", "3"]]
        )));
        assert!(abandonne(rec(
            Column,
            SnapSide::After,
            &["1"],
            &[&["2", "3"]]
        )));
        assert!(abandonne(rec(Row, SnapSide::After, &["1"], &[&["nope"]])));
        assert!(abandonne(rec(Row, SnapSide::After, &["nope"], &[&["2"]])));
    }

    #[test]
    fn reidentifier_une_pile_d_une_feuille_repliee_la_pile_pas_la_feuille() {
        let mut t = Arene::default();
        let (a, b) = (t.f("1"), t.f("2"));
        let pa = t.pile(&[a]);
        let pb = t.pile(&[b]);
        let root = t.split(Row, &[pa, pb]);
        let par_id = |id: &str| t.par_id(id);
        let s =
            reidentifier(&t, &rec(Row, SnapSide::After, &["1"], &[&["2"]]), &par_id).expect("snap");
        assert_eq!(s.split, root);
        assert_eq!(s.snapped, vec![pb]);
    }
}
