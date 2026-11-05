// Pont de test du spike S2, écrit pour ce dépôt : thème, hauteur, lien, touche.
// Le vrai pont de la vue HTML sera écrit par T9. Ici, l'hôte est une webview de premier
// niveau : tout part par window.ipc, en JSON marqué `fragmentHtml`.
(function () {
    function signaler(type, champs) {
        var message = Object.assign({ fragmentHtml: 1, type: type }, champs);
        window.ipc.postMessage(JSON.stringify(message));
    }

    // Variables CSS lues dans l'URL : posées dès maintenant, avant le premier script de la page.
    function poserTheme() {
        var encode = new URL(location.href).searchParams.get('fragment-theme');
        if (!encode) return;
        var theme = JSON.parse(encode);
        var regles = Object.entries(theme.vars || {}).map(function (paire) {
            return paire[0] + ': ' + paire[1];
        });
        var style = document.createElement('style');
        style.textContent = ':root { ' + regles.join('; ') + ' }';
        document.documentElement.prepend(style);
    }

    function suivreHauteur() {
        var connue = -1;
        function relever() {
            var hauteur = document.documentElement.scrollHeight;
            if (hauteur === connue) return;
            connue = hauteur;
            signaler('height', { height: hauteur });
        }
        new ResizeObserver(relever).observe(document.documentElement);
        relever();
    }

    poserTheme();
    signaler('hello', {});
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', suivreHauteur);
    else suivreHauteur();

    addEventListener('click', function (evenement) {
        var lien = evenement.target.closest && evenement.target.closest('a[href]');
        if (!lien) return;
        evenement.preventDefault();
        signaler('link', { href: lien.href });
    }, true);

    addEventListener('keydown', function (evenement) {
        signaler('key', { key: evenement.key, ctrlKey: evenement.ctrlKey });
    }, true);
})();
