// Scène T3 « t3-tableau-cellule » : un clic dans une cellule du tableau la focalise (ligne 1, colonne 2).
import { sceneT3b } from './_t3b';

sceneT3b('t3-tableau-cellule', 'tableau.md', async ({ act, capturer, sonde }) => {
    await act.click(487, 350);
    await act.waitStable();
    await sonde('cellule focalisée', '.cm-table-cell', 4);
    await capturer();
});
