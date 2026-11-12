// Scène T4 « t4-etroite » : fenêtre de 600 px, barre latérale ouverte : rien ne déborde, la barre d'outils tient.
import { sceneT4 } from './_t4';

sceneT4('t4-etroite', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-toolbar [aria-label="Toggle sidebar"]');
    await p.waitForSelector('.pdf-thumbnail canvas');
    await p.waitForTimeout(800);
    await act.waitStable();
    await capturer();
}, { fenetre: { largeur: 600, hauteur: 800 } });
