// Scène T3 « t3-titres » : les six niveaux de titre, marques « # » cachées (aucun focus donné).
import { sceneT3a } from './_t3a';

sceneT3a('t3-titres', 'titres.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('deuxième ligne', '.doc-editor .cm-line', 2);
    await capturer();
});
