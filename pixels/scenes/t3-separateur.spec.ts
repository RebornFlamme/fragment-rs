// Scène T3 « t3-separateur » : Séparateurs horizontaux (aucun focus donné).
import { sceneT3b } from './_t3b';

sceneT3b('t3-separateur', 'separateur.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('trait', '.cm-hr', 0);
    await capturer();
});
