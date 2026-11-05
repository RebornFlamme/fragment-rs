// Port de app/src/ui/core/layers/gutterStack.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le placement des widgets de MARGE — pur. Appelé par l'arbitre de la vue
//  (`GutterArbiter`), pour TOUS les widgets d'une marge, quel que soit le calque
//  qui les a posés.
//
//  Chaque widget veut se placer au niveau de son texte (`top` voulu). Deux ancres
//  proches se recouvriraient : on cherche les hauts qui
//    1. ne se recouvrent jamais (écart minimal `gap`) ;
//    2. gardent l'ORDRE du texte ;
//    3. s'écartent le MOINS possible des ancres (somme des carrés des écarts).
//
//  ★ C'est une régression isotone : l'algorithme PAV (pool adjacent violators) la
//    résout exactement. Une GRAPPE (au sens PAV) = des widgets collés les uns aux
//    autres ; son haut optimal est la moyenne des (voulu − décalage dans la
//    grappe). On ajoute les widgets dans l'ordre ; tant que la nouvelle grappe
//    chevauche la précédente, on les fusionne. Un widget peut donc MONTER comme
//    descendre : deux notes en conflit se partagent le décalage.
//
//  ★ ÉPINGLÉ (le widget ACTIF) : il reste PILE sur son ancre, comme un commentaire
//    Google Docs. Ceux d'après sont placés sous lui (borne basse), ceux d'avant
//    au-dessus (PAV, puis la queue remontée sous lui). S'ils ne tiennent pas entre
//    le haut du document et lui, il descend d'autant : rien ne sort par le haut.
//
//  ★ Nombres : des `f64`, comme les `number` du TS ; `±∞` sont des valeurs
//    légitimes (`min_top` absent = −∞, place illimitée = +∞).
// ═══════════════════════════════════════════════════════════════════════════

use std::cmp::Ordering;

/// L'écart minimal entre deux widgets de marge, en px du plan.
pub const GUTTER_STACK_GAP: f64 = 8.0;

/// Le souffle entre la colonne de texte et les widgets de marge : PROPORTIONNEL à
/// la marge (demande utilisateur), borné — une marge étroite garde l'ancien écart
/// de 12 px, une très large ne repousse pas les cartes au bord.
pub const GUTTER_GAP_MIN: f64 = 12.0;
pub const GUTTER_GAP_MAX: f64 = 64.0;
pub const GUTTER_GAP_RATIO: f64 = 0.1;

