# ADR-008 — Le workspace n'utilise pas `DockArea` de gpui-kit

Statut : accepté par l'orchestrateur le 06/11/2026.

## Contexte

Le choix de gpui-kit (ADR-001) comptait son dock, ses onglets et ses splits parmi ses atouts. Or Fragment est jugé au pixel sur les gabarits de `workspace.css`, et l'arbre du workspace est un modèle du cœur, que `fragment-core` tient sans dépendre de gpui-kit. `DockArea` imposerait son propre modèle et son propre rendu.

## Décision

1. `fragment-ui` rend l'arbre du workspace sur des `div` flex, depuis le modèle du cœur. `DockArea` ne sert plus ; `Coquille` est réécrite par `fr-t1-binaire`.
2. Poignées, glisser et zones de dépôt sont bâtis sur les modèles déjà portés : `layout`, `DragManager`, `snapModel`, et la règle du maximum de `resizeGesture`.
3. gpui-kit reste pour les primitives : menus, champs, listes virtuelles, infobulles.

## Conséquences

Plus de code de rendu à écrire et à tester (`fr-t1-splits`, `fr-t1-onglets`). `fr-t1-lecteur` doit prouver le geste `drag` avant `fr-t1-splits`, car la capture du pointeur n'est pas éprouvée en headless.
