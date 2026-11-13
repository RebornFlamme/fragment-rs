// Scène T7 « t7-chat-panel » : dock droit ouvert, la note au centre, une conversation de deux tours.
import { sceneT7a, MAINTENANT, AVEC_DOCK_DROIT } from './_t7a';

sceneT7a('t7-chat-panel', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(AVEC_DOCK_DROIT);
    await act.open('carnet.md');
    await act.chat('panel', 'c-panel');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
