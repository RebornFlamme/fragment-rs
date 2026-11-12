// Scène T3 « t3-etroite » : la colonne de la note à la taille minimale de la fenêtre (600 × 400),
// paragraphe qui passe à la ligne, tableau et code contenus sans défilement horizontal.
import { sceneT3b } from './_t3b';

sceneT3b('t3-etroite', 'etroite.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await capturer();
}, { largeur: 600, hauteur: 400 });