/// L'écart colonne → widget pour une marge de `marge` px (écran).
pub fn gutter_gap(marge: f64) -> f64 {
    if !marge.is_finite() || marge <= 0.0 {
        return GUTTER_GAP_MIN;
    }
    GUTTER_GAP_MAX.min(GUTTER_GAP_MIN.max(marge * GUTTER_GAP_RATIO))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GutterItem {
    /// Le haut voulu (le niveau du texte ancré).
    pub top: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrangeOptions {
    /// Index (dans l'entrée) du widget qui reste sur son ancre ; `None` = aucun.
    pub pinned: Option<usize>,
    pub gap: f64,
    /// Aucun widget ne monte au-dessus (le haut du document).
    pub min_top: f64,
}

impl Default for ArrangeOptions {
    fn default() -> Self {
        Self {
            pinned: None,
            gap: GUTTER_STACK_GAP,
            min_top: f64::NEG_INFINITY,
        }
    }
}

/// Les index de l'entrée dans l'ORDRE DU TEXTE : à haut égal, l'ordre d'entrée tranche.
fn ordre_du_texte(tops: &[f64]) -> Vec<usize> {
    let mut ordre: Vec<usize> = (0..tops.len()).collect();
    // `sort_by` est stable ; le `then` rend la règle « à égalité, l'entrée » explicite.
    ordre.sort_by(|&a, &b| {
        tops[a]
            .partial_cmp(&tops[b])
            .unwrap_or(Ordering::Equal)
            .then(a.cmp(&b))
    });
    ordre
}

/// Une grappe PAV : `n` items consécutifs collés, de `somme` de voulus ramenés au haut.
struct Grappe {
    debut: usize,
    n: usize,
    somme: f64,
    hauteur: f64,
}

/// PAV avec une borne basse : les items sont DÉJÀ dans l'ordre du texte. Rend les
/// hauts dans ce même ordre.
fn pav(items: &[GutterItem], gap: f64, lo: f64) -> Vec<f64> {
    let haut = |g: &Grappe| lo.max(g.somme / g.n as f64);
    let mut grappes: Vec<Grappe> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let mut g = Grappe {
            debut: i,
            n: 1,
            somme: it.top,
            hauteur: it.height,
        };
        while let Some(prec) = grappes.last() {
            if haut(&g) >= haut(prec) + prec.hauteur + gap {
                break;
            }
            // Fusion : dans la grappe fusionnée, les items de `g` sont décalés de
            // (hauteur de prec + gap) — leur « voulu ramené au haut de grappe »
            // baisse d'autant.
            let decalage = prec.hauteur + gap;
            g = Grappe {
                debut: prec.debut,
                n: prec.n + g.n,
                somme: prec.somme + g.somme - g.n as f64 * decalage,
                hauteur: decalage + g.hauteur,
            };
            grappes.pop();
        }
        grappes.push(g);
    }
    let mut tops = Vec::with_capacity(items.len());
    for g in &grappes {
        let mut y = haut(g);
        for it in &items[g.debut..g.debut + g.n] {
            tops.push(y);
            y += it.height + gap;
        }
    }
    tops
}

/// Les hauts effectifs, dans l'ORDRE DE L'ENTRÉE. À haut voulu égal, l'ordre
/// d'entrée tranche.
pub fn arrange_gutter(items: &[GutterItem], opts: ArrangeOptions) -> Vec<f64> {
    let ArrangeOptions {
        pinned,
        gap,
        min_top,
    } = opts;
    let voulus: Vec<f64> = items.iter().map(|it| it.top).collect();
    let ordre = ordre_du_texte(&voulus);
    let mut tops = vec![0.0; items.len()];
    // Un index épinglé hors de l'entrée n'épingle rien (le TS : `indexOf` → −1).
    let epingle = pinned.and_then(|p| ordre.iter().position(|&i| i == p).map(|k| (p, k)));

    let Some((p, k)) = epingle else {
        let tries: Vec<GutterItem> = ordre.iter().map(|&i| items[i]).collect();
        for (j, t) in pav(&tries, gap, min_top).into_iter().enumerate() {
            tops[ordre[j]] = t;
        }
        return tops;
    };

    // Ceux d'AVANT l'épinglé doivent tenir entre le haut du document et lui :
    // s'ils n'y tiennent pas, l'épinglé descend juste ce qu'il faut (seule
    // exception à « pile sur son ancre » — un widget ne sort jamais par le haut).
    let avant = &ordre[..k];
    let pile: f64 = avant.iter().map(|&i| items[i].height + gap).sum();
    let epingle = items[p];
    let haut = epingle.top.max(min_top + pile);
    tops[p] = haut;
    // Après : sous lui (borne basse).
    let apres = &ordre[k + 1..];
    let items_apres: Vec<GutterItem> = apres.iter().map(|&i| items[i]).collect();
    for (j, t) in pav(&items_apres, gap, haut + epingle.height + gap)
        .into_iter()
        .enumerate()
    {
        tops[apres[j]] = t;
    }
    // Avant : au plus près (borne basse = haut du document), puis on remonte la
    // queue sous la borne haute (haut de l'épinglé − gap), de bas en haut.
    let items_avant: Vec<GutterItem> = avant.iter().map(|&i| items[i]).collect();
    let mut t_avant = pav(&items_avant, gap, min_top);
    let mut limite = haut - gap;
    for j in (0..avant.len()).rev() {
        let i = avant[j];
        t_avant[j] = t_avant[j].min(limite - items[i].height);
        tops[i] = t_avant[j];
        limite = t_avant[j] - gap;
    }
    tops
}

// ═══════════════════════════════════════════════════════════════════════════
//  HAUTEUR OFFERTE à chaque widget de marge par le cœur.
//
//  ★ C'est une donnée, pas un ordre. Les densités (compacte, pastille…) sont des
//    seuils de plugin, inconnus du cœur. Chaque widget apprend la hauteur dont il
//    dispose avant le suivant, puis se condense selon ses seuils ou l'ignore et
//    accepte d'être décalé.
//
//  Calcul dans l'ordre du texte ; chaque valeur ne dépend que des widgets d'avant, donc
//  ni cycle ni oscillation :
//      haut(i)  = max(ancre(i), bas(i-1) + gap)      (widget épinglé : son ancre)
//      place(i) = max(0, ancre(i+1) − haut(i) − gap)
//      bas(i)   = haut(i) + measure(i, place(i))     (mesure après réaction du widget)
//  Le dernier widget a une place illimitée, laissée au défilement. L'épinglé (actif) est
//  limité à `cap`, la hauteur visible, pour qu'un long fil ne chasse pas les autres.
//  Le placement final reste `arrange_gutter`, avec les hauteurs mesurées.
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomOptions {
    /// Index (dans l'entrée) du widget actif ; `None` = aucun.
    pub pinned: Option<usize>,
    pub gap: f64,
    /// La place de l'actif (défaut : illimitée).
    pub cap: f64,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            pinned: None,
            gap: GUTTER_STACK_GAP,
            cap: f64::INFINITY,
        }
    }
}

