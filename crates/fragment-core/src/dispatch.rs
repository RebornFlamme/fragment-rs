// Port de app/src/ui/core/external/dispatch.ts, app/src/ui/core/cli/dispatch.ts et de
// `App.handleProtocol` (app.ts).
//
// Une requête externe → le handler qui lui correspond → sa sortie. Le SEUL endroit qui
// connaît le registre `externalCommands`. PUR : un registre et une requête neutre en
// entrée, un `ExternalResult` en sortie — testable sans aucun transport.
//
// ★ Ne LÈVE jamais. Une erreur de handler devient un résultat d'échec avec un code : celui
//   de l'`ExternalError` si le handler en a choisi un, `EXTERNAL_HANDLER_ERROR` sinon. Seul
//   le MESSAGE part (A21) ; la pile — qui porte les chemins d'installation — se journalise
//   en local. Un handler qui PANIQUE est traité comme une erreur ordinaire (le pendant
//   du `try/catch` du TS).
// ★ Les `CliRequest` / `CliResult` vivent dans fragment-cli, qui dépend de ce cœur : ici
//   l'adaptateur CLI prend la trame DÉMONTÉE (sous-commande, params, drapeaux).

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::external::{
    EXTERNAL_HANDLER_ERROR, EXTERNAL_UNKNOWN, ExternalCommandSpec, ExternalParams, ExternalResult,
    HandlerError,
};
use crate::registry::Registry;

/// Le registre `app.externalCommands`.
pub type ExternalCommands = Registry<ExternalCommandSpec>;

/// Une requête externe neutre : une action, des paramètres chaîne.
pub fn dispatch_external(
    registry: &ExternalCommands,
    action: &str,
    params: &ExternalParams,
) -> ExternalResult {
    let Some(spec) = registry.get(action) else {
        // Un id de plugin sans son préfixe est l'erreur la plus courante : on propose
        // ce qui ressemble.
        let suffixe = format!(":{action}");
        let proches: Vec<String> = registry
            .keys()
            .into_iter()
            .filter(|k| k.ends_with(&suffixe) || k.starts_with(action))
            .collect();
        let indice = if proches.is_empty() {
            " « help » liste les actions disponibles.".to_owned()
        } else {
            format!(" Vouliez-vous dire : {} ?", proches.join(", "))
        };
        return ExternalResult::Err {
            code: EXTERNAL_UNKNOWN,
            error: format!("action inconnue : {action}.{indice}"),
        };
    };

    match catch_unwind(AssertUnwindSafe(|| (spec.handler)(params))) {
        Ok(Ok(output)) => ExternalResult::Ok { output },
        Ok(Err(HandlerError::External(e))) => ExternalResult::Err {
            code: e.code,
            error: e.message,
        },
        Ok(Err(HandlerError::Other(message))) => {
            // La pile reste en local ; le client ne voit que le message.
            log::error!("[external] {action} a levé : {message}");
            ExternalResult::Err {
                code: EXTERNAL_HANDLER_ERROR,
                error: message,
            }
        }
        Err(panique) => {
            let message = panique
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| panique.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panique du handler".to_owned());
            log::error!("[external] {action} a paniqué : {message}");
            ExternalResult::Err {
                code: EXTERNAL_HANDLER_ERROR,
                error: message,
            }
        }
    }
}

/// L'adaptateur CLI du port externe : fusionne drapeaux (→ `"true"`) puis params (qui
/// GAGNENT en cas de collision : la forme d'`obsidian://`, où `--open` vaut `"true"` et
/// `open=false` le bat), délègue à `dispatch_external`.
pub fn executer_requete_cli(
    registry: &ExternalCommands,
    subcommand: &str,
    params: &ExternalParams,
    flags: &[String],
) -> ExternalResult {
    let mut fusion = ExternalParams::new();
    for flag in flags {
        fusion.insert(flag.clone(), "true".to_owned());
    }
    fusion.extend(params.iter().map(|(k, v)| (k.clone(), v.clone())));
    dispatch_external(registry, subcommand, &fusion)
}

