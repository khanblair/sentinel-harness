//! The wave summary. `WaveTracker` groups the verdicts of one wave into one message.

use std::collections::BTreeMap;

use sentinel_core::{Event, TaskId, TaskStatus};

use super::{ButtonSpec, Card, RenderContext, Rendered, button, cost_lines, detail_of, finish};
use crate::buttons::ButtonAction;
use crate::levels::{Action, Detail, EventKind, Level, allowed};
use crate::quiet::Priority;

/// Verifier reasons add at most this many lines at `Detail`.
const MAX_DETAIL_LINES: usize = 6;
const MAX_DETAIL_LINE_CHARS: usize = 160;

/// One message of the wave summary: the verdict of one task and the files it checkpointed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveLine {
    pub task: TaskId,
    pub status: TaskStatus,
    pub files: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveSummary {
    pub wave: u32,
    pub lines: Vec<WaveLine>,
}

impl WaveSummary {
    pub fn verified(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.status == TaskStatus::Verified)
            .count()
    }

    pub fn total(&self) -> usize {
        self.lines.len()
    }
}

/// Groups the verdicts of one wave. `observe` returns the summary once every task of the wave
/// has a verdict.
#[derive(Debug, Default)]
pub struct WaveTracker {
    wave: u32,
    expected: Vec<TaskId>,
    verdicts: BTreeMap<TaskId, Verdict>,
    files: BTreeMap<TaskId, Vec<String>>,
}

#[derive(Debug)]
struct Verdict {
    status: TaskStatus,
    reason: String,
}

impl WaveTracker {
    pub fn observe(&mut self, event: &Event) -> Option<WaveSummary> {
        if let Event::WaveStarted { wave, tasks } = event {
            self.start(*wave, tasks);
        }
        if let Event::Checkpoint { task, paths, .. } = event {
            self.files.insert(task.clone(), paths.clone());
        }
        if let Event::Verdict {
            task,
            status,
            reason,
        } = event
        {
            return self.record(task, *status, reason);
        }
        None
    }

    fn start(&mut self, wave: u32, tasks: &[TaskId]) {
        self.wave = wave;
        self.expected = tasks.to_vec();
        self.verdicts.clear();
        self.files.clear();
    }

    fn record(&mut self, task: &TaskId, status: TaskStatus, reason: &str) -> Option<WaveSummary> {
        if !self.expected.contains(task) {
            return None;
        }
        let verdict = Verdict {
            status,
            reason: reason.to_owned(),
        };
        self.verdicts.insert(task.clone(), verdict);
        if self.verdicts.len() < self.expected.len() {
            return None;
        }
        let summary = self.summary();
        self.expected.clear();
        Some(summary)
    }

    fn summary(&self) -> WaveSummary {
        let lines = self
            .expected
            .iter()
            .filter_map(|task| {
                let verdict = self.verdicts.get(task)?;
                Some(WaveLine {
                    task: task.clone(),
                    status: verdict.status,
                    files: self.files.get(task).cloned().unwrap_or_default(),
                    reason: verdict.reason.clone(),
                })
            })
            .collect();
        WaveSummary {
            wave: self.wave,
            lines,
        }
    }
}

/// The wave summary from `WaveTracker::observe`.
pub fn render_wave(summary: &WaveSummary, ctx: &RenderContext<'_>) -> Option<Rendered> {
    let allowed_here = ctx.settings.levels.contains(&Level::Notify)
        && ctx.settings.events.contains(&EventKind::WaveFinished);
    if !allowed_here {
        return None;
    }
    let detail = detail_of(ctx);
    let mut lines = vec![format!(
        "Wave {} done. {} of {} verified.",
        summary.wave,
        summary.verified(),
        summary.total()
    )];
    lines.extend(summary.lines.iter().map(|line| task_line(line, detail)));
    lines.extend(detail_lines(summary, detail));
    lines.extend(cost_lines(ctx));
    let mut card = Card::new(lines, Priority::Normal);
    card.push_row(wave_buttons(ctx));
    Some(finish(card, ctx))
}

fn wave_buttons(ctx: &RenderContext<'_>) -> Vec<ButtonSpec> {
    let mut row = Vec::new();
    if allowed(&ctx.settings.levels, Action::Pause) {
        row.push(button("Pause", ButtonAction::Pause));
    }
    if allowed(&ctx.settings.levels, Action::DiffSummary) {
        let action = ButtonAction::DiffSummary { task: None };
        row.push(button("Diff summary", action));
    }
    row
}

fn task_line(line: &WaveLine, detail: Detail) -> String {
    let base = format!("{} {}", line.task, line.status.label());
    if detail == Detail::Status || line.files.is_empty() {
        return base;
    }
    format!("{base}: {}", line.files.join(", "))
}

/// At `Detail`, the verifier's reasons add up to six short lines.
fn detail_lines(summary: &WaveSummary, detail: Detail) -> Vec<String> {
    if detail != Detail::Detail {
        return Vec::new();
    }
    summary
        .lines
        .iter()
        .flat_map(|line| line.reason.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(MAX_DETAIL_LINES)
        .map(|line| format!("  {}", short(line, MAX_DETAIL_LINE_CHARS)))
        .collect()
}

fn short(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_owned();
    }
    let kept: String = line.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{kept}...")
}
