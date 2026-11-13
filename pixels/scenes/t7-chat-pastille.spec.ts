// Scène T7 « t7-chat-pastille » : on lit plus haut pendant un flux : la pastille « Réponse en cours » apparaît.
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-pastille', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-long');
    await act.agentScript('c-long', [{ type: 'text-delta', text: 'Je compare les relevés des deux semaines. ' }], true);
    await act.cliquer('.chat-view[data-mode="page"] .chat-composer-input');
    await act.type('Fais-moi une comparaison.');
    await act.key('Enter');
    await expect(p.locator('.chat-message[data-role="assistant"]').last()).toHaveAttribute('data-state', 'streaming');
    await act.defiler('u1');
    await expect(p.locator('.chat-jump')).toBeVisible();
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
