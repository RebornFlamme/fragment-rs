// Scène T3 « t3-lecture-seule » : la note rendue en lecture seule, après un clic sur le bouton
// « Mode lecture seule » de l'en-tête de vue (Properties verrouillées, pas de bouton d'ajout).
import { sceneT3b } from './_t3b';

sceneT3b('t3-lecture-seule', 'alignement.md', async ({ act, p, capturer, sonde }) => {
    await sonde('bouton lecture seule', '.view-action[aria-label="Mode lecture seule"]');
    const b = (await p.locator('.view-action[aria-label="Mode lecture seule"]').boundingBox())!;
    await act.click(Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
    await p.waitForSelector('.metadata-container.is-readonly');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
