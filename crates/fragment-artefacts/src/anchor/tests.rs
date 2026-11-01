// Port de app/src/ui/core/artefacts/anchor.test.ts

use super::*;
use crate::types::{DurableAnchor, GutterSide, WidgetAnchor};

/// Une surface éditable minimale (`getLine` + `transaction` côté TS) : le texte, identité.
struct Editable(PlainText);
fn editable(text: &str) -> Editable {
    Editable(PlainText::new(text))
}
impl TextSource for Editable {
    fn full_text(&self) -> FullText<'_> {
        FullText::Ready(&self.0)
    }
}

/// Une surface textuelle NON éditable (PDF) : `getLine` mais pas `transaction` → jamais de texte.
struct PdfLike;
fn pdf_like() -> PdfLike {
    PdfLike
}
impl TextSource for PdfLike {
    fn full_text(&self) -> FullText<'_> {
        FullText::Never
    }
}

/// Un texte de prose, assez long pour que la citation ait du contexte.
fn texte() -> String {
    [
        "Le store garde le point d’ancrage du widget, même après une édition faite ailleurs.",
        "Une seconde phrase pour donner du contexte avant et après la citation relevée.",
        "Et une troisième, parce que la recherche approchée a besoin de matière autour.",
    ]
    .join("\n")
}