/// Les places et les hauteurs mesurées, chacune dans l'ORDRE DE L'ENTRÉE.
#[derive(Clone, Debug, PartialEq)]
pub struct Rooms {
    pub rooms: Vec<f64>,
    pub heights: Vec<f64>,
}

/// Les places (et les hauteurs mesurées), dans l'ORDRE DE L'ENTRÉE.
/// `measure(i, place)` laisse le widget réagir à sa place puis rend sa hauteur.
pub fn compute_rooms(
    tops: &[f64],
    opts: RoomOptions,
    mut measure: impl FnMut(usize, f64) -> f64,
) -> Rooms {
    let RoomOptions { pinned, gap, cap } = opts;
    let ordre = ordre_du_texte(tops);
    let mut rooms = vec![0.0; tops.len()];
    let mut heights = vec![0.0; tops.len()];
    let mut bas = f64::NEG_INFINITY;
    for (k, &i) in ordre.iter().enumerate() {
        let suivant = ordre.get(k + 1).copied();
        let (haut, place) = if Some(i) == pinned {
            (tops[i], cap)
        } else {
            let haut = tops[i].max(bas + gap);
            let place = match suivant {
                None => f64::INFINITY,
                Some(s) => 0.0_f64.max(tops[s] - haut - gap),
            };
            (haut, place)
        };
        rooms[i] = place;
        heights[i] = measure(i, place);
        bas = haut + heights[i];
    }
    Rooms { rooms, heights }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LES GRAPPES — les widgets RÉDUITS (des icônes) tombés au même endroit.
//
//  Dans l'ordre du texte, un widget réduit rejoint la grappe en cours si son
//  ancre tombe à moins de `radius` (≈ une hauteur d'icône) du DÉBUT de la grappe ;
//  tout autre widget (entier, ou l'actif) est seul — sans couper la grappe
//  (ouvrir l'un d'eux ne la scinde pas en deux). Une grappe de 2 ou plus est
//  montrée par l'arbitre en UNE tuile (« bulles ») et compte pour UN item du
//  placement.
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterItem {
    pub top: f64,
    /// Réduit à ses icônes (et pas l'actif) : regroupable.
    pub compact: bool,
}

/// Les groupes (index de l'entrée, dans l'ordre du texte) : chaque item dans
/// exactement un.
pub fn cluster_gutter(items: &[ClusterItem], radius: f64) -> Vec<Vec<usize>> {
    let tops: Vec<f64> = items.iter().map(|it| it.top).collect();
    let ordre = ordre_du_texte(&tops);
    let mut groupes: Vec<Vec<usize>> = Vec::new();
    // L'index (dans `groupes`) de la grappe de réduits ouverte.
    let mut en_cours: Option<usize> = None;
    for i in ordre {
        if !items[i].compact {
            groupes.push(vec![i]);
            continue;
        }
        let rejoint = en_cours.filter(|&g| items[i].top - items[groupes[g][0]].top <= radius);
        match rejoint {
            Some(g) => groupes[g].push(i),
            None => {
                groupes.push(vec![i]);
                en_cours = Some(groupes.len() - 1);
            }
        }
    }
    groupes
}
