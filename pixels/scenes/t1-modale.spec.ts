// Scène T1 « t1-modale » : une modale générique (titre, contenu, bouton de fermeture), décor vide.
import { sceneFlottante } from './_t1-flottantes';
import { expect } from './_t1';

// ★ Fragment n'expose pas `Modal` sur `window` : on l'importe par le serveur Vite, sous forme de
// CHAÎNE (Playwright réécrit les `import()` d'une fonction en `require`, qui n'existe pas dans la page).
const OUVRIR = `(async () => {
    const { Modal } = await import('/src/ui/core/Modal.ts');
    new Modal(window.app)
        .setTitle('Titre de la modale')
        .setContent('Un paragraphe de contenu, pour mesurer le cadre de la modale.')
        .open();
})()`;

sceneFlottante('t1-modale', async ({ p, act, capturer }) => {
    await p.evaluate(OUVRIR);
    await expect(p.locator('.modal-container .modal').first()).toBeVisible();
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
    await act.key('Escape');
});
