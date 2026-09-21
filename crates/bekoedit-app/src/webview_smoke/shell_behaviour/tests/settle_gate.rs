//! Task 021: the settle gate.

use std::cell::RefCell;
use std::rc::Rc;

use bekoedit_core::AppState;
use bekoedit_fs::RecoveryStore;
use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, EditorInstanceId, SourceEditorEvent, SourceEditorId, SourceEpoch,
    },
};

use super::*;
use crate::source_sync::lifecycle::{LifecycleState, ReadyEditor};
use crate::webview_smoke::transport::PhaseCompletion;

fn identity() -> EditorIdentity {
    EditorIdentity {
        instance_id: EditorInstanceId::new(1),
        editor_id: SourceEditorId::Text,
        document_id: 7,
        epoch: SourceEpoch::new(1),
    }
}

/// A controller that has just begun tearing its Text editor down, reached
/// through `force_unmount` -- what dropping `TextMode` does.
fn unmounting_controller() -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = LifecycleState::Ready(ReadyEditor {
        identity: identity(),
        revision: 1,
        last_seq: 0,
    });
    sync.force_unmount(10);
    assert_eq!(sync.busy_lifecycle_state(), Some("Unmounting"));
    sync
}

/// What the page's `destroyed` event does when it reaches Rust.
fn deliver_destroyed(sync: &mut SourceSyncState) {
    let LifecycleState::Unmounting { operation, .. } = sync.lifecycle.state.clone() else {
        panic!("expected Unmounting, found {:?}", sync.lifecycle.state);
    };
    let dir = tempfile::tempdir().unwrap().keep();
    let mut app = AppState::new(
        RecoveryStore::at(dir.join(".recovery")),
        dir.join(".recent.json"),
        100,
    );
    sync.handle_event(
        SourceEditorEvent::Destroyed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: operation.operation_id,
            identity: identity(),
        },
        &mut app,
        20,
    )
    .unwrap();
}

fn completed(
    kind: MessageKind,
    phase: ShellBehaviourPhase,
    exchange_id: u64,
    release: Option<PinnedExchange<ShellBehaviourPhase>>,
) -> CompletedProbe<ShellBehaviourPhase> {
    let mut message = phase_message(kind, phase.as_str(), exchange_id);
    if let Some(release) = release {
        message.released_exchange_id = Some(release.exchange_id);
        message.released_phase = Some(release.phase.as_str().into());
    }
    CompletedProbe {
        message,
        completion: PhaseCompletion {
            protocol_version: SMOKE_PROTOCOL_VERSION,
            exchange_id,
            phase: phase.as_str().into(),
            kind,
            acknowledgement_processed: true,
            evaluator_pinned: true,
        },
        pin: PinnedExchange { exchange_id, phase },
    }
}

fn quick_gate(deadline_ms: u64) -> SettleGate {
    SettleGate {
        deadline: Duration::from_millis(deadline_ms),
        poll: Duration::from_millis(2),
    }
}

fn reading(sync: &Rc<RefCell<SourceSyncState>>) -> impl FnMut() -> Option<String> + use<> {
    let sync = sync.clone();
    move || sync.borrow().busy_lifecycle_state().map(str::to_string)
}

#[tokio::test]
async fn the_first_exchange_is_not_requested_while_the_controller_is_unmounting() {
    let sync = Rc::new(RefCell::new(unmounting_controller()));
    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    let request_events = events.clone();
    let release_events = events.clone();
    let release_sync = sync.clone();
    let terminal = ShellBehaviourTerminal::default();

    let sequence = run_shell_behaviour_sequence(
        &terminal,
        quick_gate(5_000),
        reading(&sync),
        move |phase, _, _| {
            request_events
                .borrow_mut()
                .push(format!("requested:{}", phase.as_str()));
            std::future::ready(Err("stop after the first request".into()))
        },
    );
    let release = async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        release_events.borrow_mut().push("destroyed".into());
        deliver_destroyed(&mut release_sync.borrow_mut());
    };
    let (result, ()) = tokio::join!(sequence, release);

    assert_eq!(result.unwrap_err(), "stop after the first request");
    assert_eq!(
        *events.borrow(),
        ["destroyed", "requested:recovery_entry"],
        "the exchange must wait for the Destroyed event"
    );
}

/// D2's shape: an exchange returns pending after the Preview switch, the
/// controller is tearing down, and the next exchange -- Text's -- must
/// wait for the teardown to finish.
#[tokio::test]
async fn the_exchange_after_a_pending_one_waits_for_the_teardown() {
    let sync = Rc::new(RefCell::new(SourceSyncState::default()));
    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    let request_events = events.clone();
    let request_sync = sync.clone();
    let release_events = events.clone();
    let release_sync = sync.clone();
    let terminal = ShellBehaviourTerminal::default();

    let sequence = run_shell_behaviour_sequence(
        &terminal,
        quick_gate(5_000),
        reading(&sync),
        move |phase, exchange_id, release| {
            request_events
                .borrow_mut()
                .push(format!("requested:{exchange_id}"));
            if exchange_id == 1 {
                // The first exchange switched to Preview: the editor's
                // teardown begins.
                *request_sync.borrow_mut() = unmounting_controller();
                std::future::ready(Ok(completed(
                    MessageKind::Pending,
                    phase,
                    exchange_id,
                    release,
                )))
            } else {
                std::future::ready(Err("stop after the second request".into()))
            }
        },
    );
    let release = async move {
        // Waits until the sequence has requested the first exchange.
        while release_events.borrow().is_empty() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        release_events.borrow_mut().push("destroyed".into());
        deliver_destroyed(&mut release_sync.borrow_mut());
    };
    let (result, ()) = tokio::join!(sequence, release);

    assert_eq!(result.unwrap_err(), "stop after the second request");
    assert_eq!(
        *events.borrow(),
        ["requested:1", "destroyed", "requested:2"],
        "the gate must hold the second exchange until the teardown is done"
    );
}

#[tokio::test]
async fn a_settled_controller_is_not_delayed() {
    let sync = Rc::new(RefCell::new(SourceSyncState::default()));
    let terminal = ShellBehaviourTerminal::default();
    let result =
        run_shell_behaviour_sequence(&terminal, quick_gate(5_000), reading(&sync), |_, _, _| {
            std::future::ready(Err("first request reached".into()))
        })
        .await;
    assert_eq!(result.unwrap_err(), "first request reached");
}

#[tokio::test]
async fn a_controller_that_never_settles_fails_naming_the_phase_and_the_state() {
    let sync = Rc::new(RefCell::new(unmounting_controller()));
    let requested = Rc::new(RefCell::new(0_u32));
    let counted = requested.clone();
    let terminal = ShellBehaviourTerminal::default();
    let result =
        run_shell_behaviour_sequence(&terminal, quick_gate(30), reading(&sync), move |_, _, _| {
            *counted.borrow_mut() += 1;
            std::future::ready(Err("must not be requested".into()))
        })
        .await;

    let error = result.unwrap_err();
    assert!(error.contains("recovery_entry"), "{error}");
    assert!(error.contains("Unmounting"), "{error}");
    assert!(error.contains("did not settle"), "{error}");
    assert_eq!(*requested.borrow(), 0, "no exchange is requested");
}
