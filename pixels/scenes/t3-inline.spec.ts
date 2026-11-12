// Scène T3 « t3-inline » : gras, italique, code, surligné, barré, lien et lien interne rendus (aucun focus donné).
import { sceneT3a } from './_t3a';

sceneT3a('t3-inline', 'inline.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('deuxième ligne', '.doc-editor .cm-line', 2);
    await capturer();
});
