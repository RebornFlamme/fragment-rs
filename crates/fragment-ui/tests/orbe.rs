//! Tests de l'orbe de la trace (tâche `fr-t7-orbe`). La boucle d'`orb.ts` n'a pas de test TS : les
//! cas sont ceux du bead (état d'après l'étape, image fixe identique deux fois, arrêt hors écran)
//! et ceux de `orbStateOf` (`traceModel.test.ts`, côté `fragment-agent`), joués jusqu'à la forme.
//! Le moteur est comparé VALEUR POUR VALEUR à la bibliothèque d'origine : les vecteurs dorés
//! ci-dessous ont été calculés par le moteur TypeScript (neuf états, tailles 20 et 64, horloge à
//! 0,6 s et 7,3 s) — nombre de points et de traits, sommes de x, y, r, encre, opacité, une
//! signature des traits, et le premier point dans l'ordre de peinture.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use fragment_agent::contrat::ResultatOutil;
use fragment_agent::trace_model::{OrbState, OutilEntrant, TraceBuilder, orb_state_of};
use fragment_ui::orbe::{
    ETATS, Forme, HorlogeOrbes, Marque, Orbes, Point, T_FIGE, Taille, element_orbe, forme_de,
    image_etat, libelle, plan, resoudre, visible,
};

type Dore = (f32, OrbState, f64, usize, usize, [f64; 6], [f64; 3]);

