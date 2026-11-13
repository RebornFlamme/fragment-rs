# Mémoire de la vue PDF (fr-7va)

Banc : `cargo test -p fragment-ui --test vue_pdf mesure -- --ignored --nocapture` (profil debug,
fenêtre de test 1000 × 800, rendu CPU, pdfium chromium-8125). Le PDF est généré par le test
(300 pages 420 × 600, une ligne de texte par page, 0,08 Mo) : une fixture écrite ici, pas un PDF de
Fragment. Mémoire = jeu de travail du processus de test (`WorkingSet64`).

| Moment | Jeu de travail |
|---|---|
| Avant l'ouverture (fenêtre de test et vue créées, fichier non chargé) | 13,2 Mo |
| Après l'ouverture (300 pages comptées, premières pages peintes, table des matières lue) | 33,3 Mo |
| Pic échantillonné pendant le défilement (une page sur cinq, de 1 à 300) | 40,0 Mo |
| Après le défilement complet | 40,0 Mo |

Lecture : l'ouverture d'un Document pdfium par vue, sur son fil dédié, coûte environ 20 Mo pour
300 pages ; le défilement complet en ajoute environ 7 Mo et la mémoire ne remonte pas au-delà
(les canevas des pages sorties de la fenêtre sont libérés). Pas de fuite visible sur ce banc.

Limites : le contenu des pages est léger (du texte seul) ; un PDF d'images pèsera davantage. À
refaire en release sur un PDF réel de 300 pages au banc de la phase 4 (program.md §5).
