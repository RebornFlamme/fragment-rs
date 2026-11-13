// Scène T7 « t7-chat-rail » : le rail des tours, l'aperçu (question et réponse) du tiret survolé.
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-rail', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-long');
    await act.survoler('.chat-view[data-mode="page"] .chat-rail-tick', 2);
    await expect(p.locator('.chat-rail-preview')).toBeVisible();
    await act.waitStable();
    await capturer();
}, { garer: false });