const DORES: [Dore; 36] = [
    (
        20.0,
        OrbState::Working,
        0.6,
        39,
        0,
        [
            390.000000000000,
            390.000000000000,
            21.2700739612520,
            23.3100000000000,
            19.5000000000000,
            0.00000000000000,
        ],
        [9.72973140704094, 9.91089356816720, 0.425401479225040],
    ),
    (
        20.0,
        OrbState::Working,
        7.3,
        39,
        0,
        [
            390.000000000000,
            390.000000000000,
            21.2700739612520,
            23.3100000000000,
            19.5000000000000,
            0.00000000000000,
        ],
        [9.77310965051917, 5.81083173370009, 0.425401479225040],
    ),
    (
        20.0,
        OrbState::Searching,
        0.6,
        54,
        0,
        [
            540.000000000000,
            540.000000000000,
            28.1825149392764,
            18.9000000000000,
            26.5297134557902,
            0.00000000000000,
        ],
        [11.7410695618800, 10.8419742883168, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Searching,
        7.3,
        54,
        0,
        [
            540.000000000000,
            540.000000000000,
            27.4348506262503,
            18.9000000000000,
            24.4363672779999,
            0.00000000000000,
        ],
        [11.5575452341800, 10.8375194894384, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Solving,
        0.6,
        30,
        0,
        [
            299.402517032501,
            285.517977099882,
            17.1928055685767,
            9.37352042382999,
            30.0000000000000,
            0.00000000000000,
        ],
        [9.80104250746503, 10.7112910228675, 0.338069037027425],
    ),
    (
        20.0,
        OrbState::Solving,
        7.3,
        30,
        0,
        [
            291.912127322861,
            285.401573950068,
            16.8377043174444,
            9.91513730997507,
            30.0000000000000,
            0.00000000000000,
        ],
        [10.2027787253886, 7.00306671307824, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Listening,
        0.6,
        42,
        0,
        [
            420.000000000000,
            412.151813593130,
            19.8376584789025,
            15.6732660906581,
            42.0000000000000,
            0.00000000000000,
        ],
        [10.6027866929898, 9.55545523618433, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Listening,
        7.3,
        42,
        0,
        [
            420.000000000000,
            412.756747103289,
            21.1476005042914,
            15.0574456805083,
            42.0000000000000,
            0.00000000000000,
        ],
        [9.87290231001842, 9.49461046308365, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Connecting,
        0.6,
        9,
        0,
        [
            93.9136257606252,
            87.1331001969339,
            6.64261434597507,
            2.54287772453840,
            8.83714230763489,
            0.00000000000000,
        ],
        [13.5049875012527, 11.9201651455035, 0.479917692264163],
    ),
    (
        20.0,
        OrbState::Connecting,
        7.3,
        9,
        0,
        [
            84.1834889370862,
            86.9762547708815,
            6.44148436885038,
            2.66958940490083,
            8.83247536829930,
            0.00000000000000,
        ],
        [10.9773901275368, 14.3040919351390, 0.351901548341329],
    ),
    (
        20.0,
        OrbState::Weaving,
        0.6,
        35,
        0,
        [
            351.485876542971,
            352.230812805668,
            15.2123304914496,
            19.1214102344211,
            16.5991622174274,
            0.00000000000000,
        ],
        [8.37135353491813, 9.57919050292147, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Weaving,
        7.3,
        35,
        0,
        [
            349.980967834889,
            352.906374030499,
            15.1966435030578,
            19.1360520668072,
            16.5658976367380,
            0.00000000000000,
        ],
        [11.3167219377244, 7.78801115038101, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Composing,
        0.6,
        208,
        0,
        [
            2079.96863939365,
            2080.08434087080,
            76.3427941347031,
            86.2393736097147,
            141.685589016759,
            0.00000000000000,
        ],
        [10.0000000000000, 12.2000039332226, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Composing,
        7.3,
        208,
        0,
        [
            2079.96634154267,
            2080.10613162149,
            76.2635209054793,
            86.2400731252548,
            141.684635131932,
            0.00000000000000,
        ],
        [10.0000000000000, 13.0939266458538, 0.300000000000000],
    ),
    (
        20.0,
        OrbState::Breathing,
        0.6,
        120,
        0,
        [
            1200.00000000000,
            1200.00000000000,
            64.0715399352799,
            48.3428571428571,
            84.0000000000000,
            0.00000000000000,
        ],
        [3.91488719476571, 14.4210932407127, 0.415639106357423],
    ),
    (
        20.0,
        OrbState::Breathing,
        7.3,
        120,
        0,
        [
            1200.00000000000,
            1200.00000000000,
            64.0715399352799,
            48.3428571428571,
            84.0000000000000,
            0.00000000000000,
        ],
        [9.21028438168155, 2.48635779212656, 0.415410088007483],
    ),
    (
        20.0,
        OrbState::Shaping,
        0.6,
        18,
        0,
        [
            180.000000000000,
            180.000000000000,
            14.9614857000000,
            1.80000000000000,
            18.0000000000000,
            0.00000000000000,
        ],
        [10.0000000000000, 3.13253821444491, 0.831193650000000],
    ),
    (
        20.0,
        OrbState::Shaping,
        7.3,
        18,
        0,
        [
            180.000000000000,
            180.000000000000,
            14.9614857000000,
            1.80000000000000,
            18.0000000000000,
            0.00000000000000,
        ],
        [10.0000000000000, 3.16698883412159, 0.831193650000000],
    ),
    (
        64.0,
        OrbState::Working,
        0.6,
        516,
        0,
        [
            16512.0000000000,
            16512.0000000000,
            199.464571340436,
            352.440000000003,
            204.000000000000,
            0.00000000000000,
        ],
        [30.8507497190211, 30.6915179731535, 0.356186734536494],
    ),
    (
        64.0,
        OrbState::Working,
        7.3,
        516,
        0,
        [
            16512.0000000000,
            16512.0000000000,
            199.464571340436,
            352.440000000003,
            204.000000000000,
            0.00000000000000,
        ],
        [33.8472301048975, 31.6610248101463, 0.356186734536494],
    ),
    (
        64.0,
        OrbState::Searching,
        0.6,
        204,
        0,
        [
            6528.00000000000,
            6528.00000000000,
            134.891070118738,
            71.4000000000000,
            92.0321676761757,
            0.00000000000000,
        ],
        [31.9917040217082, 32.0986868729676, 0.300000000000000],
    ),
    (
        64.0,
        OrbState::Searching,
        7.3,
        204,
        0,
        [
            6528.00000000000,
            6528.00000000000,
            141.319322685049,
            71.4000000000000,
            100.968118406215,
            0.00000000000000,
        ],
        [32.3797282256741, 34.1708869976021, 0.300000000000000],
    ),
    (
        64.0,
        OrbState::Solving,
        0.6,
        138,
        0,
        [
            4394.51101400339,
            4317.06912949287,
            87.8670549519625,
            43.3223031699793,
            138.000000000000,
            0.00000000000000,
        ],
        [30.9004891629490, 29.0332501751130, 0.376573163838234],
    ),
    (
        64.0,
        OrbState::Solving,
        7.3,
        138,
        0,
        [
            4416.00000000000,
            4416.00000000000,
            87.3977640563354,
            43.5400000000000,
            138.000000000000,
            0.00000000000000,
        ],
        [32.0808016809426, 31.0903369836574, 0.374210060340875],
    ),
    (
        64.0,
        OrbState::Listening,
        0.6,
        134,
        0,
        [
            4288.00000000000,
            4163.91911034651,
            87.6612013265497,
            46.4278993533575,
            134.000000000000,
            0.00000000000000,
        ],
        [32.1003782498222, 27.0064720829702, 0.300000000000000],
    ),
    (
        64.0,
        OrbState::Listening,
        7.3,
        134,
        0,
        [
            4288.00000000000,
            4199.94866331162,
            92.2867193132475,
            44.4780538504266,
            134.000000000000,
            0.00000000000000,
        ],
        [29.5200294416304, 26.7564101728733, 0.318114561329577],
    ),
    (
        64.0,
        OrbState::Connecting,
        0.6,
        48,
        86,
        [
            1610.62576695275,
            1477.78793660716,
            43.5102671587072,
            13.8151143996473,
            46.4585479200793,
            5923.51901952175,
        ],
        [32.5202024451013, 25.8002160832589, 0.655359146555084],
    ),
    (
        64.0,
        OrbState::Connecting,
        7.3,
        48,
        85,
        [
            1405.26906979354,
            1469.36256024309,
            43.4126159486587,
            13.6506963145998,
            46.3257528213496,
            5029.33210125633,
        ],
        [25.1388628717362, 34.5043919470169, 0.410616165041174],
    ),
    (
        64.0,
        OrbState::Weaving,
        0.6,
        153,
        0,
        [
            4898.16995299757,
            4944.13687854421,
            88.3409935536168,
            83.9957781627400,
            71.1637257972446,
            0.00000000000000,
        ],
        [31.1646056649561, 29.2805633553854, 0.316610430699106],
    ),
    (
        64.0,
        OrbState::Weaving,
        7.3,
        153,
        0,
        [
            4893.37098383468,
            4845.62926599482,
            88.7934084714062,
            83.7099916731475,
            71.2542836715493,
            0.00000000000000,
        ],
        [28.4812816132924, 34.0881249670753, 0.316610430699106],
    ),
    (
        64.0,
        OrbState::Composing,
        0.6,
        566,
        0,
        [
            18114.1668499175,
            18113.1529848763,
            311.215333592134,
            239.887196223524,
            377.576395021236,
            0.00000000000000,
        ],
        [28.9692094963514, 36.1320893011860, 0.300000000000000],
    ),
    (
        64.0,
        OrbState::Composing,
        7.3,
        566,
        0,
        [
            18116.2535584368,
            18113.2822786225,
            311.220523973243,
            239.888493582254,
            377.574625895695,
            0.00000000000000,
        ],
        [32.0000000000000, 36.5177559482284, 0.300000000000000],
    ),
    (
        64.0,
        OrbState::Breathing,
        0.6,
        484,
        0,
        [
            15488.0000000000,
            15488.0000000000,
            308.392648665095,
            192.720000000000,
            338.799999999999,
            0.00000000000000,
        ],
        [12.1252795263230, 44.7727072805607, 0.467724193610262],
    ),
    (
        64.0,
        OrbState::Breathing,
        7.3,
        484,
        0,
        [
            15488.0000000000,
            15488.0000000000,
            308.392648665095,
            192.720000000000,
            338.799999999999,
            0.00000000000000,
        ],
        [35.3604579630304, 8.62750148400058, 0.467768702518552],
    ),
    (
        64.0,
        OrbState::Shaping,
        0.6,
        24,
        0,
        [
            768.000000000000,
            768.284239896287,
            24.9407424000000,
            2.40000000000000,
            24.0000000000000,
            0.00000000000000,
        ],
        [32.0000000000000, 10.1487028537356, 1.03919760000000],
    ),
    (
        64.0,
        OrbState::Shaping,
        7.3,
        24,
        0,
        [
            768.000000000000,
            808.580943647861,
            24.9407424000000,
            2.40000000000000,
            24.0000000000000,
            0.00000000000000,
        ],
        [32.0000000000000, 8.40704288757767, 1.03919760000000],
    ),
];

fn proche(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-7 * b.abs().max(1.0)
}

#[test]
fn moteur_identique_a_la_bibliotheque_d_origine() {
    for (cote, etat, secondes, nb_points, nb_traits, sommes, premier) in DORES {
        let img = image_etat(etat, cote, secondes);
        let ctx = format!("{etat:?} {cote} à {secondes}s");
        assert_eq!(img.points.len(), nb_points, "{ctx}");
        assert_eq!(img.segments.len(), nb_traits, "{ctx}");
        let somme = |f: fn(&Point) -> f64| img.points.iter().map(f).sum::<f64>();
        let calc = [
            somme(|p| p.x),
            somme(|p| p.y),
            somme(|p| p.r),
            somme(|p| p.blanc),
            somme(|p| p.a.unwrap_or(1.0)),
            img.segments
                .iter()
                .map(|s| s.x1 + s.y2 + s.a.unwrap_or(1.0))
                .sum(),
        ];
        for (c, d) in calc.iter().zip(sommes) {
            assert!(proche(*c, d), "{ctx} : {calc:?} ≠ {sommes:?}");
        }
        let p0 = img.points[0];
        for (c, d) in [p0.x, p0.y, p0.r].iter().zip(premier) {
            assert!(proche(*c, d), "{ctx} : premier point {p0:?} ≠ {premier:?}");
        }
    }
}

// ── orbStateOf → la forme peinte ─────────────────────────────────────────────

fn outil(id: &str, nom: &str) -> OutilEntrant {
    OutilEntrant {
        id: id.into(),
        name: nom.into(),
        input: None,
    }
}

fn ok() -> ResultatOutil {
    ResultatOutil {
        is_error: false,
        text: "ok".into(),
    }
}

#[test]
fn orb_state_of_l_animation_de_l_orbe_suit_l_etape() {
    let t0 = 1_000_000;
    let mut b = TraceBuilder::new(t0);
    let forme = |b: &TraceBuilder| forme_de(orb_state_of(&b.snapshot()));
    // Avant tout, et pendant une pensée : on résout.
    assert_eq!(forme(&b), Forme::Rubik);
    b.thinking("Je cherche.", t0 + 10, true);
    assert_eq!(forme(&b), Forme::Rubik);
    b.tool(outil("1", "Grep"), t0 + 20);
    assert_eq!(forme(&b), Forme::Globe);
    b.result("1", ok(), t0 + 30);
    assert_eq!(forme(&b), Forme::Rubik);
    b.tool(outil("2", "Edit"), t0 + 40);
    assert_eq!(forme(&b), Forme::Echarpe);
    b.result("2", ok(), t0 + 50);
    b.tool(outil("3", "Bash"), t0 + 60);
    assert_eq!(forme(&b), Forme::Orbites);
    b.result("3", ok(), t0 + 70);
    // La réponse s'écrit : on compose.
    b.ecrire(t0 + 80);
    assert_eq!(forme(&b), Forme::Echarpe);
}

#[test]
fn changer_d_etat_change_l_image_mais_pas_l_horloge() {
    let h = HorlogeOrbes::new();
    assert_eq!(h.secondes(5_000), 0.0);
    let s = h.secondes(7_500);
    assert!((s - 2.5).abs() < 1e-12);
    // Même instant, deux gestes : deux images différentes sur la même horloge.
    assert_ne!(
        image_etat(OrbState::Solving, 20.0, s),
        image_etat(OrbState::Searching, 20.0, s)
    );
    // Une horloge murale qui recule ne fait pas tourner à l'envers.
    assert_eq!(h.secondes(4_000), 0.0);
}

// ── image fixe (mouvement réduit) ────────────────────────────────────────────

#[test]
fn image_fixe_identique_deux_fois() {
    for etat in ETATS {
        let a = plan(&image_etat(etat, 20.0, T_FIGE), true, Some([220, 221, 222]));
        let b = plan(&image_etat(etat, 20.0, T_FIGE), true, Some([220, 221, 222]));
        assert!(!a.is_empty(), "{etat:?}");
        assert_eq!(a, b, "{etat:?}");
    }
}

#[test]
fn l_image_animee_bouge_avec_le_temps() {
    for etat in ETATS {
        assert_ne!(
            image_etat(etat, 20.0, 1.0),
            image_etat(etat, 20.0, 1.5),
            "{etat:?}"
        );
    }
}

// ── arrêt hors écran ─────────────────────────────────────────────────────────

#[test]
fn arret_hors_ecran() {
    use gpui_kit::{Bounds, point, px, size};
    let carre = |x: f32, y: f32| Bounds::new(point(px(x), px(y)), size(px(20.0), px(20.0)));
    let fenetre = Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(300.0)));
    assert!(visible(carre(10.0, 10.0), fenetre));
    assert!(
        visible(carre(-10.0, 290.0), fenetre),
        "à cheval sur le bord"
    );
    assert!(!visible(carre(10.0, 320.0), fenetre), "défilée sous la vue");
    assert!(!visible(carre(10.0, -40.0), fenetre), "défilée au-dessus");
}

// ── encre ────────────────────────────────────────────────────────────────────

#[test]
fn l_encre_suit_la_teinte_du_theme_et_s_inverse_en_sombre() {
    let teinte = Some([200, 100, 50]);
    let img = image_etat(OrbState::Shaping, 20.0, T_FIGE);
    let canaux = |sombre| match plan(&img, sombre, teinte)[0] {
        Marque::Disque { couleur, .. } | Marque::Trait { couleur, .. } => couleur,
    };
    // Morph : encre 0,1 sur tout le contour. Sombre : 90 % de la teinte ; clair : 10 % vers le blanc.
    assert_eq!(
        canaux(true),
        [180.0 / 255.0, 90.0 / 255.0, 45.0 / 255.0, 1.0]
    );
    assert_eq!(
        canaux(false),
        [206.0 / 255.0, 116.0 / 255.0, 71.0 / 255.0, 1.0]
    );
}

// ── réglages livrés ──────────────────────────────────────────────────────────

#[test]
fn les_reglages_livres_sont_mis_a_l_echelle() {
    let r = resoudre(OrbState::Working, Taille::T20);
    assert_eq!(r.forme, Forme::Orbites);
    assert_eq!(r.vitesse, 3.9);
    assert_eq!(r.reglages.orbit_n, Some(3.0));
    assert_eq!(r.reglages.ghost_n, Some(10.0));
    // L'anneau n'a pas de sphère fantôme : le multiplicateur ne la ressuscite pas.
    let anneau = resoudre(OrbState::Breathing, Taille::T20);
    assert_eq!(anneau.reglages.ghost_n, Some(0.0));
    assert_eq!(anneau.reglages.face_on, Some(1.0));
    assert_eq!(Taille::proche(20.0), Taille::T20);
    assert_eq!(Taille::proche(64.0), Taille::T64);
}

#[test]
fn chaque_orbe_a_un_nom() {
    assert_eq!(ETATS.len(), 9);
    assert_eq!(libelle(OrbState::Searching), "Globe");
    assert_eq!(libelle(OrbState::Breathing), "Anneau");
}

// ── headless : la toile se peint ─────────────────────────────────────────────

#[test]
fn headless_la_toile_de_l_orbe_se_peint_animee_et_fixe() {
    use fragment_ui::agent_trace::FournisseurOrbe;
    use fragment_ui::theme_global::{CouleurGpui, couleur};
    use gpui_kit::{
        AnyElement, Context, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
        Window, div,
    };

    struct Banc {
        enfants: Vec<AnyElement>,
    }
    impl Render for Banc {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .children(std::mem::take(&mut self.enfants))
        }
    }

    let encre: CouleurGpui = couleur(fragment_theme::Theme::sombre().palette.text_normal);
    let mut cx = TestAppContext::single();
    let (vue, vcx) = cx.add_window_view(|_, _| Banc {
        enfants: Vec::new(),
    });
    for reduit in [false, true] {
        let f = Orbes::new(reduit);
        vcx.update(|w, cx| {
            vue.update(cx, |b, _| {
                b.enfants = ETATS
                    .iter()
                    .map(|e| f.orbe(*e, 20.0, encre, 1_000))
                    .collect();
                b.enfants
                    .push(element_orbe(OrbState::Connecting, 64.0, encre, 3.0, true));
            });
            w.draw(cx).clear(cx);
        });
    }
}
