//! Les mesures du spike S5 : chaque voie rend les 12 formules × 2 thèmes, rastérise par resvg et
//! se compare à la capture de Fragment. Les tableaux s'impriment (`--nocapture`) et les images
//! (candidat, diff) vont dans le dossier temporaire de cargo, `spike-maths/<voie>/`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::Instant;

use spike_maths::diff::{Ecart, comparer, image_de_diff};
use spike_maths::formules::{FORMULES, Formule};
use spike_maths::raster::{RenduSvg, peindre, peindre_sur_echantillonne, placer};
use spike_maths::reference::{self, Mesure, couleur};

fn sortie(voie: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("spike-maths")
        .join(voie);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn formule(m: &Mesure) -> Formule {
    *FORMULES.iter().find(|f| f.nom == m.formule).unwrap()
}

/// Compare un rendu à sa capture, écrit les images, rend l'écart et l'écart de taille (px).
fn mesurer(voie: &str, m: &Mesure, rendu: &RenduSvg, meme_boite: bool) -> (Ecart, f64, f64) {
    let reference = reference::image(m).unwrap();
    let candidat = peindre(rendu, m, placer(rendu, m, meme_boite)).unwrap();
    let d = sortie(voie);
    candidat.save(d.join(format!("{}.png", m.nom))).unwrap();
    image_de_diff(&reference, &candidat)
        .save(d.join(format!("{}-diff.png", m.nom)))
        .unwrap();
    let e = comparer(&reference, &candidat, couleur(&m.fond).unwrap());
    (e, rendu.largeur - m.largeur, rendu.hauteur - m.hauteur)
}

/// Une ligne de tableau : la scène et, si rendue, son écart et l'écart de boîte.
type Ligne = (String, Option<(Ecart, f64, f64)>);

fn imprimer(voie: &str, lignes: &[Ligne]) {
    println!("\n=== {voie} ===");
    println!(
        "{:<26} {:>6} {:>7} {:>6} {:>5} {:>7} {:>7} {:>7}",
        "scène", "px", "≠ %", ">16", "max", "encre", "Δl px", "Δh px"
    );
    let (mut tot, mut diff, mut fort, mut max) = (0u32, 0u32, 0u32, 0u8);
    for (nom, r) in lignes {
        match r {
            Some((e, dl, dh)) => {
                println!(
                    "{nom:<26} {:>6} {:>7.2} {:>6} {:>5} {:>+6.1}% {dl:>+7.2} {dh:>+7.2}",
                    e.pixels,
                    e.pourcent(),
                    e.au_dela_16,
                    e.delta_max,
                    100.0 * e.encre_relative()
                );
                tot += e.pixels;
                diff += e.differents;
                fort += e.au_dela_16;
                max = max.max(e.delta_max);
            }
            None => println!("{nom:<26} non rendu"),
        }
    }
    println!(
        "TOTAL {voie} : {diff}/{tot} px différents ({:.2} %), {fort} au-delà de 16 ({:.2} %), delta max {max}",
        100.0 * f64::from(diff) / f64::from(tot.max(1)),
        100.0 * f64::from(fort) / f64::from(tot.max(1))
    );
}

#[test]
fn les_captures_couvrent_toutes_les_formules() {
    let mesures = reference::mesures().unwrap();
    assert_eq!(mesures.len(), FORMULES.len() * 2);
    for m in &mesures {
        assert_eq!(
            formule(m).tex,
            m.tex,
            "{} : source différente de la capture",
            m.nom
        );
        assert_eq!(formule(m).bloc, m.bloc);
    }
}

#[cfg(feature = "voie-a")]
mod voie_a {
    use super::*;
    use spike_maths::voie_a::{MathJax, normaliser, svg_pour_resvg};

    /// MathJax veut plus de pile native que les 2 Mio d'un fil de test (voir voie_a::PILE).
    fn grand_fil(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn mathjax_dans_quickjs_produit_le_svg_de_fragment() {
        grand_fil(svg_de_fragment);
    }

    fn svg_de_fragment() {
        let mj = MathJax::new().unwrap();
        for m in reference::mesures().unwrap() {
            let conteneur = mj
                .conteneur(&m.tex, m.bloc, m.em_px, m.ex_px, 800.0)
                .unwrap();
            let debut = conteneur.find("<svg").unwrap();
            let fin = conteneur.rfind("</svg>").unwrap() + 6;
            assert_eq!(
                normaliser(&conteneur[debut..fin]),
                normaliser(&m.svg),
                "{} : SVG différent",
                m.nom
            );
        }
    }

    #[test]
    fn mathjax_meme_boite_que_chromium() {
        grand_fil(meme_boite);
    }

    fn meme_boite() {
        let mj = MathJax::new().unwrap();
        for m in reference::mesures().unwrap() {
            let r = mj
                .rendre(
                    &m.tex,
                    m.bloc,
                    m.ex_px,
                    m.em_px,
                    couleur(&m.couleur).unwrap(),
                )
                .unwrap();
            assert_eq!(
                (r.largeur, r.hauteur),
                (m.largeur, m.hauteur),
                "{} : boîte",
                m.nom
            );
        }
    }

    #[test]
    fn mesures_voie_a() {
        grand_fil(mesures_a);
    }

    fn mesures_a() {
        let t0 = Instant::now();
        let mj = MathJax::new().unwrap();
        let init = t0.elapsed();
        let mesures = reference::mesures().unwrap();
        let t1 = Instant::now();
        let premier = mj
            .conteneur(&mesures[0].tex, false, 16.0, 8.0, 800.0)
            .unwrap();
        let premier_rendu = t1.elapsed();
        assert!(premier.contains("<svg"));
        // Rendu chaud : les 12 formules, 10 fois (sans cache : MathJax refait tout).
        let t2 = Instant::now();
        for _ in 0..10 {
            for f in FORMULES {
                mj.conteneur(f.tex, f.bloc, 16.0, 8.064, 800.0).unwrap();
            }
        }
        let chaud = t2.elapsed() / (10 * FORMULES.len() as u32);
        // Rastérisation seule (resvg) d'une formule de bloc.
        let somme = mesures.iter().find(|m| m.formule == "somme").unwrap();
        let rs = mj
            .rendre(&somme.tex, true, somme.ex_px, somme.em_px, [255, 255, 255])
            .unwrap();
        let t3 = Instant::now();
        for _ in 0..20 {
            peindre(&rs, somme, (2.0, 2.0)).unwrap();
        }
        let raster = t3.elapsed() / 20;
        println!(
            "\nvoie A : VM + paquet {init:?} ; 1er rendu {premier_rendu:?} ; rendu chaud moyen {chaud:?} ; resvg (somme) {raster:?}"
        );
        #[cfg(feature = "voie-a-jit")]
        {
            let t0 = Instant::now();
            let jit = MathJax::avec_jit().unwrap();
            let init = t0.elapsed();
            let t2 = Instant::now();
            for _ in 0..10 {
                for f in FORMULES {
                    jit.conteneur(f.tex, f.bloc, 16.0, 8.064, 800.0).unwrap();
                }
            }
            println!(
                "voie A + JIT : VM + paquet {init:?} ; rendu chaud moyen {:?}",
                t2.elapsed() / (10 * FORMULES.len() as u32)
            );
        }
        // Le temps par formule (chaud, moyenne de 10).
        for f in FORMULES {
            let t = Instant::now();
            for _ in 0..10 {
                mj.conteneur(f.tex, f.bloc, 16.0, 8.064, 800.0).unwrap();
            }
            println!("  {:<18} {:?}", f.nom, t.elapsed() / 10);
        }
        let mut lignes = Vec::new();
        let mut lignes_fraction = Vec::new();
        for m in &mesures {
            let conteneur = mj
                .conteneur(&m.tex, m.bloc, m.em_px, m.ex_px, 800.0)
                .unwrap();
            let r = svg_pour_resvg(&conteneur, m.ex_px, couleur(&m.couleur).unwrap()).unwrap();
            lignes.push((m.nom.clone(), Some(mesurer("a", m, &r, true))));
            // Variante écartée : l'origine fractionnaire (sans l'arrondi de Chromium).
            let reference = reference::image(m).unwrap();
            let cand = peindre(&r, m, (m.x - m.clip.x, m.y - m.clip.y)).unwrap();
            let e = comparer(&reference, &cand, couleur(&m.fond).unwrap());
            lignes_fraction.push((m.nom.clone(), Some((e, 0.0, 0.0))));
        }
        imprimer("A (MathJax/QuickJS, origine arrondie au px)", &lignes);
        imprimer("A, origine fractionnaire (écartée)", &lignes_fraction);
    }
}

#[cfg(feature = "voie-b")]
mod voie_b {
    use super::*;
    use spike_maths::voie_b::MoteurTex;

    /// L'em de la math chez MathJax : `matchFontHeight` cale le x de la fonte TeX (0,442 em)
    /// sur le ex du texte (DM Sans), soit un em de math de ex / 0,442.
    pub fn em_mathjax(m: &Mesure) -> f64 {
        m.ex_px / 0.442
    }

    fn passe(voie: &str, moteur: &MoteurTex) {
        let mesures = reference::mesures().unwrap();
        let mut lignes = Vec::new();
        for m in &mesures {
            let r = moteur.rendre(&m.tex, m.bloc, em_mathjax(m), couleur(&m.couleur).unwrap());
            lignes.push((m.nom.clone(), r.ok().map(|r| mesurer(voie, m, &r, false))));
        }
        imprimer(voie, &lignes);
    }

    #[test]
    fn mesures_voie_b() {
        let t0 = Instant::now();
        let moteur = MoteurTex::stix().unwrap();
        let init = t0.elapsed();
        let t1 = Instant::now();
        moteur.rendre("x^2", false, 18.0, [0, 0, 0]).unwrap();
        let premier = t1.elapsed();
        let t2 = Instant::now();
        let mut n = 0u32;
        for _ in 0..10 {
            for f in FORMULES {
                if moteur.rendre(f.tex, f.bloc, 18.0, [0, 0, 0]).is_ok() {
                    n += 1;
                }
            }
        }
        let chaud = t2.elapsed() / n.max(1);
        println!(
            "\nvoie B : police {init:?} ; 1er rendu {premier:?} ; rendu chaud moyen {chaud:?}"
        );
        for f in FORMULES {
            if let Err(e) = moteur.rendre(f.tex, f.bloc, 18.0, [0, 0, 0]) {
                println!("voie B : {} non rendu : {e}", f.nom);
            }
        }
        passe("b-stix", &moteur);
        #[cfg(feature = "voie-c")]
        {
            let newcm = spike_maths::voie_c::police_math().unwrap();
            passe("b-newcm", &MoteurTex::avec_police(newcm).unwrap());
        }
    }
}

#[cfg(feature = "voie-c")]
mod voie_c {
    use super::*;
    use spike_maths::voie_c::Typst;

    #[test]
    fn mesures_voie_c() {
        let t0 = Instant::now();
        let typst = Typst::new();
        let init = t0.elapsed();
        let t1 = Instant::now();
        typst.rendre("x^2", false, 18.0, [0, 0, 0]).unwrap();
        let premier = t1.elapsed();
        let t2 = Instant::now();
        for _ in 0..3 {
            for f in FORMULES {
                typst.rendre(f.typst, f.bloc, 18.0, [0, 0, 0]).unwrap();
            }
        }
        let chaud = t2.elapsed() / (3 * FORMULES.len() as u32);
        println!(
            "\nvoie C : polices {init:?} ; 1er rendu {premier:?} ; rendu chaud moyen {chaud:?}"
        );
        let mut lignes = Vec::new();
        for m in &reference::mesures().unwrap() {
            let f = formule(m);
            let r = typst.rendre(
                f.typst,
                m.bloc,
                m.ex_px / 0.442,
                couleur(&m.couleur).unwrap(),
            );
            lignes.push((m.nom.clone(), r.ok().map(|r| mesurer("c", m, &r, false))));
        }
        imprimer("C (typst, New Computer Modern Math)", &lignes);
    }
}

/// Exploration (ignorée par défaut) : quelle origine et quelle épaisseur de trait rapprochent
/// resvg de Chromium ? `cargo test --release -- --ignored explorer --nocapture`.
#[cfg(feature = "voie-a")]
#[test]
#[ignore]
fn explorer_calage_voie_a() {
    use spike_maths::voie_a::{MathJax, svg_pour_resvg};
    let mj = MathJax::new().unwrap();
    let mesures = reference::mesures().unwrap();
    for nom in ["somme-dark", "puissance-dark", "matrice-light"] {
        let m = mesures.iter().find(|m| m.nom == nom).unwrap();
        let reference = reference::image(m).unwrap();
        let fond = couleur(&m.fond).unwrap();
        let conteneur = mj
            .conteneur(&m.tex, m.bloc, m.em_px, m.ex_px, 800.0)
            .unwrap();
        let base = svg_pour_resvg(&conteneur, m.ex_px, couleur(&m.couleur).unwrap()).unwrap();
        let (x0, y0) = placer(&base, m, true);
        let mut meilleur = (u64::MAX, String::new());
        for k in [1u32, 2, 4, 8, 16] {
            for trait_ in ["0", "5"] {
                let mut r = base.clone();
                r.svg = r
                    .svg
                    .replace("stroke-width=\"0\"", &format!("stroke-width=\"{trait_}\""));
                let c = peindre_sur_echantillonne(&r, m, (x0, y0), k).unwrap();
                let s: u64 = reference
                    .pixels()
                    .zip(c.pixels())
                    .map(|(a, b)| {
                        (0..3)
                            .map(|i| u64::from(a.0[i].abs_diff(b.0[i])))
                            .sum::<u64>()
                    })
                    .sum();
                let e = comparer(&reference, &c, fond);
                println!(
                    "  {nom} k {k} trait {trait_} : somme {s} ≠ {:.2} % >16 {} max {} encre {:+.1} %",
                    e.pourcent(),
                    e.au_dela_16,
                    e.delta_max,
                    100.0 * e.encre_relative()
                );
                if s < meilleur.0 {
                    meilleur = (s, format!("k {k} trait {trait_}"));
                }
            }
        }
        println!("{nom} (x0 {:.4} y0 {:.4}) → {}", x0, y0, meilleur.1);
    }
}