/// L'adaptateur `fragment://` (fire-and-forget) : dispatche sur le port unifié et
/// journalise un échec — le client d'une URL n'a pas de canal de retour.
pub fn handle_protocol(registry: &ExternalCommands, action: &str, params: &ExternalParams) {
    if let ExternalResult::Err { error, .. } = dispatch_external(registry, action, params) {
        log::warn!("[protocol] {action} : {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::ExternalError;
    use log::{Level, Log, Metadata, Record};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Mutex;

    fn faire_app() -> ExternalCommands {
        Registry::new("externalCommands", "Commandes externes")
    }

    fn params(paires: &[(&str, &str)]) -> ExternalParams {
        paires
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn spec(
        action: &str,
        h: impl Fn(&ExternalParams) -> Result<String, HandlerError> + 'static,
    ) -> ExternalCommandSpec {
        ExternalCommandSpec::new(action, h)
    }

    fn reg(app: &ExternalCommands, s: ExternalCommandSpec) {
        let action = s.action.clone();
        // Disposer volontairement ignoré : l'entrée reste pour la durée du test.
        let _ = app.register(&action, s, "test");
    }

    // ─── dispatchExternal — le port générique (CLI + fragment://) ───

    #[test]
    fn trouve_le_handler_et_rend_sa_sortie_coercee_en_chaine() {
        let app = faire_app();
        reg(&app, spec("x", |_| Ok("ok".into())));
        assert_eq!(
            dispatch_external(&app, "x", &params(&[])),
            ExternalResult::Ok {
                output: "ok".into()
            }
        );
    }

    #[test]
    fn passe_les_params_au_handler_et_attend_lasync() {
        let app = faire_app();
        let vu: Rc<RefCell<Vec<ExternalParams>>> = Rc::default();
        let v = vu.clone();
        reg(
            &app,
            spec("x", move |p| {
                v.borrow_mut().push(p.clone());
                Ok(format!("salut {}", p["qui"]))
            }),
        );
        let r = dispatch_external(&app, "x", &params(&[("qui", "toi")]));
        assert_eq!(
            r,
            ExternalResult::Ok {
                output: "salut toi".into()
            }
        );
        assert_eq!(*vu.borrow(), vec![params(&[("qui", "toi")])]);
    }

    #[test]
    fn action_inconnue_external_unknown_avec_une_piste_sur_le_prefixe_manquant() {
        let app = faire_app();
        reg(&app, spec("templater:create", |_| Ok(String::new())));
        let r = dispatch_external(&app, "create", &params(&[]));
        match r {
            ExternalResult::Err { code, error } => {
                assert_eq!(code, EXTERNAL_UNKNOWN);
                assert!(error.contains("templater:create"), "{error}");
            }
            autre => panic!("attendu un échec, reçu {autre:?}"),
        }
    }

    #[test]
    fn une_external_error_choisit_son_code_toute_autre_erreur_handler_error_le_message_seul() {
        let app = faire_app();
        reg(
            &app,
            spec("a", |_| Err(ExternalError::new(8, "indispo").into())),
        );
        reg(&app, spec("b", |_| Err("boum".into())));

        assert_eq!(
            dispatch_external(&app, "a", &params(&[])),
            ExternalResult::Err {
                code: 8,
                error: "indispo".into()
            }
        );
        assert_eq!(
            dispatch_external(&app, "b", &params(&[])),
            ExternalResult::Err {
                code: EXTERNAL_HANDLER_ERROR,
                error: "boum".into()
            }
        );
    }

    // ─── App.handleProtocol — l'adaptateur fragment:// (fire-and-forget) ───

    #[test]
    fn dispatche_sur_le_port_unifie_un_handler_enregistre_repond_aussi_a_lurl() {
        let app = faire_app();
        let vu: Rc<RefCell<Vec<ExternalParams>>> = Rc::default();
        let v = vu.clone();
        reg(
            &app,
            spec("ouvrir", move |p| {
                v.borrow_mut().push(p.clone());
                Ok("peu importe la sortie, l’URL l’ignore".into())
            }),
        );
        handle_protocol(&app, "ouvrir", &params(&[("note", "x")]));
        assert_eq!(*vu.borrow(), vec![params(&[("note", "x")])]);
    }

    /// Un journal de test : retient les avertissements émis par ce module.
    struct Journal;
    static AVERTISSEMENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    impl Log for Journal {
        fn enabled(&self, _: &Metadata) -> bool {
            true
        }
        fn log(&self, r: &Record) {
            if r.level() == Level::Warn
                && r.target().ends_with("dispatch")
                && let Ok(mut v) = AVERTISSEMENTS.lock()
            {
                v.push(r.args().to_string());
            }
        }
        fn flush(&self) {}
    }

    #[test]
    fn action_inconnue_ne_leve_pas_journalise_juste_un_avertissement() {
        // Un seul logger par processus : on l'installe si personne ne l'a fait.
        let _ = log::set_logger(&Journal);
        log::set_max_level(log::LevelFilter::Warn);
        let app = faire_app();
        handle_protocol(&app, "nope-unique", &params(&[]));
        let v = AVERTISSEMENTS.lock().unwrap();
        let n = v.iter().filter(|m| m.contains("nope-unique")).count();
        assert_eq!(n, 1);
    }

    // ─── executerRequeteCli — la couture trame ↔ registre ───

    fn flags(f: &[&str]) -> Vec<String> {
        f.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn appelle_le_handler_avec_params_et_drapeaux_fusionnes_un_drapeau_vaut_true() {
        let app = faire_app();
        let vu: Rc<RefCell<Vec<ExternalParams>>> = Rc::default();
        let v = vu.clone();
        reg(
            &app,
            spec("x", move |p| {
                v.borrow_mut().push(p.clone());
                Ok("ok".into())
            }),
        );
        let r = executer_requete_cli(&app, "x", &params(&[("a", "1")]), &flags(&["verbose"]));
        assert_eq!(
            r,
            ExternalResult::Ok {
                output: "ok".into()
            }
        );
        assert_eq!(
            *vu.borrow(),
            vec![params(&[("verbose", "true"), ("a", "1")])]
        );
    }

    #[test]
    fn les_params_gagnent_sur_les_drapeaux_en_cas_de_collision() {
        let app = faire_app();
        let vu: Rc<RefCell<Vec<ExternalParams>>> = Rc::default();
        let v = vu.clone();
        reg(
            &app,
            spec("x", move |p| {
                v.borrow_mut().push(p.clone());
                Ok(String::new())
            }),
        );
        executer_requete_cli(&app, "x", &params(&[("open", "false")]), &flags(&["open"]));
        assert_eq!(*vu.borrow(), vec![params(&[("open", "false")])]);
    }

    #[test]
    fn sous_commande_inconnue_4_avec_une_piste_quand_le_prefixe_manque() {
        let app = faire_app();
        reg(&app, spec("templater:create", |_| Ok(String::new())));
        let r = executer_requete_cli(&app, "create", &params(&[]), &[]);
        assert!(
            matches!(&r, ExternalResult::Err { code: 4, error } if error.contains("templater:create"))
        );
    }

    #[test]
    fn une_cli_error_choisit_son_code_une_autre_erreur_donne_1_le_message_pas_la_pile() {
        let app = faire_app();
        reg(
            &app,
            spec("a", |_| Err(ExternalError::new(7, "pas là").into())),
        );
        reg(&app, spec("b", |_| Err("boum".into())));
        assert_eq!(
            executer_requete_cli(&app, "a", &params(&[]), &[]),
            ExternalResult::Err {
                code: 7,
                error: "pas là".into()
            }
        );
        assert_eq!(
            executer_requete_cli(&app, "b", &params(&[]), &[]),
            ExternalResult::Err {
                code: 1,
                error: "boum".into()
            }
        );
    }

    #[test]
    fn un_handler_asynchrone_est_attendu() {
        // ★ Pas d'async dans le cœur (voir external.rs) : le handler rend sa sortie.
        let app = faire_app();
        reg(&app, spec("x", |_| Ok("tard".into())));
        assert_eq!(
            executer_requete_cli(&app, "x", &params(&[]), &[]),
            ExternalResult::Ok {
                output: "tard".into()
            }
        );
    }

    // ─── Ajouté ───
    #[test]
    fn un_handler_qui_panique_devient_un_echec_code_1() {
        let app = faire_app();
        reg(&app, spec("p", |_| panic!("kaboum")));
        assert_eq!(
            dispatch_external(&app, "p", &params(&[])),
            ExternalResult::Err {
                code: 1,
                error: "kaboum".into()
            }
        );
    }
}
