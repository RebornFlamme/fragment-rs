# ADR-010 — Arbre syntaxique Markdown : un port de `@lezer/markdown`, pas tree-sitter-md ni pulldown-cmark

Statut : proposé (équipier de `fr-t3-syntaxe`, 11/11/2026). Le principe du port a été accepté par le chef le même jour, comme troisième voie ; cet ADR consigne les mesures et les conséquences.

## Contexte

Fragment analyse ses notes avec Lezer (`@lezer/markdown` 1.7.2 via CodeMirror) : CommonMark, GFM (tableaux, tâches, barré, URL nues), indice, exposant, émoji, plus ses propres extensions (`app/src/ui/views/markdown/language.ts`) : surligné `==…==`, liens internes `[[…]]` et embeds `![[…]]`, maths `$…$` et `$$…$$` en ligne, `$$…$$` en bloc qui interrompt un paragraphe. Tout le live preview (`decorationPlugin`), les listes et les blocs lisent CET arbre : noms de nœuds, bornes exactes, imbrication. Un arbre qui découpe autrement change ce qui est masqué, agrandi ou rendu en bloc.

Le bead offrait deux voies : `tree-sitter-md` (incrémental, déjà au lock par la feature `tree-sitter-markdown` de gpui-kit) et `pulldown-cmark` (autorisé d'avance), à départager par la mesure contre Lezer.

## Mesures

Méthode : un script HORS du dépôt charge le vrai parseur de Fragment (`markdownLanguage.parser.configure(markdownExtensions)`, bundlé par l'esbuild de Fragment) et relève chaque nœud (`nom from to`, positions UTF-16, montages ignorés). Les arbres de fragment-rs sont relevés au même format (`releve_rust`, test `#[ignore]`) et comparés ligne à ligne.

**Fidélité** (part des nœuds de Lezer retrouvés avec le même nom, la même profondeur et la même plage) :

| Voie | Corpus | Nœuds conformes |
|---|---|---|
| tree-sitter-md 0.5.4 (noms traduits) | 7 documents réécrits, 303 nœuds | 70,6 % |
| pulldown-cmark 0.13.6 (événements traduits) | idem | 59,4 % |
| **port de @lezer/markdown** | idem (`tests/syntaxe/*.lezer`, en CI) | **100 %** |
| port | 39 documents des specs `editor-hell`, `markdown-html`, `properties` (et leurs fixtures) | **100 %** (39/39 identiques) |
| port | 98 fichiers Markdown réels des deux dépôts, 4,9 Mo, 108 546 nœuds | **100 %** (98/98 identiques) |

Les deux bibliothèques ignorent les extensions de Fragment (aucun `BlockMath` qui interrompt, ni surligné, ni lien interne, ni embed, ni maths en ligne) et découpent autrement même en CommonMark pur : marques séparées (`EmphasisMark`, `HeaderMark`, `ListMark`, `LinkMark`…), bornes de `ListItem`, `Task` sans texte, tableau qui ne s'arrête qu'à une ligne vide, blocs HTML.

**Vitesse**, document de 10 000 lignes (171 741 caractères, titres, gras, liens internes, maths, surligné, tâches, tableaux, code, citations ; test `mesure_dix_mille_lignes`, `--release`) :

| Voie | Analyse complète | Après une frappe au milieu |
|---|---|---|
| tree-sitter-md | ≈ 465 ms (bloc seul ≈ 220 ms) | ≈ 85 ms (incrémental) |
| pulldown-cmark | ≈ 4 à 7 ms (événements bruts, sans arbre) | réanalyse complète |
| Lezer (Node 22, même machine, au repos) | ≈ 82 à 109 ms | ≈ 4 à 11 ms (incrémental, `TreeFragment`) |
| **port** (174 409 octets, 33 347 nœuds) | **≈ 32 à 46 ms** (min et médiane de 5 passes, 3 séries) | **≈ 31 à 56 ms** (réanalyse complète) |

Les chiffres de tree-sitter-md et pulldown-cmark viennent de l'équipier précédent (machine chargée) ; ceux de Lezer et du port ont été relevés le même jour sur la même machine. Le port analyse le document entier 2 à 3 fois plus vite que Lezer ; en revanche, après une frappe, l'incrémental de Lezer (≈ 10 ms) reste 3 à 5 fois plus rapide que notre réanalyse complète. Sur une note réelle (quelques centaines de lignes, moins de 2 ms) la réanalyse complète tient largement dans une image à 60 Hz ; sur 10 000 lignes (≈ 40 ms) elle n'y tient pas : d'où le point 5.

## Décision

1. **L'arbre Markdown de fragment-rs est une traduction en Rust de `@lezer/markdown` 1.7.2** et des extensions de `language.ts` (`crates/fragment-markdown/src/syntaxe/`) : mêmes noms de nœuds (`Genre`), mêmes règles, mêmes plages. Aucune dépendance nouvelle.
2. **Positions** : chaque nœud porte sa plage en UTF-16 (le contrat, ADR-004) ET en octets (pour découper le `&str`). L'arbre est à plat (indices, parent, profondeur), avec un parcours limité à une plage (viewport), `noeud_a` (l'équivalent de `resolveInner`) et les ancêtres.
3. **Un ajout** : le nœud `FrontMatter`, absent de Lezer, enveloppe la tête `---…---` de la note (ce que Fragment détecte à part pour Properties). La comparaison avec Lezer le retire et remonte ses enfants.
4. **La vérité reste Lezer** : `tests/syntaxe/*.md` (documents réécrits) et leurs relevés `*.lezer` sont comparés en CI ; tout écart futur se tranche en relevant le vrai parseur hors du dépôt, jamais en ajustant le relevé à la main.
5. **Pas d'incrémental pour l'instant** : une frappe réanalyse le document entier (≈ 4 µs par ligne). Au-delà de quelques milliers de lignes, la frappe dépasse une image ; la piste est celle de Lezer (réutiliser les blocs de tête intacts avant la ligne modifiée), à ouvrir comme un bead propre.

## Licence

`@lezer/markdown` est sous licence MIT (© 2020 Marijn Haverbeke et contributeurs). Une traduction est une œuvre dérivée : la notice MIT complète est reproduite en tête de `crates/fragment-markdown/src/syntaxe/mod.rs`, qui désigne les fichiers concernés (`analyse_blocs.rs`, `en_ligne.rs`, `genre.rs`). La licence du dépôt est compatible. Les commentaires sont réécrits en français ; ni en-tête, ni diagramme, ni texte de Lezer ou de Fragment n'est recopié (ADR-007). Les extensions de Fragment sont traduites, pas copiées : `cargo xtask audit-sources` le vérifie avant fusion.

## Conséquences

- `fr-t3-decorations`, `fr-t3-blocs-modele` et `fr-t3-listes` lisent l'arbre par `Genre` et par plage UTF-16, exactement comme le TS lit l'arbre de Lezer : leurs ports gardent la logique du TS sans traduction de noms.
- `syntaxe/modele_rangees.rs` porte les types partagés par ces tâches (masques, hauteurs et polices par ligne, blocs, conversion vers `set_row_model` d'ADR-006). ★ La conversion d'un bloc `ALaPlace` multi-lignes (hauteur 0) n'est pas encore éprouvée sur le fork : à vérifier par `fr-t3-surface`.
- La coloration des langages dans les blocs de code (`parseMixed`, `stexMath` pour les maths) ne fait pas partie de cet arbre : elle reviendra au rendu des blocs.
- Une montée de version de `@lezer/markdown` dans Fragment se suit en relançant les relevés hors du dépôt ; un écart devient un bead.
- `tree-sitter-md` reste au lock par gpui-kit (éditeur stock, spike S1) mais n'est plus une dépendance de l'arbre Markdown.
