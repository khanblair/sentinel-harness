//! Crash resume. A run that stopped without finishing keeps its tasks in the store. The next
//! start puts the tree back to the last checkpoint, sends interrupted tasks back to Ready,
//! checks the commits that were not yet verified, and carries on with the waves.

use std::sync::Arc;

use sentinel_core::{ApprovalState, ChatId, Event, HarnessState, TaskId, TaskStatus};
use sentinel_tree::{Checkpoint, Git, restore_path};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result, lock};
use crate::run_ctx::RunCtx;
use crate::wave::barrier_and_verify;

/// The setting that holds the start time of a run that has not finished. It is removed when the
/// run closes out, so a value left behind means the daemon stopped in the middle.
const LIVE_KEY: &str = "run.live";
/// The chat of the run that has not finished.
const CHAT_KEY: &str = "run.chat";

impl Engine {
    /// When the interrupted run started, or `None` when the last run finished or never planned.
    /// A run that stopped while planning has tasks still in `Planned` and starts over instead.
    pub(crate) fn interrupted_since(&self) -> Result<Option<OffsetDateTime>> {
        let Some(text) = self
            .store()
            .get_setting(LIVE_KEY)?
            .filter(|t| !t.is_empty())
        else {
            return Ok(None);
        };
        let Ok(started) = OffsetDateTime::parse(&text, &Rfc3339) else {
            return Ok(None);
        };
        let tasks = self.store().list_tasks()?;
        let planned = !tasks.is_empty() && tasks.iter().all(|t| t.status != TaskStatus::Planned);
        Ok(planned.then_some(started))
    }

    pub(crate) fn mark_run_live(&self, started: OffsetDateTime, chat: &ChatId) -> Result<()> {
        let text = started
            .format(&Rfc3339)
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        self.store().set_setting(LIVE_KEY, &text)?;
        self.store().set_setting(CHAT_KEY, chat.as_str())?;
        Ok(())
    }

    /// The chat of the interrupted run. A store from before chats has none, so a chat is made.
    pub(crate) fn interrupted_chat(&self) -> Result<ChatId> {
        let saved = self
            .store()
            .get_setting(CHAT_KEY)?
            .filter(|id| !id.is_empty());
        if let Some(id) = saved {
            let id = ChatId::new(id);
            if self.store().get_chat(&id)?.is_some() {
                return Ok(id);
            }
        }
        Ok(self
            .create_chat(Some("Earlier runs"), &Caller::pc("Sentinel"))?
            .id)
    }

    pub(crate) fn clear_run_live(&self) {
        if let Err(error) = self.store().set_setting(LIVE_KEY, "") {
            eprintln!("sentinel: could not clear the live run mark: {error}");
        }
    }

    /// After a start, a daemon that finds a live mark knows the last run was cut short.
    pub(crate) fn note_crash_on_open(&self) {
        if self.interrupted_since().ok().flatten().is_some() {
            lock(&self.inner.live).state = HarnessState::Crashed;
        }
    }
}

/// The git handle for a resumed run: every stray change goes back to HEAD, then the tree must be
/// clean.
pub(crate) fn restored_git(engine: &Engine) -> Result<Git> {
    let repo = engine.repo_path();
    let invalid = |e: sentinel_tree::TreeError| EngineError::Invalid(e.to_string());
    let git = Git::new(&repo, repo.join(".sentinel/index")).map_err(invalid)?;
    git.reset_private_index().map_err(invalid)?;
    for path in git.changed_paths().map_err(invalid)? {
        restore_path(&git, &path, "HEAD").map_err(invalid)?;
    }
    if !git.is_clean().map_err(invalid)? {
        return Err(EngineError::Refused(
            "The working tree still differs from the last checkpoint. Commit or stash the changes, then resume."
                .to_owned(),
        ));
    }
    Ok(git)
}

/// Get a resumed run ready for its next wave and return that wave's number.
pub(crate) async fn resume_phase(ctx: &Arc<RunCtx>) -> Result<u32> {
    let engine = &ctx.engine;
    default_open_cards(ctx)?;
    let requeued = lock(&ctx.scheduler)
        .recover_after_crash(engine.now())
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    requeue_failed(ctx)?;
    let unverified = reload_checkpoints(ctx)?;
    say(
        ctx,
        &format!(
            "Resuming after a stop: {} task(s) go back for another try, {} commit(s) are checked first.",
            requeued.len(),
            unverified.len()
        ),
    );
    engine.set_state(HarnessState::Running);
    let mut wave = last_wave(ctx)? + 1;
    if !unverified.is_empty() {
        barrier_and_verify(ctx, wave, &unverified).await?;
        wave += 1;
    }
    Ok(wave)
}

fn say(ctx: &RunCtx, text: &str) {
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: text.to_owned(),
    });
}

/// Cards nobody can answer any more get the safe default, the same as a timeout.
fn default_open_cards(ctx: &RunCtx) -> Result<()> {
    let engine = &ctx.engine;
    for approval in engine.store().list_approvals(&[ApprovalState::Pending])? {
        engine.store().answer_approval(
            &approval.id,
            ApprovalState::Defaulted,
            "default",
            engine.now(),
        )?;
        engine.emit(&Event::ApprovalResolved {
            approval: approval.id,
            state: ApprovalState::Defaulted,
            decided_by: "default".to_owned(),
        })?;
    }
    Ok(())
}

/// A task that was failing when the run stopped gets its next try.
fn requeue_failed(ctx: &RunCtx) -> Result<()> {
    for task in ctx.engine.store().list_tasks()? {
        if task.status == TaskStatus::Failed {
            ctx.engine
                .store()
                .set_task_status(&task.id, TaskStatus::Ready)?;
        }
    }
    Ok(())
}

/// Tasks that were committed but not yet verified: put their commits back in memory.
fn reload_checkpoints(ctx: &RunCtx) -> Result<Vec<TaskId>> {
    let store = ctx.engine.store();
    let mut pending = Vec::new();
    for record in store.list_checkpoints()? {
        let waiting = store.get_task(&record.task)?.is_some_and(|task| {
            matches!(
                task.status,
                TaskStatus::Checkpointed | TaskStatus::Verifying
            )
        });
        if !waiting {
            continue;
        }
        lock(&ctx.memory).checkpoints.insert(
            record.task.clone(),
            Checkpoint {
                commit: record.commit,
                paths: record.paths,
            },
        );
        if !pending.contains(&record.task) {
            pending.push(record.task);
        }
    }
    Ok(pending)
}

fn last_wave(ctx: &RunCtx) -> Result<u32> {
    Ok(ctx
        .engine
        .store()
        .list_barriers()?
        .iter()
        .map(|barrier| barrier.wave)
        .max()
        .unwrap_or(0))
}
