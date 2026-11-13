//! Tests headless de la trace d'un tour (tâche `fr-t7-trace`), nommés d'après les cas de
//! `app/e2e/agent-trace.spec.ts` : direct, relue à l'identique, tour simple, geste, orbe des
//! réglages. Le SDK doublé du test TS devient un `TraceBuilder` rythmé à la main : chaque pas
//! pose une heure, la poignée lit l'instantané (`vue`) à cette heure, comme la vue GPUI.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::collections::BTreeSet;

use fragment_agent::contrat::{BlocOutil, ChatBlock, ResultatOutil};
use fragment_agent::trace_model::{
    OrbState, OutilEntrant, StepKind, TraceBuilder, TraceStatus, split_turn,
};
use fragment_ui::agent_trace::{EtatEtape, EtatsTrace, Marque, OrbeChoix, Poignee, VueTrace};
use serde_json::json;

const T0: i64 = 1_000_000_000;

fn entree(v: serde_json::Value) -> Option<fragment_agent::contrat::Entree> {
    v.as_object().cloned()
}

fn outil(id: &str, nom: &str, v: serde_json::Value) -> OutilEntrant {
    OutilEntrant {
        id: id.into(),
        name: nom.into(),
        input: entree(v),
    }
}

fn resultat(texte: &str) -> ResultatOutil {
    ResultatOutil {
        is_error: false,
        text: texte.into(),
    }
}

fn titre(v: &VueTrace) -> String {
    v.titre
        .iter()
        .find(|c| !c.sortante)
        .map(|c| c.texte.clone())
        .unwrap_or_default()
}

fn poignee(b: &TraceBuilder, orbe: OrbeChoix, cle: &str, maintenant: i64) -> Poignee {
    let mut etats = EtatsTrace::new();
    Poignee::new(etats.pour(cle), b.snapshot(), orbe, false, maintenant)
}

/// Le tour du test e2e : pensée, recherche, narration, lecture, réponse, fin. Rend les
/// instantanés pris à chaque pas, pour que chaque test relise ce qu'il veut.
struct Tour {
    b: TraceBuilder,
    p: Poignee,
}

impl Tour {
    fn demarre(orbe: OrbeChoix, cle: &str) -> Self {
        let b = TraceBuilder::new(T0);
        let p = poignee(&b, orbe, cle, T0);
        Self { b, p }
    }

    fn pousse(&mut self, maintenant: i64) {
        self.p.update(self.b.snapshot(), maintenant);
    }
}

// ── en direct ────────────────────────────────────────────────────────────────

#[test]
fn en_direct_des_l_envoi_la_trace_vide_et_son_orbe_pas_de_chevron() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m1");
    let v = t.p.vue(T0);
    assert_eq!(v.status, TraceStatus::Running);
    assert!(v.vide);
    assert_eq!(v.orbe, Some(OrbState::Solving));
    assert_eq!(titre(&v), "Au travail…");
}

#[test]
fn en_direct_la_pensee_repliee_l_en_tete_dit_rereflechit_l_orbe_solving_et_le_chrono_avance() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m2");
    t.b.thinking("", T0 + 1000, true);
    for mot in "Je dois trouver où l’article parle de Palantir puis le relire.".split(' ') {
        t.b.thinking(&format!("{mot} "), T0 + 1000, false);
    }
    t.pousse(T0 + 1000);
    let v = t.p.vue(T0 + 1000);
    assert!(!v.ouvert);
    assert_eq!(titre(&v), "Réfléchit…");
    assert!(v.titre.iter().any(|c| c.reflet));
    assert_eq!(v.orbe, Some(OrbState::Solving));
    let pensee = &v.etapes[0];
    assert_eq!(pensee.kind, StepKind::Thinking);
    assert_eq!(pensee.etat, EtatEtape::Running);
    assert_eq!(pensee.marque, Marque::Arc);
    assert!(pensee.contenu.contains("Palantir puis le relire"));
    assert!(pensee.ouvert);
    // Le chrono avance (des chiffres qui roulent) : « 1s » à T0+1000, « 4s » à T0+4000.
    t.p.tic(T0 + 4000);
    let chrono = t.p.vue(T0 + 4000).chrono;
    assert!(
        chrono.len() == 2
            && chrono.ends_with('s')
            && chrono.starts_with(|c: char| ('1'..='9').contains(&c)),
        "{chrono}"
    );
}

