// Port de app/src/ui/views/chat/scroll/followModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le DÉFILEMENT du chat — le modèle PUR.
//
//  Principe (« What Makes a Great Streaming Chat Experience ») : ne JAMAIS
//  déplacer le lecteur contre son intention. Un état unique, `FollowMode` :
//
//    free  (défaut)  on lit librement : rien ne bouge, jamais ;
//    turn            on vient d'ENVOYER : la question est placée près du HAUT,
//                    la réponse grandit dans l'espace libre (la réserve), sans
//                    défiler ;
//    live            on est au bord vivant et on l'a CHOISI (« Aller au plus
//                    récent », Fin, ou on y est descendu soi-même pendant un
//                    flux) : on suit le bas.
//
//  TOUTE intention de l'utilisateur (molette, toucher, touches de défilement,
//  barre de défilement, sélection, lien, recherche, focus dans un message) rend
//  `free`. Le modèle dit QUOI faire ; le contrôleur (fragment-ui) le fait.
//
//  Les longueurs sont des pixels logiques (`f64`) ; `Math.round` du TS = [`arrondi`]
//  (la moitié vers +∞, comme JS, pas « loin de zéro » comme `f64::round`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowMode {
    Free,
    Turn,
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowEvent {
    /// L'utilisateur envoie un message.
    Send,
    /// « Aller au plus récent », la touche Fin.
    Jump,
    /// Un geste de l'utilisateur (molette, sélection, recherche…).
    Intent,
    /// L'utilisateur est arrivé LUI-MÊME au bas (un défilement qui n'est pas le nôtre).
    ReachedBottom { streaming: bool },
    /// Le tour est fini (réponse complète, arrêt, erreur).
    TurnEnd,
}

/// `Math.round` de JS : la moitié vers +∞.
fn arrondi(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub fn next_mode(mode: FollowMode, e: FollowEvent) -> FollowMode {
    match e {
        FollowEvent::Send => FollowMode::Turn,
        FollowEvent::Jump => FollowMode::Live,
        FollowEvent::Intent => FollowMode::Free,
        // Descendre soi-même au bord vivant PENDANT un flux, c'est le choisir.
        FollowEvent::ReachedBottom { streaming } => {
            if streaming && mode == FollowMode::Free {
                FollowMode::Live
            } else {
                mode
            }
        }
        // `live` reste jusqu'à la prochaine intention ; `turn` a fini son travail.
        FollowEvent::TurnEnd => {
            if mode == FollowMode::Turn {
                FollowMode::Free
            } else {
                mode
            }
        }
    }
}

/// La RÉSERVE sous le dernier tour (le motif de ChatGPT et Claude) : assez pour que
/// la question puisse monter à `context` px du haut même dans un fil court — elle
/// RÉTRÉCIT à mesure que la réponse grandit, rien ne bouge à l'écran.
///   visible  : la hauteur visible du défileur ;
///   context  : l'air gardé au-dessus de la question (≈ 2 lignes du tour précédent) ;
///   last_turn : la hauteur du dernier tour (question + réponse).
pub fn reserve_height(visible: f64, context: f64, last_turn: f64) -> f64 {
    arrondi(visible - context - last_turn).max(0.0)
}

/// Le `scrollTop` qui place un message (son haut à `top`, en px du contenu) à `context` px du haut.
pub fn turn_target(top: f64, context: f64) -> f64 {
    arrondi(top - context).max(0.0)
}

/// Un message tel que le contrôleur le mesure : son haut et son bas, en px du contenu défilant.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageBox {
    pub id: String,
    pub top: f64,
    pub bottom: f64,
}

/// L'ANCRE DE LECTURE : le 1er message visible et l'écart de son haut au haut de l'écran.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingAnchor {
    pub message: String,
    pub dy: f64,
}

/// Le premier message dont le bas dépasse le haut de l'écran, et où son haut en est.
pub fn reading_anchor(boxes: &[MessageBox], scroll_top: f64) -> Option<ReadingAnchor> {
    let b = boxes
        .iter()
        .find(|x| x.bottom > scroll_top + 1.0)
        .or_else(|| boxes.last())?;
    Some(ReadingAnchor {
        message: b.id.clone(),
        dy: arrondi(b.top - scroll_top),
    })
}

/// Le `scrollTop` qui rend une ancre de lecture, ou `None` si son message n'est pas (encore) là.
pub fn anchor_target(a: &ReadingAnchor, boxes: &[MessageBox]) -> Option<f64> {
    let b = boxes.iter().find(|x| x.id == a.message)?;
    Some(arrondi(b.top - a.dy).max(0.0))
}

/// Le sens d'un déplacement de tour en tour (`1 | -1` en TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sens {
    Suivant,
    Precedent,
}

/// Le tour (index dans `turn_tops`, tops croissants) qui suit ou précède la lecture courante.
/// `None` : il n'y en a pas (`-1` en TS).
pub fn neighbour_turn(
    turn_tops: &[f64],
    scroll_top: f64,
    context: f64,
    dir: Sens,
) -> Option<usize> {
    // Où le défilement IRAIT pour chaque tour : le suivant est le 1er plus bas, le précédent le dernier plus haut.
    let cibles: Vec<f64> = turn_tops.iter().map(|&t| turn_target(t, context)).collect();
    match dir {
        Sens::Suivant => cibles.iter().position(|&c| c > scroll_top + 4.0),
        Sens::Precedent => cibles.iter().rposition(|&c| c < scroll_top - 4.0),
    }
}

