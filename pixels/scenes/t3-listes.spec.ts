// Scène T3 « t3-listes » : puces, sous-puces, liste numérotée, barres d'indentation (aucun focus donné).
import { sceneT3a } from './_t3a';

sceneT3a('t3-listes', 'listes.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('deuxième ligne', '.doc-editor .cm-line', 2);
    await capturer();
});
