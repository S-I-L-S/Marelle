// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

//! Cancelling an answer that is being waited for.
//!
//! The full-screen interface does not read the keyboard while a model is
//! answering, because the request is driven to completion by `block_on`.
//! [`until_cancelled`] gives that wait a way out: every few milliseconds it
//! looks up and asks whether the person has asked to stop. [`settle`] then
//! decides what a stopped turn leaves behind in the conversation.

use std::future::{Future, poll_fn};
use std::io;
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use marelle_core::{Message, Role, Session};

/// How often a wait looks up to ask whether it was cancelled.
const LISTEN_EVERY: Duration = Duration::from_millis(40);

/// How a wait ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Waited<T> {
    /// The work finished first.
    Done(T),
    /// The person cancelled first. The work was dropped unfinished, which is
    /// what closes a request that is still in flight.
    Cancelled,
}

/// Drives `work` to its end unless `cancelled` says stop first.
///
/// `cancelled` runs between waits and never while `work` is being polled, so
/// it may briefly block, for instance to read the terminal. It is not asked at
/// all when the work is ready at once. An error from it ends the wait, and the
/// work is dropped like in a cancel.
pub(crate) async fn until_cancelled<F, C>(
    work: F,
    mut cancelled: C,
) -> io::Result<Waited<F::Output>>
where
    F: Future,
    C: FnMut() -> io::Result<bool>,
{
    let mut work = pin!(work);
    loop {
        let mut tick = pin!(tokio::time::sleep(LISTEN_EVERY));
        let finished = poll_fn(|cx| {
            if let Poll::Ready(output) = work.as_mut().poll(cx) {
                return Poll::Ready(Some(output));
            }
            tick.as_mut().poll(cx).map(|()| None)
        })
        .await;
        if let Some(output) = finished {
            return Ok(Waited::Done(output));
        }
        if cancelled()? {
            return Ok(Waited::Cancelled);
        }
    }
}

/// What a cancelled turn leaves in the conversation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Settled {
    /// No tool had run, so nothing happened on the machine and the question is
    /// taken back as if it had not been asked. Its text comes back so that it
    /// can be edited and sent again.
    Withdrawn(String),
    /// Tools had already acted. Their calls and results stay, so that the next
    /// question can build on what was done, and a note closes the turn so the
    /// conversation still alternates the way the providers expect.
    Kept,
}

/// Puts the conversation in order after a cancel. The last message is the
/// person's question when nothing has run yet. `note` is what closes a turn
/// whose work is kept.
pub(crate) fn settle(session: &mut Session, tools_ran: bool, note: &str) -> Settled {
    if tools_ran {
        session.push(Message::new(Role::Assistant, note));
        return Settled::Kept;
    }
    let asked_last = session
        .messages()
        .last()
        .is_some_and(|message| message.role == Role::User);
    let question = if asked_last {
        session
            .pop()
            .map(|message| message.content)
            .unwrap_or_default()
    } else {
        String::new()
    };
    Settled::Withdrawn(question)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("a runtime with timers")
    }

    #[test]
    fn finished_work_is_returned() {
        let waited = runtime()
            .block_on(until_cancelled(async { 7 }, || Ok(false)))
            .expect("no terminal error");
        assert_eq!(waited, Waited::Done(7));
    }

    #[test]
    fn work_that_is_ready_at_once_is_never_interrupted() {
        let asked = Cell::new(0);
        let waited = runtime()
            .block_on(until_cancelled(async { 1 }, || {
                asked.set(asked.get() + 1);
                Ok(true)
            }))
            .expect("no terminal error");
        assert_eq!(waited, Waited::Done(1));
        assert_eq!(asked.get(), 0);
    }

    #[test]
    fn slow_work_outlives_a_check_that_keeps_saying_no() {
        let asked = Cell::new(0);
        let work = async {
            tokio::time::sleep(Duration::from_millis(150)).await;
            5
        };
        let waited = runtime()
            .block_on(until_cancelled(work, || {
                asked.set(asked.get() + 1);
                Ok(false)
            }))
            .expect("no terminal error");
        assert_eq!(waited, Waited::Done(5));
        assert!(asked.get() >= 2, "the check ran only {} times", asked.get());
    }

    #[test]
    fn a_cancel_drops_the_work() {
        struct Flag(Rc<Cell<bool>>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let guard = Flag(Rc::clone(&dropped));
        let work = async move {
            let _guard = guard;
            tokio::time::sleep(Duration::from_secs(60)).await;
        };
        let waited = runtime()
            .block_on(until_cancelled(work, || Ok(true)))
            .expect("no terminal error");
        assert_eq!(waited, Waited::Cancelled);
        assert!(dropped.get(), "the request was not dropped");
    }

    #[test]
    fn the_check_is_repeated_until_it_says_yes() {
        let asked = Cell::new(0u32);
        let work = std::future::pending::<()>();
        let waited = runtime()
            .block_on(until_cancelled(work, || {
                asked.set(asked.get() + 1);
                Ok(asked.get() >= 3)
            }))
            .expect("no terminal error");
        assert_eq!(waited, Waited::Cancelled);
        assert_eq!(asked.get(), 3);
    }

    #[test]
    fn an_error_from_the_check_ends_the_wait() {
        let work = std::future::pending::<()>();
        let error = runtime()
            .block_on(until_cancelled(work, || {
                Err(io::Error::other("terminal gone"))
            }))
            .expect_err("the check failed");
        assert_eq!(error.to_string(), "terminal gone");
    }

    fn asked_twice() -> Session {
        let mut session = Session::new();
        session.push(Message::new(Role::User, "first"));
        session.push(Message::new(Role::Assistant, "answer"));
        session.push(Message::new(Role::User, "second"));
        session
    }

    #[test]
    fn a_cancel_before_any_tool_takes_the_question_back() {
        let mut session = asked_twice();
        let settled = settle(&mut session, false, "note");
        assert_eq!(settled, Settled::Withdrawn("second".to_owned()));
        assert_eq!(session.len(), 2);
        let last = session.messages().last().expect("the earlier answer");
        assert_eq!(last.content, "answer");
    }

    #[test]
    fn a_cancel_after_tools_keeps_the_work_and_closes_the_turn() {
        let mut session = asked_twice();
        session.push(Message::new(Role::Assistant, "reading the file"));
        session.push(Message::tool_result("call-1", "read_file", "contents"));
        let settled = settle(&mut session, true, "stopped");
        assert_eq!(settled, Settled::Kept);
        assert_eq!(session.len(), 6);
        let last = session.messages().last().expect("the closing note");
        assert!(last.role == Role::Assistant);
        assert_eq!(last.content, "stopped");
    }

    #[test]
    fn nothing_is_taken_back_when_the_question_is_not_last() {
        let mut session = Session::new();
        session.push(Message::new(Role::User, "asked"));
        session.push(Message::new(Role::Assistant, "answered"));
        let settled = settle(&mut session, false, "note");
        assert_eq!(settled, Settled::Withdrawn(String::new()));
        assert_eq!(session.len(), 2);
    }
}
