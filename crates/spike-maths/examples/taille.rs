//! Mesure du poids ajouté au binaire : un exécutable qui rend UNE formule par chaque voie
//! activée (le code est donc lié), à comparer à la version sans voie.
//! `cargo build --release -p spike-maths --example taille --no-default-features [--features voie-x]`

fn main() {
    // Sans voie : seulement la rastérisation (resvg), commune à toutes.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="5" height="5"/></svg>"#;
    let r = spike_maths::raster::RenduSvg {
        svg: svg.into(),
        largeur: 10.0,
        hauteur: 10.0,
        ligne_de_base: 10.0,
    };
    println!("svg {} octets", r.svg.len());
    #[cfg(feature = "voie-a")]
    {
        let ok = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(|| {
                spike_maths::voie_a::MathJax::new()
                    .and_then(|m| m.rendre("x^2", false, 8.064, 16.0, [0, 0, 0]))
                    .is_ok()
            })
            .map(|h| h.join().unwrap_or(false))
            .unwrap_or(false);
        println!("voie A : {ok}");
    }
    #[cfg(feature = "voie-b")]
    println!(
        "voie B : {}",
        spike_maths::voie_b::MoteurTex::stix()
            .and_then(|m| m.rendre("x^2", false, 18.0, [0, 0, 0]))
            .is_ok()
    );
    #[cfg(feature = "voie-c")]
    println!(
        "voie C : {}",
        spike_maths::voie_c::Typst::new()
            .rendre("x^2", false, 18.0, [0, 0, 0])
            .is_ok()
    );
}
