// Scène T3 « t3-tableau » : Tableau rendu, alignements de colonnes, aucune cellule focalisée.
import { sceneT3b } from './_t3b';

sceneT3b('t3-tableau', 'tableau.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('cellule 0', '.cm-table-cell', 0);
    await sonde('cellule 4', '.cm-table-cell', 4);
    await capturer();
});