fn chars(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `String.prototype.indexOf`, en unités UTF-16.
fn index_of(text: &str, needle: &str) -> i64 {
    let (t, n) = (chars(text), chars(needle));
    (0..=t.len() - n.len())
        .find(|&i| t[i..i + n.len()] == n[..])
        .map(|i| i as i64)
        .unwrap()
}

fn last_index_of(text: &str, needle: &str) -> i64 {
    let (t, n) = (chars(text), chars(needle));
    (0..=t.len() - n.len())
        .rev()
        .find(|&i| t[i..i + n.len()] == n[..])
        .map(|i| i as i64)
        .unwrap()
}

/// `text.slice(a, b)`, en unités UTF-16.
fn sl(text: &str, a: i64, b: i64) -> String {
    let t = chars(text);
    let a = a.clamp(0, t.len() as i64) as usize;
    let b = b.clamp(a as i64, t.len() as i64) as usize;
    String::from_utf16_lossy(&t[a..b])
}

fn len(text: &str) -> i64 {
    fragment_text::utf16::utf16_len(text) as i64
}

fn pos() -> i64 {
    index_of(&texte(), "ancrage")
}

fn doc(pos: i64, dx: f64, dy: f64) -> WidgetAnchor {
    WidgetAnchor::Document {
        pos,
        to: None,
        dx,
        dy,
    }
}

const Q: usize = QUOTE_LEN;
const QL: i64 = QUOTE_LEN as i64;

mod todurable_la_citation_15_7 {
    use super::*;

    #[test]
    fn viewport_copie_fidele_sans_citation() {
        let a = WidgetAnchor::Viewport {
            x: 10.0,
            y: 20.0,
            from: None,
        };
        assert_eq!(
            to_durable(&a, Some(&editable("hello"))),
            DurableAnchor::from(a.clone())
        );
    }

    #[test]
    fn document_32_caracteres_avant_32_a_partir_de_pos_32_encore() {
        let t = texte();
        let p = pos();
        let d = to_durable(&doc(p, 0.0, 0.0), Some(&editable(&t)));
        assert_eq!(
            d.quote,
            Some(AnchorQuote {
                prefix: sl(&t, (p - QL).max(0), p),
                exact: sl(&t, p, p + QL),
                suffix: sl(&t, p + QL, p + 2 * QL),
            })
        );
    }

    #[test]
    fn sans_editeur_ou_surface_non_editable_pdf_pas_de_citation() {
        let a = doc(1, 0.0, 0.0);
        assert_eq!(to_durable(&a, None).quote, None);
        assert_eq!(to_durable(&a, Some(&pdf_like())).quote, None);
    }

    #[test]
    fn l_ancien_indice_char_ne_passe_pas() {
        let a: WidgetAnchor =
            serde_json::from_str(r#"{"mode":"document","pos":1,"dx":0,"dy":0,"char":"e"}"#)
                .unwrap();
        assert_eq!(to_durable(&a, None), DurableAnchor::from(doc(1, 0.0, 0.0)));
        assert_eq!(
            serde_json::to_string(&to_durable(&a, None)).unwrap(),
            r#"{"mode":"document","pos":1,"dx":0,"dy":0}"#
        );
    }
}

mod clamptodoc {
    use super::*;

    #[test]
    fn borne_un_offset_au_dela_de_la_longueur_et_un_negatif_a_0() {
        assert_eq!(
            clamp_to_doc(&doc(99, 0.0, 0.0).into(), Some(&editable("hello"))).pos(),
            Some(5)
        );
        assert_eq!(
            clamp_to_doc(&doc(-3, 0.0, 0.0).into(), Some(&editable("hello"))).pos(),
            Some(0)
        );
    }

    #[test]
    fn retire_la_citation_l_ancre_vivante_de_l_api_layers() {
        let a = DurableAnchor {
            anchor: doc(2, 1.0, 1.0),
            quote: Some(quote_at(&chars("hello"), 2)),
        };
        assert_eq!(clamp_to_doc(&a, Some(&editable("hello"))), doc(2, 1.0, 1.0));
    }

    #[test]
    fn surface_sans_longueur_pdf_inchange_jamais_de_rangeerror() {
        assert_eq!(
            clamp_to_doc(&doc(999, 0.0, 0.0).into(), Some(&pdf_like())).pos(),
            Some(999)
        );
    }
}

mod resolveanchor_la_cascade_figure_35 {
    use super::*;

    fn ancre() -> DurableAnchor {
        DurableAnchor {
            anchor: doc(pos(), 3.0, 4.0),
            quote: Some(quote_at(&chars(&texte()), pos())),
        }
    }

    #[test]
    fn le_texte_a_pos_est_celui_de_la_citation_gardee() {
        assert_eq!(
            resolve_anchor_in(&ancre(), &texte()),
            Resolution {
                anchor: doc(pos(), 3.0, 4.0),
                status: ResolutionStatus::Kept
            }
        );
    }

    #[test]
    fn du_texte_insere_avant_retrouvee_a_l_identique_relocalisee() {
        let avant = "Un paragraphe ajouté en tête du fichier, hors de l’application.\n\n";
        let r = resolve_anchor_in(&ancre(), &format!("{avant}{}", texte()));
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(r.anchor, doc(pos() + len(avant), 3.0, 4.0));
    }

    #[test]
    fn citation_retouchee_de_moins_d_un_quart_retrouvee() {
        // 3 caractères changés sur 32 dans le texte exact, et un décalage.
        let t = texte();
        let p = pos();
        let exact = sl(&t, p, p + QL);
        let retouche = format!("{}XYZ{}", sl(&exact, 0, 5), sl(&exact, 8, len(&exact)));
        let texte = format!(
            "Intro.\n{}{retouche}{}",
            sl(&t, 0, p),
            sl(&t, p + QL, len(&t))
        );
        let r = resolve_anchor_in(&ancre(), &texte);
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(r.anchor.pos(), Some(p + len("Intro.\n")));
    }

    #[test]
    fn au_dela_d_un_quart_d_erreurs_orpheline_rangee_dans_la_marge() {
        let t = texte();
        let p = pos();
        let exact = sl(&t, p, p + QL);
        // 12 caractères remplacés sur 32 : au-delà du quart (8).
        let massacre = format!("{}{}", "#".repeat(12), sl(&exact, 12, len(&exact)));
        let texte = format!("{}{massacre}{}", sl(&t, 0, p), sl(&t, p + QL, len(&t))).replacen(
            "le point d’",
            "zzzzzzzzzzz",
            1,
        );
        let r = resolve_anchor_in(&ancre(), &texte);
        assert_eq!(r.status, ResolutionStatus::Orphan);
        assert!(matches!(
            r.anchor,
            WidgetAnchor::Gutter {
                side: GutterSide::Right,
                ..
            }
        ));
    }

    #[test]
    fn plusieurs_candidats_le_contexte_departage() {
        let bloc = "motif répété assez long pour servir de citation exacte!!";
        let texte = format!("Avant A. {bloc} après A.\nAvant B. {bloc} après B.");
        let p = last_index_of(&texte, bloc);
        let a = DurableAnchor {
            anchor: doc(p, 0.0, 0.0),
            quote: Some(quote_at(&chars(&texte), p)),
        };
        // On décale tout : la position ne tombe plus juste, la citation existe deux fois.
        let r = resolve_anchor_in(&a, &format!("XX{texte}"));
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(r.anchor.pos(), Some(p + 2));
    }

    #[test]
    fn surface_non_textuelle_pdf_offsets_fixes_gardee() {
        let a: DurableAnchor = doc(1_000_005, 0.0, 0.0).into();
        assert_eq!(
            resolve_anchor(&a, None),
            Resolution {
                anchor: doc(1_000_005, 0.0, 0.0),
                status: ResolutionStatus::Kept
            }
        );
    }

    #[test]
    fn sans_citation_ancre_importee_gardee_bornee() {
        let a: DurableAnchor = doc(999, 0.0, 0.0).into();
        let r = resolve_anchor_in(&a, "court");
        assert_eq!(
            (r.anchor.pos(), r.status),
            (Some(5), ResolutionStatus::Kept)
        );
    }
}

// Une surface à PAS FIXE (une conversation : une plage de positions par unité)
// — `getText` rend le texte joint et sa carte index ↔ position.
const STRIDE: i64 = 1_000;

struct Strided {
    text: Vec<u16>,
    starts: Vec<i64>,
    lens: Vec<i64>,
}

fn strided(units: &[&str]) -> Strided {
    let sep = "\n\n";
    let mut text = String::new();
    let mut starts = Vec::new();
    let mut lens = Vec::new();
    for (k, u) in units.iter().enumerate() {
        if k > 0 {
            text.push_str(sep);
        }
        starts.push(len(&text));
        lens.push(len(u));
        text.push_str(u);
    }
    Strided {
        text: chars(&text),
        starts,
        lens,
    }
}

impl SurfaceText for Strided {
    fn units(&self) -> &[u16] {
        &self.text
    }
    fn to_index(&self, pos: i64) -> i64 {
        let k = (pos.div_euclid(STRIDE)).clamp(0, self.starts.len() as i64 - 1) as usize;
        self.starts[k] + (pos - k as i64 * STRIDE).min(self.lens[k])
    }
    fn to_pos(&self, i: i64) -> i64 {
        let mut k = self.starts.len() - 1;
        while k > 0 && self.starts[k] > i {
            k -= 1;
        }
        k as i64 * STRIDE + (i - self.starts[k]).min(self.lens[k])
    }
}

struct StridedSurface(Option<Strided>);
fn strided_surface(t: Option<Strided>) -> StridedSurface {
    StridedSurface(t)
}
impl TextSource for StridedSurface {
    fn full_text(&self) -> FullText<'_> {
        match &self.0 {
            Some(t) => FullText::Ready(t),
            None => FullText::Pending,
        }
    }
}

mod surface_a_pas_fixe_gettext {
    use super::*;

    fn units() -> Vec<String> {
        vec![
            "Bonjour le monde.".into(),
            texte(),
            "Fin de la conversation.".into(),
        ]
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(|s| s.as_str()).collect()
    }
    fn p() -> i64 {
        STRIDE + pos() // « ancrage » dans l'unité 1
    }
    fn durable() -> DurableAnchor {
        let u = units();
        to_durable(
            &doc(p(), 0.0, 0.0),
            Some(&strided_surface(Some(strided(&refs(&u))))),
        )
    }

    #[test]
    fn la_citation_est_relevee_dans_le_texte_joint_a_la_bonne_place() {
        assert!(durable().quote.unwrap().exact.starts_with("ancrage"));
    }

    #[test]
    fn gardee_quand_rien_n_a_bouge() {
        let u = units();
        let r = resolve_anchor(&durable(), Some(&strided(&refs(&u))));
        assert_eq!(
            (r.anchor.pos(), r.status),
            (Some(p()), ResolutionStatus::Kept)
        );
    }

    #[test]
    fn relocalisee_quand_le_passage_change_d_unite_rechargement_regroupe() {
        let mut apres = vec!["Un message ajouté avant.".to_string()];
        apres.extend(units());
        let r = resolve_anchor(&durable(), Some(&strided(&refs(&apres))));
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(r.anchor.pos(), Some(2 * STRIDE + pos()));
    }

    #[test]
    fn orpheline_quand_le_passage_a_disparu() {
        let r = resolve_anchor(
            &durable(),
            Some(&strided(&[
                "Tout autre chose ici, sans aucun rapport.",
                "Et encore.",
            ])),
        );
        assert_eq!(r.status, ResolutionStatus::Orphan);
    }

    #[test]
    fn un_index_dans_le_separateur_rend_la_fin_de_l_unite_precedente() {
        let t = strided(&["ab", "cd"]);
        assert_eq!(t.to_pos(3), 2); // 'ab' + '\n' → fin de l'unité 0
        assert_eq!(t.to_pos(t.to_index(STRIDE + 1)), STRIDE + 1);
    }

    #[test]
    fn pas_encore_lisible_en_attente_rien_de_releve() {
        let s = strided_surface(None);
        assert!(surface_pending(Some(&s)));
        assert_eq!(to_durable(&doc(5, 0.0, 0.0), Some(&s)).quote, None);
        assert!(!surface_pending(Some(&pdf_like())));
    }

    #[test]
    fn clamptodoc_borne_dans_l_unite() {
        let s = strided_surface(Some(strided(&["abc"])));
        assert_eq!(
            clamp_to_doc(&doc(50, 0.0, 0.0).into(), Some(&s)).pos(),
            Some(3)
        );
    }
}

mod ancre_plage_pos_to_la_plage_tenue_par_le_c_ur {
    use super::*;

    fn from() -> i64 {
        index_of(&texte(), "point d’ancrage")
    }
    fn to() -> i64 {
        from() + len("point d’ancrage")
    }
    fn gutter(pos: i64, to: i64) -> WidgetAnchor {
        WidgetAnchor::Gutter {
            side: GutterSide::Right,
            pos,
            to: Some(to),
            dy: 0.0,
        }
    }
    fn plage() -> WidgetAnchor {
        gutter(from(), to())
    }

    #[test]
    fn la_citation_d_une_plage_est_son_texte_entre_son_contexte() {
        let t = texte();
        let q = to_durable(&plage(), Some(&editable(&t))).quote.unwrap();
        assert_eq!(q.exact, "point d’ancrage");
        assert_eq!(q.prefix, sl(&t, (from() - QL).max(0), from()));
        assert_eq!(q.suffix, sl(&t, to(), to() + QL));
    }

    #[test]
    fn texte_modifie_ailleurs_la_plage_est_relocalisee_entiere_meme_longueur() {
        let t = texte();
        let d = to_durable(&plage(), Some(&editable(&t)));
        let avant = "Un paragraphe ajouté en tête, hors de l’app.\n";
        let r = resolve_anchor_in(&d, &format!("{avant}{t}"));
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(
            (r.anchor.pos(), r.anchor.to()),
            (Some(from() + len(avant)), Some(to() + len(avant)))
        );
    }

    #[test]
    fn surface_sans_texte_pdf_image_la_plage_est_gardee_telle_quelle() {
        assert_eq!(to_durable(&plage(), Some(&pdf_like())).quote, None);
        assert_eq!(
            resolve_anchor(&plage().into(), None),
            Resolution {
                anchor: plage(),
                status: ResolutionStatus::Kept
            }
        );
    }

    #[test]
    fn pas_fixe_unites_regroupees_a_la_relecture_la_plage_garde_la_longueur_de_sa_citation_pas_l_ecart_des_anciennes_positions()
     {
        // En direct : l'appel d'outil (unité 1) puis la réponse (unité 2) ; relue : la réponse seule.
        let direct = [
            "Question ?",
            "outil : lecture",
            "Voici la réponse complète.",
        ];
        let de = 2 * STRIDE;
        let fil = gutter(de, de + len("Voici"));
        let d = to_durable(&fil, Some(&strided_surface(Some(strided(&direct)))));
        assert_eq!(d.quote.as_ref().map(|q| q.exact.as_str()), Some("Voici"));
        let relue = strided(&["Question ?", "Voici la réponse complète."]);
        let r = resolve_anchor(&d, Some(&relue));
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(
            (r.anchor.pos(), r.anchor.to()),
            (Some(STRIDE), Some(STRIDE + len("Voici")))
        );
    }

    #[test]
    fn plage_vide_to_pos_le_fil_d_un_cercle_reste_vide_une_fois_relocalisee() {
        let t = texte();
        let vide = gutter(from(), from());
        let d = to_durable(&vide, Some(&editable(&t)));
        let avant = "Un paragraphe ajouté en tête.\n";
        let r1 = resolve_anchor_in(&d, &t).anchor;
        assert_eq!((r1.pos(), r1.to()), (Some(from()), Some(from())));
        let r2 = resolve_anchor_in(&d, &format!("{avant}{t}")).anchor;
        assert_eq!(
            (r2.pos(), r2.to()),
            (Some(from() + len(avant)), Some(from() + len(avant)))
        );
    }

    #[test]
    fn clamptodoc_borne_aussi_la_fin() {
        let a = WidgetAnchor::Document {
            pos: 2,
            to: Some(9_999),
            dx: 0.0,
            dy: 0.0,
        };
        let r = clamp_to_doc(&a.into(), Some(&editable("abcdef")));
        assert_eq!((r.pos(), r.to()), (Some(2), Some(6)));
    }
}

// ADR-004 : un émoji (hors plan multilingue de base) vaut 2 unités, comme
// `String.length` côté TS. Nombres donnés par le TS : "😀".length === 2.
mod emoji_avant_l_ancre_unites_utf16 {
    use super::*;

    #[test]
    fn la_citation_se_decoupe_en_unites_utf16() {
        // "😀😀ab" : 6 unités ; pos 4 = juste avant "ab".
        let q = quote_at(&chars("😀😀ab"), 4);
        assert_eq!(
            q,
            AnchorQuote {
                prefix: "😀😀".into(),
                exact: "ab".into(),
                suffix: String::new()
            }
        );
        // La plage [4, 5) de "😀😀ab" est "a".
        assert_eq!(quote_of(&chars("😀😀ab"), 4, 5).exact, "a");
    }

    #[test]
    fn une_ancre_derriere_un_emoji_est_relocalisee_puis_gardee_avec_les_nombres_du_ts() {
        let ancre = DurableAnchor {
            anchor: doc(pos(), 3.0, 4.0),
            quote: Some(quote_at(&chars(&texte()), pos())),
        };
        // « 😀 » + espace = 3 unités UTF-16 ("😀 ".length === 3).
        let avant = "😀 ";
        assert_eq!(len(avant), 3);
        let texte_emoji = format!("{avant}{}", texte());
        let r = resolve_anchor_in(&ancre, &texte_emoji);
        assert_eq!(r.status, ResolutionStatus::Relocated);
        assert_eq!(r.anchor, doc(pos() + 3, 3.0, 4.0));
        // Et l'ancre posée APRÈS l'émoji est vérifiée à sa position UTF-16.
        let p = index_of(&texte_emoji, "ancrage");
        assert_eq!(p, pos() + 3);
        let a = DurableAnchor {
            anchor: doc(p, 0.0, 0.0),
            quote: Some(quote_at(&chars(&texte_emoji), p)),
        };
        let r = resolve_anchor_in(&a, &texte_emoji);
        assert_eq!(
            (r.anchor.pos(), r.status),
            (Some(p), ResolutionStatus::Kept)
        );
    }
}

#[test]
fn quote_len_vaut_32_et_la_fenetre_se_borne_en_fin_de_texte() {
    assert_eq!(Q, 32);
    let q = quote_at(&chars("abc"), 10);
    assert_eq!(
        q,
        AnchorQuote {
            prefix: "abc".into(),
            exact: String::new(),
            suffix: String::new()
        }
    );
}