/// L'ANNONCEUR (lecteurs d'écran) : au plus un message toutes les `gap` ms, le
/// dernier REMPLACE ceux qui attendent (on dit l'état, pas chaque étape).
/// Rend quand annoncer ce qui vient d'arriver (`now` si on peut tout de suite).
pub fn announce_at(now: f64, last: f64, gap: f64) -> f64 {
    now.max(last + gap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [FollowMode; 3] = [FollowMode::Free, FollowMode::Turn, FollowMode::Live];

    fn boite(id: &str, top: f64, bottom: f64) -> MessageBox {
        MessageBox {
            id: id.into(),
            top,
            bottom,
        }
    }

    mod follow_model_les_modes {
        use super::*;

        #[test]
        fn envoyer_turn_quel_que_soit_le_mode() {
            for m in MODES {
                assert_eq!(next_mode(m, FollowEvent::Send), FollowMode::Turn);
            }
        }

        #[test]
        fn toute_intention_free() {
            for m in MODES {
                assert_eq!(next_mode(m, FollowEvent::Intent), FollowMode::Free);
            }
        }

        #[test]
        fn aller_au_plus_recent_live() {
            assert_eq!(
                next_mode(FollowMode::Free, FollowEvent::Jump),
                FollowMode::Live
            );
        }

        #[test]
        fn arriver_soi_meme_au_bas_pendant_un_flux_live_hors_flux_rien() {
            let bas = |streaming| FollowEvent::ReachedBottom { streaming };
            assert_eq!(next_mode(FollowMode::Free, bas(true)), FollowMode::Live);
            assert_eq!(next_mode(FollowMode::Free, bas(false)), FollowMode::Free);
            assert_eq!(next_mode(FollowMode::Turn, bas(true)), FollowMode::Turn);
        }

        #[test]
        fn fin_du_tour_turn_free_live_reste_live() {
            assert_eq!(
                next_mode(FollowMode::Turn, FollowEvent::TurnEnd),
                FollowMode::Free
            );
            assert_eq!(
                next_mode(FollowMode::Live, FollowEvent::TurnEnd),
                FollowMode::Live
            );
            assert_eq!(
                next_mode(FollowMode::Free, FollowEvent::TurnEnd),
                FollowMode::Free
            );
        }
    }

    mod follow_model_geometrie {
        use super::*;

        #[test]
        fn la_reserve_retrecit_quand_le_tour_grandit_jamais_negative() {
            assert_eq!(reserve_height(800.0, 48.0, 100.0), 652.0);
            assert_eq!(reserve_height(800.0, 48.0, 700.0), 52.0);
            assert_eq!(reserve_height(800.0, 48.0, 2000.0), 0.0);
        }

        #[test]
        fn placer_un_tour_son_haut_a_context_px_du_haut_jamais_avant_0() {
            assert_eq!(turn_target(1000.0, 48.0), 952.0);
            assert_eq!(turn_target(20.0, 48.0), 0.0);
        }

        #[test]
        fn ancre_de_lecture_le_1er_message_dont_le_bas_passe_le_haut_de_l_ecran() {
            let b = [
                boite("a", 0.0, 300.0),
                boite("b", 310.0, 900.0),
                boite("c", 910.0, 1200.0),
            ];
            assert_eq!(
                reading_anchor(&b, 400.0),
                Some(ReadingAnchor {
                    message: "b".into(),
                    dy: -90.0
                })
            );
            assert_eq!(
                reading_anchor(&b, 0.0),
                Some(ReadingAnchor {
                    message: "a".into(),
                    dy: 0.0
                })
            );
            assert_eq!(reading_anchor(&[], 0.0), None);
        }

        #[test]
        fn rejouer_une_ancre_meme_ecart_apres_une_reconstruction_qui_decale_tout() {
            let avant = [boite("a", 0.0, 300.0), boite("b", 310.0, 900.0)];
            let a = reading_anchor(&avant, 400.0).expect("ancre");
            let apres = [
                boite("z", 0.0, 500.0),
                boite("a", 510.0, 810.0),
                boite("b", 820.0, 1410.0),
            ];
            assert_eq!(anchor_target(&a, &apres), Some(910.0));
            let inconnue = ReadingAnchor {
                message: "x".into(),
                dy: 0.0,
            };
            assert_eq!(anchor_target(&inconnue, &apres), None);
        }

        #[test]
        fn tour_voisin_suivant_precedent_de_la_lecture() {
            let tops = [0.0, 500.0, 1000.0, 1500.0];
            assert_eq!(neighbour_turn(&tops, 452.0, 48.0, Sens::Suivant), Some(2));
            assert_eq!(neighbour_turn(&tops, 452.0, 48.0, Sens::Precedent), Some(0));
            assert_eq!(neighbour_turn(&tops, 1452.0, 48.0, Sens::Suivant), None);
            assert_eq!(neighbour_turn(&tops, 0.0, 48.0, Sens::Precedent), None);
        }

        #[test]
        fn annonceur_au_plus_un_message_toutes_les_2_s() {
            assert_eq!(announce_at(10_000.0, 0.0, 2000.0), 10_000.0);
            assert_eq!(announce_at(10_500.0, 10_000.0, 2000.0), 12_000.0);
        }
    }
}
