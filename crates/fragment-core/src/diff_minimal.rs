// Port de app/src/ui/core/diffMinimal.ts
//! Le plus petit remplacement qui transforme `avant` en `apres`.
//!
//! POURQUOI CETTE PRIMITIVE EXISTE. Réinjecter un contenu venu du dehors par un
//! remplacement plein document est correct sur le TEXTE et destructeur sur tout
//! le reste : l'éditeur mappe les positions à travers le changement, et toute
//! position INTÉRIEURE à la plage remplacée s'effondre sur la borne gauche,
//! c'est-à-dire 0. Mesuré dans Fragment sur un document de 206 caractères :
//!
//! ```text
//! curseur   0 → 0        curseur 103 → 0        curseur 206 → 206
//! ```
//!
//! Seule la fin exacte du document survit (elle suit la nouvelle fin), ce qui
//! rend le défaut trompeusement intermittent : qui édite en fin de fichier ne le
//! voit jamais. Le pliage, les décorations et l'historique d'annulation partent
//! avec.
//!
//! En ne remplaçant que le segment réellement différent, le mapping devient
//! trivialement juste : un curseur hors du segment ne bouge pas, un curseur
//! dedans se recale normalement. Et dans le cas courant — quelques caractères
//! changés par un `git pull` ou une synchro — la transaction est minuscule au
//! lieu d'être de la taille du fichier.
//!
//! ★ Unité des positions : des OCTETS UTF-8 (les offsets natifs d'une `String`
//!   Rust), là où le TS comptait des unités UTF-16. Le piège change de forme
//!   mais pas de nature : au lieu d'une paire de surrogates, c'est une séquence
//!   UTF-8 multi-octets qu'il ne faut pas couper. Un emoji partageant ses trois
//!   premiers octets avec un autre (😀 / 😁) arrête le préfixe commun EN PLEIN
//!   caractère ; on recule jusqu'à une frontière de caractère.

/// Le remplacement à appliquer : `avant[from..to]` devient `insert`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

