// Scène T4 « t4-sans-toc » : un PDF sans table des matières : le message de la barre latérale.
import { sceneT4 } from './_t4';

sceneT4('t4-sans-toc', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-toolbar [aria-label="Toggle sidebar"]');
    await act.cliquer('.pdf-toolbar [aria-label="Sidebar options"]');
    await act.entreeMenu('Table of contents');
    await act.waitStable();
    await capturer();
}, { fichier: 'memo.pdf' });
