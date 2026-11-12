// Scène T4 « t4-toc » : table des matières dépliée (chapitres 1 et 2 ouverts).
import { sceneT4 } from './_t4';

sceneT4('t4-toc', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-toolbar [aria-label="Toggle sidebar"]');
    await act.cliquer('.pdf-toolbar [aria-label="Sidebar options"]');
    await act.entreeMenu('Table of contents');
    await act.cliquer('.pdf-outline-view > .pdf-outline-item:nth-child(2) > .tree-item-self .collapse-icon');
    await act.cliquer('.pdf-outline-view > .pdf-outline-item:nth-child(3) > .tree-item-self .collapse-icon');
    await act.waitStable();
    await capturer();
});