#[test]
fn en_direct_une_pensee_seule_depliee_n_a_pas_de_liste() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m3");
    t.b.thinking("", T0 + 1000, true);
    t.b.thinking("Je réfléchis. ", T0 + 1000, false);
    t.pousse(T0 + 1000);
    t.p.clic_en_tete(T0 + 1100);
    let v = t.p.vue(T0 + 1100);
    assert!(v.ouvert);
    assert!(v.seule);
    // La ligne est masquée par la vue (`seule`) ; la pensée, elle, est ouverte.
    assert!(v.etapes[0].ouvert);
    // L'en-tête dit l'étape (sans ligne, rien d'autre ne la répète).
    assert_eq!(titre(&v), "Réfléchit…");
    t.p.clic_en_tete(T0 + 1200);
    let v = t.p.vue(T0 + 1200);
    assert!(!v.ouvert);
    assert_eq!(titre(&v), "Réfléchit…");
}

#[test]
fn en_direct_clic_sur_une_trace_vide_ne_deplie_rien() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m4");
    t.p.clic_en_tete(T0 + 10);
    assert!(!t.p.vue(T0 + 10).ouvert);
}

#[test]
fn en_direct_pensee_outils_narration_repliee_reponse_seule_resume() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m5");
    // 1. la pensée
    t.b.thinking("", T0 + 1000, true);
    t.b.thinking(
        "Je dois trouver où l’article parle de Palantir. ",
        T0 + 1000,
        false,
    );
    t.pousse(T0 + 1000);

    // 2. un outil : la pensée se pose, la recherche tourne
    t.b.tool(
        outil("g1", "Grep", json!({ "pattern": "Palantir" })),
        T0 + 2000,
    );
    t.pousse(T0 + 2000);
    let v = t.p.vue(T0 + 2000);
    let pensee = &v.etapes[0];
    assert_eq!(pensee.etat, EtatEtape::Done);
    assert_eq!(pensee.marque, Marque::Point);
    assert!(!pensee.ouvert);
    assert_eq!(v.etapes[1].etat, EtatEtape::Running);
    assert_eq!(titre(&v), "Cherche « Palantir »…");
    assert_eq!(v.orbe, Some(OrbState::Searching));
    assert_eq!(v.etapes[1].cible.as_ref().unwrap().texte, "« Palantir »");

    // Résultat : la dernière étape finie montre son résultat.
    t.b.result(
        "g1",
        resultat("article.md:4: The first was Palantir."),
        T0 + 2200,
    );
    t.pousse(T0 + 2200);
    let v = t.p.vue(T0 + 2200);
    let grep = &v.etapes[1];
    assert_eq!(grep.etat, EtatEtape::Done);
    let verbe: String = grep
        .verbe
        .iter()
        .find(|c| !c.sortante)
        .unwrap()
        .texte
        .clone();
    assert_eq!(verbe, "A cherché");
    assert!(grep.ouvert);
    assert!(grep.contenu.contains("The first was Palantir."));
    // Plus d'une étape : la liste.
    assert!(!v.seule);

    // 3. Narration : elle s'écrit d'abord comme une réponse (l'orbe « écrit »)…
    t.b.ecrire(T0 + 2400);
    t.pousse(T0 + 2400);
    let v = t.p.vue(T0 + 2400);
    assert_eq!(titre(&v), "Écrit la réponse…");
    assert_eq!(v.orbe, Some(OrbState::Composing));
    // …puis un outil la suit : elle passe dans la trace (une note).
    t.b.note("Je lis la note pour vérifier.", T0 + 2500);
    t.b.tool(
        outil("r1", "Read", json!({ "file_path": "C:/coffre/article.md" })),
        T0 + 2600,
    );
    t.pousse(T0 + 2600);
    let v = t.p.vue(T0 + 2600);
    let note = v.etapes.iter().find(|e| e.kind == StepKind::Note).unwrap();
    assert_eq!(note.verbe[0].texte, "Je lis la note pour vérifier.");
    let lire = v
        .etapes
        .iter()
        .filter(|e| e.kind == StepKind::Tool)
        .nth(1)
        .unwrap();
    assert_eq!(lire.etat, EtatEtape::Running);
    assert_eq!(lire.cible.as_ref().unwrap().texte, "article.md");
    // Le résultat de la recherche se replie quand l'étape suivante commence.
    assert!(!v.etapes[1].ouvert);
    t.b.result(
        "r1",
        resultat("FDEs have the hottest job in AI."),
        T0 + 2900,
    );
    t.pousse(T0 + 2900);
    assert_eq!(t.p.vue(T0 + 2900).etapes[3].etat, EtatEtape::Done);

    // 4. La réponse s'écrit SOUS la trace : la trace s'arrête (son chrono se fige).
    t.b.ecrire(T0 + 3000);
    t.pousse(T0 + 3000);

    // 5. Fin : tout se replie sur le résumé (sans reflet), chrono figé.
    t.b.finish(T0 + 3500, false);
    t.pousse(T0 + 3500);
    let v = t.p.vue(T0 + 3500);
    assert_eq!(v.status, TraceStatus::Done);
    assert!(!v.ouvert);
    let titre_fin = titre(&v);
    assert!(
        titre_fin.starts_with("A réfléchi") && titre_fin.ends_with(" · lu 1 fichier · 1 recherche"),
        "{titre_fin}"
    );
    assert!(v.titre.iter().filter(|c| !c.sortante).all(|c| !c.reflet));
    assert_eq!(v.orbe, None);
    let t1 = v.chrono.clone();
    t.p.tic(T0 + 4800);
    assert_eq!(t.p.vue(T0 + 4800).chrono, t1);

    // Un clic rouvre — en mode NERVEUX (110 ms) ; les étapes restent repliées.
    t.p.clic_en_tete(T0 + 5000);
    let v = t.p.vue(T0 + 5000);
    assert!(v.geste);
    assert_eq!(v.transition_corps, (110, 0));
    assert!(v.ouvert);
    assert_eq!(v.etapes.len(), 4);
    let lire_id = v.etapes[3].id.clone();
    assert!(!v.etapes[3].ouvert);
    t.p.clic_etape(&lire_id, T0 + 5100);
    assert!(t.p.vue(T0 + 5100).etapes[3].ouvert);
    t.p.clic_en_tete(T0 + 5200);
    assert!(!t.p.vue(T0 + 5200).ouvert);
}

