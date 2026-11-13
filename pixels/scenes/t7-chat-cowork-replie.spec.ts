// Scène T7 « t7-chat-cowork-replie » : la barre ouverte puis repliée : la colonne retrouve la place, le bouton d'ouverture reste en tête.
import { sceneT7a, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-cowork-replie', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-page');
    await act.cliquer('.chat-view[data-mode="page"] .chat-header-toggle');
    await act.waitStable();
    await act.cliquer('.chat-sidebar .chat-sidebar-toggle');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
