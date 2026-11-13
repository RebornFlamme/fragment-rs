// Scène T7 « t7-chat-flux » : une question envoyée, la réponse en cours d'écriture (le tour figé, aucune trace).
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-flux', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-flux');
    await act.agentScript('c-flux', [
        { type: 'text-delta', text: 'Je relis tes notes de la semaine. ' },
        { type: 'text-delta', text: 'La date du deuxième relevé est la seule ' },
    ], true);
    await act.cliquer('.chat-view[data-mode="page"] .chat-composer-input');
    await act.type('Quelle date manque ?');
    await act.key('Enter');
    await expect(p.locator('.chat-message[data-role="assistant"]').last()).toHaveAttribute('data-state', 'streaming');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