// ── le geste ─────────────────────────────────────────────────────────────────

#[test]
fn le_geste_une_etape_neuve_entre_depuis_le_flou_un_texte_qui_change_garde_au_plus_deux_couches() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m6");
    let mut entrees = BTreeSet::new();
    let mut couches = 0;
    let mut observer = |t: &mut Tour, now: i64| {
        let v = t.p.vue(now);
        for e in &v.etapes {
            if e.est_entree {
                entrees.insert(e.id.clone());
            }
        }
        couches = couches.max(v.titre.len());
    };
    let mut now = T0;
    t.b.thinking("", now + 250, true);
    t.b.thinking("Je réfléchis. ", now + 250, false);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.tool(outil("g1", "Grep", json!({ "pattern": "x" })), now + 250);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.result("g1", resultat("ok"), now + 250);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.ecrire(now + 250);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.note("Je lis.", now + 250);
    t.b.tool(
        outil("r1", "Read", json!({ "file_path": "a.md" })),
        now + 250,
    );
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.result("r1", resultat("fin"), now + 250);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    t.b.finish(now + 250, false);
    now += 250;
    t.pousse(now);
    observer(&mut t, now);
    // Chaque étape neuve est entrée depuis le flou (4 : pensée, recherche, narration, lecture).
    assert_eq!(
        entrees.into_iter().collect::<Vec<_>>(),
        vec!["g1", "r1", "s0", "s1"]
    );
    assert!(couches <= 2);
    // Aucune classe de transition ne reste posée : plus rien ne s'anime une seconde après.
    assert!(!t.p.anime(now + 1000));
    assert!(
        t.p.vue(now + 1000)
            .etapes
            .iter()
            .all(|e| (e.entree - 1.0).abs() < 1e-6)
    );
}

