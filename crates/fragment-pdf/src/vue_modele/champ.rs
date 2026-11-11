// Port de app/src/ui/views/pdf/PdfView.ts (champ de page : change, focus, Échap)
//
// Le champ « page » de la barre d'outils : saisie bornée, Entrée va à la page,
// Échap restaure la page courante.

/// L'état du champ de saisie.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChampPage {
    /// Le texte affiché dans le champ.
    pub valeur: String,
    /// Le champ a le focus : le défilement ne doit pas écraser ce que l'on tape.
    pub focus: bool,
    /// Le contenu doit être sélectionné en entier (à la prise de focus).
    pub tout_selectionne: bool,
}

impl ChampPage {
    pub fn focaliser(&mut self) {
        self.focus = true;
        self.tout_selectionne = true;
    }

    pub fn saisir(&mut self, texte: &str) {
        self.valeur = texte.to_string();
        self.tout_selectionne = false;
    }

    /// Affiche la page courante, sauf si l'utilisateur est en train de taper.
    pub fn synchroniser(&mut self, page: usize) {
        if !self.focus {
            self.valeur = page.to_string();
        }
    }

    /// Remet le texte de la page courante (0 = aucune : champ vide).
    pub fn restaurer(&mut self, page: usize) {
        self.valeur = if page == 0 {
            String::new()
        } else {
            page.to_string()
        };
    }
}

/// `parseInt(texte, 10)` de JavaScript : blancs de tête ignorés, signe optionnel,
/// puis les chiffres du début ; le reste est ignoré (« 25abc » donne 25). `None` = NaN.
pub fn analyser_entier(texte: &str) -> Option<i64> {
    let t = texte.trim_start();
    let (negatif, reste) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let chiffres: String = reste.chars().take_while(char::is_ascii_digit).collect();
    if chiffres.is_empty() {
        return None;
    }
    // Un nombre trop long sature : il sera borné au total des pages ensuite.
    let n = chiffres.parse::<i64>().unwrap_or(i64::MAX);
    Some(if negatif { -n } else { n })
}
