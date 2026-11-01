// Port de app/src/ui/views/agent/controls/controlsModel.ts
//
// Les contrôles du champ de l'agent — leur logique PURE.

use crate::contrat::{HarnessModel, TurnUsage};

/// Le nom d'un niveau d'effort (ceux du SDK Claude ; un autre moteur garde son id).
pub const EFFORT_LABELS: [(&str, &str); 5] = [("low", "Rapide"), ("medium", "Normal"), ("high", "Approfondi"), ("xhigh", "Très approfondi"), ("max", "Maximal")];

pub fn effort_label(e: Option<&str>) -> String {
    match e.filter(|e| !e.is_empty()) {
        Some(e) => EFFORT_LABELS.iter().find(|(k, _)| *k == e).map_or_else(|| e.to_string(), |(_, v)| (*v).to_string()),
        None => "Par défaut".to_string(),
    }
}

/// Le modèle courant d'une conversation : son choix s'il est proposé, sinon le défaut du moteur.
pub fn current_model<'a>(models: &'a [HarnessModel], chosen: Option<&str>) -> Option<&'a HarnessModel> {
    chosen
        .and_then(|c| models.iter().find(|m| m.id == c))
        .or_else(|| models.iter().find(|m| m.default == Some(true)))
        .or_else(|| models.first())
}

/// `12 345` → `12 k` ; `1 234 567` → `1,2 M`.
pub fn compact_tokens(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    if n < 1_000_000 {
        return format!("{} k", ((n as f64) / 1000.0).round() as u64);
    }
    format!("{:.1} M", (n as f64) / 1_000_000.0).replace('.', ",")
}

/// L'anneau d'usage : la fraction de la fenêtre de contexte remplie (0..1) et son libellé.
#[derive(Debug, Clone, PartialEq)]
pub struct AnneauUsage {
    pub fraction: f64,
    pub label: String,
}

pub fn usage_ring(u: Option<&TurnUsage>) -> Option<AnneauUsage> {
    let u = u?;
    let contexte = u.context_tokens.filter(|c| *c > 0)?;
    let fenetre = u.context_window.unwrap_or(0);
    let fraction = if fenetre > 0 { (contexte as f64 / fenetre as f64).min(1.0) } else { 0.0 };
    let mut parts = vec![if fenetre > 0 {
        format!("{} / {} tokens de contexte", compact_tokens(contexte), compact_tokens(fenetre))
    } else {
        format!("{} tokens de contexte", compact_tokens(contexte))
    }];
    if let Some(c) = u.cost_usd {
        let decimales = if c < 0.1 { 3 } else { 2 };
        parts.push(format!("dernier tour {} $", format!("{c:.decimales$}").replace('.', ",")));
    }
    Some(AnneauUsage { fraction, label: parts.join(" · ") })
}

/// Le CONTEXTE que la barre / le chat joint à une question : le document regardé
/// (et le passage sélectionné). Rendu en tête du prompt, en anglais comme le reste
/// des consignes ; la langue de la réponse reste celle de la question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptContext {
    /// La clé du document (chemin du coffre ou `schéma:reste`).
    pub doc: String,
    pub title: String,
    /// Le texte sélectionné dans ce document, s'il y en a un.
    pub selection: Option<String>,
}

pub fn context_preamble(c: &PromptContext, prompt: &str) -> String {
    let lieu = if c.doc.contains(':') { format!("the document \"{}\" ({})", c.title, c.doc) } else { format!("the file \"{}\" of the vault", c.doc) };
    let sel = match c.selection.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => format!("\nThe user selected this passage in it:\n> {}", s.replace('\n', "\n> ")),
        None => String::new(),
    };
    format!("[Context: the user is looking at {lieu}.{sel}]\n\n{prompt}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, defaut: bool) -> HarnessModel {
        HarnessModel { id: id.into(), label: id.to_uppercase(), default: if defaut { Some(true) } else { None }, ..HarnessModel::default() }
    }

    #[test]
    fn modele_courant_le_choix_sil_existe_sinon_le_defaut() {
        let modeles = vec![m("a", false), m("b", true)];
        assert_eq!(current_model(&modeles, Some("a")).map(|x| x.id.as_str()), Some("a"));
        assert_eq!(current_model(&modeles, Some("zz")).map(|x| x.id.as_str()), Some("b"));
        assert_eq!(current_model(&modeles, None).map(|x| x.id.as_str()), Some("b"));
        assert_eq!(current_model(&[m("x", false)], None).map(|x| x.id.as_str()), Some("x"));
    }

    #[test]
    fn effort_un_libelle_ou_lid_dun_autre_moteur() {
        assert_eq!(effort_label(Some("high")), "Approfondi");
        assert_eq!(effort_label(Some("turbo")), "turbo");
        assert_eq!(effort_label(None), "Par défaut");
    }

    #[test]
    fn tokens_compacts() {
        assert_eq!(compact_tokens(950), "950");
        assert_eq!(compact_tokens(12_345), "12 k");
        assert_eq!(compact_tokens(1_234_567), "1,2 M");
    }

    #[test]
    fn anneau_dusage_fraction_de_la_fenetre_cout() {
        assert_eq!(usage_ring(None), None);
        let r = usage_ring(Some(&TurnUsage { context_tokens: Some(50_000), context_window: Some(200_000), cost_usd: Some(0.0421), ..TurnUsage::default() })).unwrap();
        assert_eq!(r.fraction, 0.25);
        assert_eq!(r.label, "50 k / 200 k tokens de contexte · dernier tour 0,042 $");
    }

    #[test]
    fn contexte_le_fichier_la_selection_citee_puis_la_question() {
        let c = |doc: &str, title: &str, sel: Option<&str>| PromptContext { doc: doc.into(), title: title.into(), selection: sel.map(str::to_string) };
        assert_eq!(context_preamble(&c("Cours/a.md", "a", None), "Résume"), "[Context: the user is looking at the file \"Cours/a.md\" of the vault.]\n\nRésume");
        assert_eq!(
            context_preamble(&c("chat:agent/x", "Conv", Some("un\ndeux")), "Q"),
            "[Context: the user is looking at the document \"Conv\" (chat:agent/x).\nThe user selected this passage in it:\n> un\n> deux]\n\nQ"
        );
    }
}