// ── relue à l'identique ──────────────────────────────────────────────────────

fn blocs_du_tour() -> Vec<ChatBlock> {
    let outil_bloc = |id: &str,
                      nom: &str,
                      resume: &str,
                      input: serde_json::Value,
                      res: &str,
                      at: i64,
                      fin: i64| {
        ChatBlock::Tool(BlocOutil {
            id: id.into(),
            name: nom.into(),
            summary: resume.into(),
            input: entree(input),
            result: Some(resultat(res)),
            at: Some(at),
            result_at: Some(fin),
        })
    };
    vec![
        ChatBlock::Thinking {
            text: "Je dois trouver où l’article parle de Palantir.".into(),
            at: Some(T0 + 1500),
        },
        outil_bloc(
            "g1",
            "Grep",
            "Palantir",
            json!({ "pattern": "Palantir" }),
            "article.md:4",
            T0 + 2000,
            T0 + 2200,
        ),
        ChatBlock::Text {
            text: "Je lis la note pour vérifier.".into(),
            at: Some(T0 + 2500),
        },
        outil_bloc(
            "r1",
            "Read",
            "article.md",
            json!({ "file_path": "C:/coffre/article.md" }),
            "FDEs…",
            T0 + 2600,
            T0 + 2900,
        ),
        ChatBlock::Text {
            text: "Palantir est la **première** expérience citée.".into(),
            at: Some(T0 + 3500),
        },
    ]
}

#[test]
fn relue_rouvrir_la_conversation_meme_trace_repliee_resume_durees_puis_la_reponse() {
    let tour = split_turn(&blocs_du_tour(), Some(T0));
    let trace = tour.trace.expect("une trace");
    assert_eq!(
        tour.answer,
        vec!["Palantir est la **première** expérience citée."]
    );
    let mut etats = EtatsTrace::new();
    let mut p = Poignee::new(etats.pour("m7"), trace, OrbeChoix::Auto, false, T0 + 10_000);
    let v = p.vue(T0 + 10_000);
    assert_eq!(v.status, TraceStatus::Done);
    assert!(!v.ouvert);
    assert_eq!(titre(&v), "A réfléchi 2s · lu 1 fichier · 1 recherche");
    // La durée de la TRACE : jusqu'à sa dernière étape (la réponse qui suit ne compte pas).
    assert_eq!(v.chrono, "3s");
    // « 3s », l'unité collée : une case par caractère.
    let cases: Vec<String> = v
        .chrono_cases
        .iter()
        .map(|c| c.courant.to_string())
        .collect();
    assert_eq!(cases, ["3", "s"]);
    p.clic_en_tete(T0 + 10_100);
    let v = p.vue(T0 + 10_100);
    assert_eq!(v.etapes.len(), 4);
    let note = v.etapes.iter().find(|e| e.kind == StepKind::Note).unwrap();
    assert_eq!(note.verbe[0].texte, "Je lis la note pour vérifier.");
    // Une relecture n'anime rien : aucune étape n'entre depuis le flou.
    assert!(v.etapes.iter().all(|e| !e.est_entree));
}

#[test]
fn relue_l_etat_choisi_survit_a_la_reconstruction_du_message() {
    let tour = split_turn(&blocs_du_tour(), Some(T0));
    let trace = tour.trace.expect("une trace");
    let mut etats = EtatsTrace::new();
    let mut p = Poignee::new(etats.pour("m8"), trace.clone(), OrbeChoix::Auto, false, T0);
    p.clic_en_tete(T0 + 1);
    // Le message est reconstruit : une nouvelle poignée sur le même état.
    let mut p2 = Poignee::new(etats.pour("m8"), trace, OrbeChoix::Auto, false, T0 + 2);
    assert!(p2.vue(T0 + 2).ouvert);
}

// ── l'orbe des réglages ──────────────────────────────────────────────────────