/// Le plus petit remplacement de `avant` vers `apres`, ou `None` si les deux
/// contenus sont identiques (aucune transaction à émettre).
pub fn diff_minimal(avant: &str, apres: &str) -> Option<Replacement> {
    if avant == apres {
        return None;
    }

    let a = avant.as_bytes();
    let b = apres.as_bytes();
    let max = a.len().min(b.len());

    // Préfixe commun.
    let mut debut = 0;
    while debut < max && a[debut] == b[debut] {
        debut += 1;
    }

    // Suffixe commun, sans jamais empiéter sur le préfixe déjà consommé.
    let mut fin = 0;
    while fin < max - debut && a[a.len() - 1 - fin] == b[b.len() - 1 - fin] {
        fin += 1;
    }

    // ★ Les bornes ne doivent pas tomber AU MILIEU d'une séquence UTF-8.
    //   Reculer d'un octet à la fois suffit : une séquence fait au plus quatre
    //   octets, et le préfixe/suffixe commun garantit que les deux chaînes ont
    //   les mêmes octets de part et d'autre, donc les mêmes frontières.
    while debut > 0 && !avant.is_char_boundary(debut) {
        debut -= 1;
    }
    while fin > 0 && !avant.is_char_boundary(avant.len() - fin) {
        fin -= 1;
    }

    // Après ces reculs, les bornes peuvent se croiser sur des contenus très
    // courts : on les reclampe plutôt que d'émettre une plage inversée.
    if debut > avant.len() - fin || debut > apres.len() - fin {
        return Some(Replacement {
            from: 0,
            to: avant.len(),
            insert: apres.to_owned(),
        });
    }

    Some(Replacement {
        from: debut,
        to: avant.len() - fin,
        insert: apres[debut..apres.len() - fin].to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applique le remplacement rendu par diff_minimal — le contrat, en une ligne.
    fn appliquer(avant: &str, d: Option<&Replacement>) -> String {
        match d {
            None => avant.to_owned(),
            Some(d) => format!("{}{}{}", &avant[..d.from], d.insert, &avant[d.to..]),
        }
    }

    #[test]
    fn rend_null_quand_rien_ne_change() {
        assert_eq!(diff_minimal("bonjour", "bonjour"), None);
    }

    #[test]
    fn ne_touche_que_le_segment_modifie_au_milieu() {
        let d = diff_minimal("abcXYZdef", "abc123def").unwrap();
        assert_eq!(
            d,
            Replacement {
                from: 3,
                to: 6,
                insert: "123".into()
            }
        );
    }

    #[test]
    fn cible_la_fin_quand_on_ajoute_une_ligne() {
        let avant = "# Titre\n\nUn paragraphe.";
        let apres = format!("{avant}\nUne ligne de plus.");
        let d = diff_minimal(avant, &apres).unwrap();
        // Le préfixe commun couvre tout l'ancien contenu : rien n'est réécrit.
        assert_eq!(d.from, avant.len());
        assert_eq!(d.to, avant.len());
        assert_eq!(appliquer(avant, Some(&d)), apres);
    }

    #[test]
    fn cible_le_debut_quand_on_insere_en_tete() {
        let avant = "paragraphe";
        let apres = format!("# Titre\n{avant}");
        let d = diff_minimal(avant, &apres).unwrap();
        assert_eq!(d.from, 0);
        assert_eq!(d.to, 0);
        assert_eq!(appliquer(avant, Some(&d)), apres);
    }

    #[test]
    fn gere_une_suppression_pure() {
        let d = diff_minimal("abcdef", "abef").unwrap();
        assert_eq!(appliquer("abcdef", Some(&d)), "abef");
        assert_eq!(d.insert, "");
    }

    #[test]
    fn gere_le_passage_a_vide_et_depuis_vide() {
        assert_eq!(appliquer("abc", diff_minimal("abc", "").as_ref()), "");
        assert_eq!(appliquer("", diff_minimal("", "abc").as_ref()), "abc");
    }

    /// Le cas qui justifie le code de frontières : un emoji occupe quatre
    /// octets UTF-8 (deux unités UTF-16 en TS). Couper dedans produirait une
    /// chaîne invalide — ce que `String` refuse, là où CM6 stockait les deux
    /// demi-caractères tels quels.
    #[test]
    fn ne_coupe_pas_une_paire_de_surrogates_emoji() {
        // 😀 = U+1F600 ; 😁 = U+1F601. Le préfixe commun s'arrête NATURELLEMENT
        // au milieu du caractère, les deux emoji partageant leurs trois
        // premiers octets (F0 9F 98).
        let avant = "salut 😀 fin";
        let apres = "salut 😁 fin";

        let d = diff_minimal(avant, apres).unwrap();
        assert!(avant.is_char_boundary(d.from));
        assert!(avant.is_char_boundary(d.to));
        let resultat = appliquer(avant, Some(&d));

        assert_eq!(resultat, apres);
        // La borne n'a pas isolé un fragment de caractère.
        assert_eq!(
            resultat.chars().collect::<Vec<_>>(),
            apres.chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn reste_correct_sur_une_rafale_de_cas_aleatoires() {
        const ALPHABET: [char; 7] = ['a', 'b', 'c', '\n', '😀', 'é', ' '];
        // Un xorshift suffit : pas de dépendance pour un tirage de test.
        struct Xorshift(u64);
        impl Xorshift {
            fn suivant(&mut self) -> u64 {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                self.0
            }
            fn tirer(&mut self, n: usize) -> String {
                (0..n)
                    .map(|_| ALPHABET[(self.suivant() % ALPHABET.len() as u64) as usize])
                    .collect()
            }
        }
        let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);

        for _ in 0..500 {
            let na = (rng.suivant() % 30) as usize;
            let nb = (rng.suivant() % 30) as usize;
            let avant = rng.tirer(na);
            let apres = rng.tirer(nb);
            let d = diff_minimal(&avant, &apres);

            assert_eq!(appliquer(&avant, d.as_ref()), apres);
            if let Some(d) = d {
                assert!(d.to >= d.from);
                assert!(d.to <= avant.len());
                assert!(avant.is_char_boundary(d.from));
                assert!(avant.is_char_boundary(d.to));
            }
        }
    }
}
