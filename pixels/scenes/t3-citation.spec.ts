// Scène T3 « t3-citation » : citations simples, longues et imbriquées (aucun focus donné).
import { sceneT3a } from './_t3a';

sceneT3a('t3-citation', 'citation.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('deuxième ligne', '.doc-editor .cm-line', 2);
    await capturer();
});