#[test]
fn l_orbe_choisie_dans_les_reglages_remplace_l_automatique() {
    let mut t = Tour::demarre(OrbeChoix::Etat(OrbState::Weaving), "m9");
    t.b.thinking("", T0 + 1000, true);
    t.b.thinking("a ", T0 + 1000, false);
    t.pousse(T0 + 1000);
    assert_eq!(t.p.vue(T0 + 1000).orbe, Some(OrbState::Weaving));
    t.b.tool(
        outil("g1", "Grep", json!({ "pattern": "Palantir" })),
        T0 + 2000,
    );
    t.pousse(T0 + 2000);
    let v = t.p.vue(T0 + 2000);
    assert_eq!(titre(&v), "Cherche « Palantir »…");
    assert_eq!(v.orbe, Some(OrbState::Weaving));
    t.b.finish(T0 + 3000, false);
    t.pousse(T0 + 3000);
    assert_eq!(t.p.vue(T0 + 3000).orbe, None);
    // Relayé depuis la fenêtre de réglages, le choix change en cours de tour.
    let mut t = Tour::demarre(OrbeChoix::Auto, "m10");
    t.b.tool(outil("g1", "Grep", json!({ "pattern": "x" })), T0 + 100);
    t.pousse(T0 + 100);
    assert_eq!(t.p.vue(T0 + 100).orbe, Some(OrbState::Searching));
    t.p.definir_orbe(OrbeChoix::Etat(OrbState::Weaving), T0 + 200);
    assert_eq!(t.p.vue(T0 + 200).orbe, Some(OrbState::Weaving));
}

// ── un tour simple ───────────────────────────────────────────────────────────

#[test]
fn un_tour_sans_pensee_ni_outil_l_orbe_ecrit_pendant_la_reponse_puis_plus_de_trace() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m11");
    assert_eq!(t.p.vue(T0).orbe, Some(OrbState::Solving));
    t.b.ecrire(T0 + 500);
    t.pousse(T0 + 500);
    let v = t.p.vue(T0 + 500);
    assert_eq!(v.orbe, Some(OrbState::Composing));
    assert_eq!(titre(&v), "Écrit la réponse…");
    // La fin : une trace vide est retirée par le fournisseur ; la poignée, elle, la dit « Terminé ».
    t.b.finish(T0 + 900, false);
    t.pousse(T0 + 900);
    let v = t.p.vue(T0 + 900);
    assert!(v.vide);
    assert_eq!(v.orbe, None);
    assert_eq!(titre(&v), "Terminé");
}

// ── détails de présentation ──────────────────────────────────────────────────

#[test]
fn un_resultat_long_est_coupe_a_six_lignes_non_vides() {
    let texte: String = (1..=9).map(|i| format!("ligne {i}\n\n")).collect();
    let e = fragment_ui::agent_trace::extrait(&texte);
    assert_eq!(e.lines().count(), 7);
    assert!(e.ends_with("… 3 ligne(s) de plus"));
}

#[test]
fn une_trace_interrompue_dit_interrompu() {
    let mut t = Tour::demarre(OrbeChoix::Auto, "m12");
    t.b.tool(outil("g1", "Grep", json!({ "pattern": "x" })), T0 + 100);
    t.b.finish(T0 + 300, true);
    t.pousse(T0 + 300);
    let v = t.p.vue(T0 + 300);
    assert_eq!(v.status, TraceStatus::Error);
    assert_eq!(titre(&v), "Interrompu · 1 recherche");
}

// ── la vue GPUI ──────────────────────────────────────────────────────────────

#[test]
fn la_vue_se_peint_repliee_puis_depliee_sans_fenetre_ni_gpu() {
    use fragment_ui::agent_trace::{TraceView, vue::trace_de_scene};
    use gpui_kit::{TestAppContext, px, size};
    let mut cx = TestAppContext::single();
    let (vue, vcx) = cx.add_window_view(|_, _| {
        let mut etats = EtatsTrace::new();
        TraceView::new(&mut etats, "vue", trace_de_scene(), OrbeChoix::Auto, true)
    });
    vcx.simulate_resize(size(px(600.0), px(400.0)));
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vue.update(vcx, |v, _| {
        assert!(!v.poignee().trace().steps.is_empty());
    });
    // Déplier : l'image suivante peint la liste sans panique.
    vue.update(vcx, |v, cx| v.update(trace_de_scene(), cx));
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}
