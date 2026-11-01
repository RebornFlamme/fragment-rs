// Port de app/src/ui/views/doc-widget/anchorLink.ts (la partie PURE)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le LIEN POINTILLÉ d'un widget ancré à son texte — montré au SURVOL de tout
//  DocWidget ancré à quelque chose (un passage, un point), quelle que soit la vue
//  qu'il héberge.
//
//  ★ Le lien CONTINUE le soulignement du passage : il part du bout de sa dernière
//    ligne, AU NIVEAU du soulignement (même couleur, même épaisseur — CSS), file à
//    l'HORIZONTALE, descend (ou monte) en OBLIQUE, puis entre à l'HORIZONTALE dans le
//    widget, au milieu de sa rangée d'icônes. Un passage à la hauteur de l'en-tête :
//    un trait droit. La géométrie est un pur calcul (`trace`, testé).
//
//  ★ Pas porté ici (présentation, fragment-ui) : `LinkMarker.draw` (SVG), la classe
//    `Liens` (calque `addLayer` de la surface, suivi image par image, `queueMicrotask`)
//    et le registre par surface. On garde la CLÉ de géométrie (`lien_cle`,
//    `geometry_key`), qui décide si un redessin est nécessaire.
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Les deux bouts du lien, dans le repère du plan, et les abscisses où il QUITTE
/// l'horizontale du passage (`elbow_x`) et où il REJOINT celle du widget (`entry_x`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkEnds {
    pub from: Point,
    pub to: Point,
    pub elbow_x: f64,
    pub entry_x: f64,
}

/// Le tracé (pur), en trois morceaux : à l'horizontale (la suite du soulignement)
/// jusqu'à `elbow_x`, en OBLIQUE jusqu'à `entry_x` à la hauteur du widget, puis à
/// l'horizontale jusqu'au widget. Les abscisses sont rangées entre départ et
/// arrivée (jamais de retour en arrière) ; à hauteurs égales : un trait droit.
pub fn trace(e: &LinkEnds) -> Vec<Point> {
    let (from, to) = (e.from, e.to);
    if (to.y - from.y).abs() < 0.5 {
        return vec![from, to];
    }
    let lo = from.x.min(to.x);
    let hi = from.x.max(to.x);
    let borne = |x: f64| lo.max(hi.min(x));
    let sens = if to.x >= from.x { 1.0 } else { -1.0 };
    let mut x1 = borne(e.elbow_x);
    let x2 = borne(e.entry_x);
    // Le départ de l'oblique ne passe pas l'arrivée de l'oblique.
    if (x2 - x1) * sens < 0.0 {
        x1 = x2;
    }
    let mut pts = vec![from];
    if (x1 - from.x).abs() >= 0.5 {
        pts.push(Point { x: x1, y: from.y });
    }
    pts.push(Point { x: x2, y: to.y });
    if (to.x - x2).abs() >= 0.5 {
        pts.push(to);
    }
    pts
}

/// `Math.round(n * 10) / 10` de JS : l'arrondi au dixième, demi vers +∞ (`-2.5` → `-2`),
/// jamais `-0`.
pub fn arrondi(n: f64) -> f64 {
    ((n * 10.0 + 0.5).floor() / 10.0) + 0.0
}

/// La clé d'un marqueur de lien : `<id>|x,y x,y …` (points arrondis). Deux marqueurs
/// de même clé sont égaux (`LinkMarker.eq`) : la surface ne redessine pas.
pub fn lien_cle(id: &str, pts: &[Point]) -> String {
    let pts: Vec<String> = pts
        .iter()
        .map(|p| format!("{},{}", arrondi(p.x), arrondi(p.y)))
        .collect();
    format!("{id}|{}", pts.join(" "))
}

/// La géométrie courante de tous les liens, en clé (arrondie au dixième) : `<id>:x,y …;`
/// ou `<id>:-;` pour un lien sans bouts. C'est ce que compare le suivi image par image.
pub fn geometry_key(liens: &[(&str, Option<LinkEnds>)]) -> String {
    let mut k = String::new();
    for (id, ends) in liens {
        match ends {
            Some(e) => {
                let pts: Vec<String> = trace(e)
                    .iter()
                    .map(|p| format!("{},{}", arrondi(p.x), arrondi(p.y)))
                    .collect();
                k.push_str(&format!("{id}:{};", pts.join(" ")));
            }
            None => k.push_str(&format!("{id}:-;")),
        }
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn ends(from: Point, to: Point, elbow_x: f64, entry_x: f64) -> LinkEnds {
        LinkEnds {
            from,
            to,
            elbow_x,
            entry_x,
        }
    }

    #[test]
    fn trois_morceaux_horizontal_suite_du_soulignement_oblique_horizontal_entree_dans_le_widget() {
        assert_eq!(
            trace(&ends(p(100.0, 50.0), p(300.0, 20.0), 260.0, 288.0)),
            vec![p(100.0, 50.0), p(260.0, 50.0), p(288.0, 20.0), p(300.0, 20.0)]
        );
    }

    #[test]
    fn a_hauteurs_egales_un_trait_droit() {
        assert_eq!(
            trace(&ends(p(100.0, 50.0), p(300.0, 50.0), 260.0, 288.0)),
            vec![p(100.0, 50.0), p(300.0, 50.0)]
        );
    }

    #[test]
    fn le_coude_ne_revient_jamais_derriere_le_depart_l_oblique_ne_se_croise_pas() {
        // Le passage finit APRÈS le coude : l'oblique part du bout du passage.
        assert_eq!(
            trace(&ends(p(280.0, 50.0), p(300.0, 20.0), 260.0, 288.0)),
            vec![p(280.0, 50.0), p(288.0, 20.0), p(300.0, 20.0)]
        );
        // Plus étroit que l'entrée : l'oblique va droit au widget.
        assert_eq!(
            trace(&ends(p(295.0, 50.0), p(300.0, 20.0), 260.0, 288.0)),
            vec![p(295.0, 50.0), p(295.0, 20.0), p(300.0, 20.0)]
        );
    }

    #[test]
    fn widget_a_gauche_du_texte_le_meme_trace_en_miroir() {
        assert_eq!(
            trace(&ends(p(300.0, 50.0), p(100.0, 20.0), 140.0, 112.0)),
            vec![p(300.0, 50.0), p(140.0, 50.0), p(112.0, 20.0), p(100.0, 20.0)]
        );
    }

    // ── ajouts du port : la clé de géométrie ──
    #[test]
    fn l_arrondi_suit_math_round_de_js() {
        assert_eq!(arrondi(1.24), 1.2);
        assert_eq!(arrondi(1.25), 1.3);
        // Math.round(-2.5) = -2 : demi vers +∞, pas « loin de zéro ».
        assert_eq!(arrondi(-0.25), -0.2);
        assert_eq!(format!("{}", arrondi(-0.04)), "0");
    }

    #[test]
    fn la_cle_du_marqueur_et_celle_de_la_geometrie() {
        let pts = [p(1.04, 2.0), p(3.0, 4.26)];
        assert_eq!(lien_cle("w", &pts), "w|1,2 3,4.3");
        let e = ends(p(0.0, 0.0), p(10.0, 0.0), 0.0, 0.0);
        assert_eq!(
            geometry_key(&[("a", Some(e)), ("b", None)]),
            "a:0,0 10,0;b:-;"
        );
    }
}
