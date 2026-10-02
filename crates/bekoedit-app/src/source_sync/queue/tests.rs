//! Review, 2026-10-02 §2.3: two submissions must reach their outcome in
//! submission order, never in whichever order their own async
//! processing happens to resolve.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;

use super::*;
use crate::source_sync::SourceCommand;

fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// `enqueue` itself, with the real `Submission` type: two sends land in
/// the channel in the order they were made. Necessary, but not the whole
/// guarantee -- see the next test for the part that actually matters,
/// processing order under per-item delay.
#[test]
fn enqueue_preserves_send_order_in_the_channel() {
    let (tx, mut rx) = futures_channel::mpsc::unbounded();
    tx.unbounded_send(Submission::WithoutFocusClaim {
        command: SourceCommand::SaveNow,
    })
    .unwrap();
    tx.unbounded_send(Submission::WithoutFocusClaim {
        command: SourceCommand::OpenSettings,
    })
    .unwrap();
    drop(tx);
    let names = run(async {
        let mut names = Vec::new();
        while let Some(submission) = rx.next().await {
            names.push(match submission {
                Submission::WithoutFocusClaim { command } => format!("{command:?}"),
                Submission::WithFocusClaim { command, .. } => format!("{command:?}"),
            });
        }
        names
    });
    assert_eq!(
        names,
        vec!["SaveNow".to_string(), "OpenSettings".to_string()]
    );
}

/// The shape `SourceCommandQueue` actually relies on: one coroutine,
/// draining one channel, `.await`ing each message's own processing fully
/// before taking the next. `Submission` itself carries live Dioxus
/// signals (needing a real reactive scope to construct), so this proves
/// the pattern with a toy message standing in for the real one -- a name,
/// and an artificial delay standing in for the real per-submission async
/// work (the pending-field commit's `eval_body` round trip, or the
/// focus-guard's arm exchange).
struct Delayed {
    name: &'static str,
    delay: Duration,
}

async fn drain_serially(
    mut rx: futures_channel::mpsc::UnboundedReceiver<Delayed>,
) -> Vec<&'static str> {
    let mut order = Vec::new();
    while let Some(msg) = rx.next().await {
        tokio::time::sleep(msg.delay).await;
        order.push(msg.name);
    }
    order
}

#[test]
fn draining_one_channel_serially_preserves_submission_order_even_when_the_first_is_slower() {
    let (tx, rx) = futures_channel::mpsc::unbounded();
    tx.unbounded_send(Delayed {
        name: "first",
        delay: Duration::from_millis(30),
    })
    .unwrap();
    tx.unbounded_send(Delayed {
        name: "second",
        delay: Duration::from_millis(1),
    })
    .unwrap();
    drop(tx);
    let order = run(drain_serially(rx));
    assert_eq!(order, vec!["first", "second"]);
}

/// The mutation: the shape this fix replaced -- one task spawned per
/// message, each racing through its own delay independently, with no
/// single consumer awaiting them in turn. This is not a mutation of
/// `queue.rs`'s own code (nothing in it can run headlessly end to end,
/// since the real consumer needs live Dioxus signals); it is the same
/// channel-level counter-example the review's own finding rests on,
/// demonstrating concretely why a single serial consumer is required and
/// spawning independently is not equivalent.
#[test]
fn spawning_one_task_per_submission_does_not_preserve_submission_order() {
    let order = Arc::new(Mutex::new(Vec::new()));
    run(async {
        let first = {
            let order = Arc::clone(&order);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                order.lock().unwrap().push("first");
            })
        };
        let second = {
            let order = Arc::clone(&order);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                order.lock().unwrap().push("second");
            })
        };
        first.await.unwrap();
        second.await.unwrap();
    });
    assert_eq!(*order.lock().unwrap(), vec!["second", "first"]);
}
