// Scène T7 « t7-chat-menu » : le menu ⋯ de la vue chat (télécharger, copier, supprimer).
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-menu', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-page');
    await act.cliquer('button[aria-label="More options"]');
    await expect(p.locator('.menu')).toBeVisible();
    await act.waitStable();
    await capturer();
}, { garer: false });
