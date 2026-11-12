// Scène T3 « t3-alignement » : Titre, Properties, texte, tableau et code alignés sur le même bord gauche.
import { sceneT3b } from './_t3b';

sceneT3b('t3-alignement', 'alignement.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await capturer();
});
