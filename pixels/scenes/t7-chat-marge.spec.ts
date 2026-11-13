// Scène T7 « t7-chat-marge » : la marge de lecture (1/4 – 3/4) sur le chat : la colonne glisse à droite.
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-marge', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-page');
    await act.command('workspace:toggle-margin');
    await expect.poll(() => p.evaluate(() => (window as any).app.workspace.activeLeaf.containerEl.classList.contains('is-margin-mode'))).toBe(true);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
