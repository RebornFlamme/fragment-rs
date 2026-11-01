// Port de app/src/ui/core/debounce.ts
//
// L'utilitaire de coalescence (§6.3, §7.5) : `requestSave` d'un éditeur,
// `requestSaveLayout` du workspace, `requestSaveConfig` des réglages passent tous par ici.
// On n'écrit jamais le disque dans un chemin chaud, on le « demande ».
//
// ★ ÉCART DE MODÈLE : le TS arme un `setTimeout`. Le cœur n'a ni horloge ni exécuteur ;
//   le `Debouncer` est donc un automate à TEMPS INJECTÉ (millisecondes monotones fournies
//   par l'hôte) : `call` arme/repousse l'échéance, `poll` exécute si l'échéance est
//   passée, `next_deadline` dit à l'hôte quand le rappeler. Même sémantique, testable
//   sans dormir.

pub struct Debouncer<A> {
    cb: Box<dyn FnMut(A)>,
    timeout_ms: u64,
    reset_timer: bool,
    /// Échéance du minuteur armé.
    deadline: Option<u64>,
    pending: Option<A>,
}

impl<A> Debouncer<A> {
    /// - `timeout_ms` : délai ;
    /// - `reset_timer` : si `true`, chaque appel REPART du délai complet (« attendre que
    ///   ça se calme ») ; si `false`, le premier appel arme le minuteur et les suivants ne
    ///   le repoussent pas (« au plus une fois par fenêtre »). La frappe veut `true`.
    pub fn new(cb: impl FnMut(A) + 'static, timeout_ms: u64, reset_timer: bool) -> Self {
        Self {
            cb: Box::new(cb),
            timeout_ms,
            reset_timer,
            deadline: None,
            pending: None,
        }
    }

    /// Demande l'exécution avec ces arguments (les derniers gagnent).
    pub fn call(&mut self, args: A, now_ms: u64) {
        self.pending = Some(args);
        // Sans reset_timer, un minuteur déjà armé n'est pas repoussé.
        if self.deadline.is_some() && !self.reset_timer {
            return;
        }
        self.deadline = Some(now_ms + self.timeout_ms);
    }

    /// Exécute si l'échéance est passée. Rend `true` si le callback a tourné.
    pub fn poll(&mut self, now_ms: u64) -> bool {
        match self.deadline {
            Some(d) if now_ms >= d => self.fire(),
            _ => false,
        }
    }

    /// La prochaine échéance, pour que l'hôte arme SON minuteur.
    pub fn next_deadline(&self) -> Option<u64> {
        self.deadline
    }

    /// Exécute immédiatement si un appel est en attente, et annule le minuteur.
    pub fn run(&mut self) {
        self.fire();
    }

    /// Annule l'appel en attente sans l'exécuter.
    pub fn cancel(&mut self) {
        self.deadline = None;
        self.pending = None;
    }

    fn fire(&mut self) -> bool {
        self.deadline = None;
        match self.pending.take() {
            Some(args) => {
                (self.cb)(args);
                true
            }
            None => false,
        }
    }
}

/// La fonction du TS (`debounce(cb, timeout, resetTimer)`) ; docs/contrats.md : `fn debounce`.
pub fn debounce<A>(
    cb: impl FnMut(A) + 'static,
    timeout_ms: u64,
    reset_timer: bool,
) -> Debouncer<A> {
    Debouncer::new(cb, timeout_ms, reset_timer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn mk(timeout: u64, reset: bool) -> (Debouncer<i32>, Rc<RefCell<Vec<i32>>>) {
        let vu: Rc<RefCell<Vec<i32>>> = Rc::default();
        let v = vu.clone();
        (
            Debouncer::new(move |a| v.borrow_mut().push(a), timeout, reset),
            vu,
        )
    }

    // ─── Ajoutés : debounce.ts n'a pas de test TS ───
    #[test]
    fn coalesce_les_appels_et_garde_les_derniers_arguments() {
        let (mut d, vu) = mk(100, false);
        d.call(1, 0);
        d.call(2, 50);
        assert!(!d.poll(99));
        assert!(d.poll(100));
        assert_eq!(*vu.borrow(), vec![2]);
        assert!(!d.poll(500)); // une seule exécution
    }

    #[test]
    fn sans_reset_la_fenetre_nest_pas_repoussee() {
        let (mut d, _) = mk(100, false);
        d.call(1, 0);
        d.call(2, 90);
        assert_eq!(d.next_deadline(), Some(100));
    }

    #[test]
    fn avec_reset_chaque_appel_repart_du_delai_complet() {
        let (mut d, vu) = mk(100, true);
        d.call(1, 0);
        d.call(2, 90);
        assert_eq!(d.next_deadline(), Some(190));
        assert!(!d.poll(150));
        assert!(d.poll(190));
        assert_eq!(*vu.borrow(), vec![2]);
    }

    #[test]
    fn run_execute_tout_de_suite_et_desarme() {
        let (mut d, vu) = mk(100, false);
        d.call(7, 0);
        d.run();
        assert_eq!(*vu.borrow(), vec![7]);
        assert_eq!(d.next_deadline(), None);
        d.run(); // rien en attente
        assert_eq!(vu.borrow().len(), 1);
    }

    #[test]
    fn cancel_abandonne_sans_executer() {
        let (mut d, vu) = mk(100, false);
        d.call(1, 0);
        d.cancel();
        assert!(!d.poll(1000));
        assert!(vu.borrow().is_empty());
    }
}
