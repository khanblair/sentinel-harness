//! Contradiction detection and decision handling.

use crate::Plan;
use sentinel_core::{Decision, DecisionId, DecisionState, TaskId};

/// A pair of contradicting tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContradictionPair {
    pub a: TaskId,
    pub b: TaskId,
}

/// Find all contradiction pairs in a plan.
/// Each unordered pair appears once, sorted by task id.
pub fn find_contradictions(plan: &Plan) -> Vec<ContradictionPair> {
    let mut seen = std::collections::HashSet::new();
    let mut pairs: Vec<ContradictionPair> = plan
        .tasks
        .iter()
        .flat_map(|task| {
            task.contradicts
                .iter()
                .map(move |other| ordered_pair(&task.id, other))
        })
        .filter(|pair| seen.insert((pair.a.clone(), pair.b.clone())))
        .collect();

    // Sort pairs by first then second task id
    pairs.sort_by(|p1, p2| p1.a.cmp(&p2.a).then_with(|| p1.b.cmp(&p2.b)));

    pairs
}

/// Put the smaller id first, so each unordered pair has one spelling.
fn ordered_pair(a: &TaskId, b: &TaskId) -> ContradictionPair {
    let (first, second) = if a.as_str() < b.as_str() {
        (a, b)
    } else {
        (b, a)
    };
    ContradictionPair {
        a: first.clone(),
        b: second.clone(),
    }
}

/// Create a Decision for a contradiction pair.
/// The prompt names both tasks and the tasks field contains both ids.
/// Decision id format: D-<a>-<b>
pub fn decision_for(pair: &ContradictionPair, titles: &dyn Fn(&TaskId) -> String) -> Decision {
    let id = DecisionId::new(format!("D-{}-{}", pair.a, pair.b));
    let title_a = titles(&pair.a);
    let title_b = titles(&pair.b);
    let prompt = format!(
        "Two tasks contradict each other and cannot both be done:\n\n\
         - {0}: {title_a}\n\
         - {1}: {title_b}\n\n\
         Reply with the task id to keep (e.g. \"{0}\"), \"skip_both\" to skip both, or nothing to default to skipping both.",
        pair.a, pair.b
    );

    Decision {
        id,
        prompt,
        tasks: vec![pair.a.clone(), pair.b.clone()],
        state: DecisionState::Pending,
        answer: None,
    }
}

/// Resolution of a contradiction decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Keep { chosen: TaskId, skipped: TaskId },
    SkipBoth { a: TaskId, b: TaskId },
}

/// Apply an answer to a decision and return the resolution.
/// `answer`: task id of the pair (keep that one), `skip_both`, or `None` (skip both).
pub fn apply_answer(decision: &Decision, answer: Option<&str>) -> Resolution {
    let task_a = &decision.tasks[0];
    let task_b = &decision.tasks[1];

    // "skip_both", no answer, and any unknown id all skip both tasks
    if answer == Some(task_a.as_str()) {
        return Resolution::Keep {
            chosen: task_a.clone(),
            skipped: task_b.clone(),
        };
    }
    if answer == Some(task_b.as_str()) {
        return Resolution::Keep {
            chosen: task_b.clone(),
            skipped: task_a.clone(),
        };
    }
    Resolution::SkipBoth {
        a: task_a.clone(),
        b: task_b.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::PlannedTask;
    use sentinel_core::Tier;

    fn make_plan(tasks: Vec<PlannedTask>) -> Plan {
        Plan { tasks }
    }

    fn make_pt(id: &str, contradicts: Vec<&str>) -> PlannedTask {
        PlannedTask {
            id: TaskId::new(id),
            scope: vec![],
            depends_on: vec![],
            read_after_write: vec![],
            tier: Tier::Cheap,
            already_done: false,
            contradicts: contradicts.into_iter().map(TaskId::new).collect(),
        }
    }

    #[test]
    fn find_contradictions_returns_each_pair_once_sorted() {
        let plan = make_plan(vec![
            make_pt("T-01", vec!["T-02", "T-03"]),
            make_pt("T-02", vec!["T-01"]),
            make_pt("T-03", vec!["T-01"]),
            make_pt("T-04", vec![]),
        ]);
        let pairs = find_contradictions(&plan);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].a.as_str(), "T-01");
        assert_eq!(pairs[0].b.as_str(), "T-02");
        assert_eq!(pairs[1].a.as_str(), "T-01");
        assert_eq!(pairs[1].b.as_str(), "T-03");
    }

    #[test]
    fn decision_for_prompt_names_both_tasks() {
        let pair = ContradictionPair {
            a: TaskId::new("T-01"),
            b: TaskId::new("T-02"),
        };
        let decision = decision_for(&pair, &|id| format!("Title for {id}"));
        assert_eq!(decision.id.as_str(), "D-T-01-T-02");
        assert!(decision.prompt.contains("T-01"));
        assert!(decision.prompt.contains("T-02"));
        assert!(decision.prompt.contains("Title for T-01"));
        assert!(decision.prompt.contains("Title for T-02"));
        assert_eq!(decision.tasks.len(), 2);
        assert_eq!(decision.state, DecisionState::Pending);
        assert!(decision.answer.is_none());
    }

    #[test]
    fn apply_answer_keeps_chosen_skips_other() {
        let pair = ContradictionPair {
            a: TaskId::new("T-01"),
            b: TaskId::new("T-02"),
        };
        let decision = decision_for(&pair, &|id| format!("Title for {id}"));

        let res = apply_answer(&decision, Some("T-01"));
        assert!(
            matches!(res, Resolution::Keep { chosen, skipped } if chosen.as_str() == "T-01" && skipped.as_str() == "T-02")
        );

        let res = apply_answer(&decision, Some("T-02"));
        assert!(
            matches!(res, Resolution::Keep { chosen, skipped } if chosen.as_str() == "T-02" && skipped.as_str() == "T-01")
        );
    }

    #[test]
    fn apply_answer_skip_both() {
        let pair = ContradictionPair {
            a: TaskId::new("T-01"),
            b: TaskId::new("T-02"),
        };
        let decision = decision_for(&pair, &|id| format!("Title for {id}"));

        let res = apply_answer(&decision, Some("skip_both"));
        assert!(
            matches!(res, Resolution::SkipBoth { a, b } if a.as_str() == "T-01" && b.as_str() == "T-02")
        );
    }

    #[test]
    fn apply_answer_none_defaults_to_skip_both() {
        let pair = ContradictionPair {
            a: TaskId::new("T-01"),
            b: TaskId::new("T-02"),
        };
        let decision = decision_for(&pair, &|id| format!("Title for {id}"));

        let res = apply_answer(&decision, None);
        assert!(
            matches!(res, Resolution::SkipBoth { a, b } if a.as_str() == "T-01" && b.as_str() == "T-02")
        );

        let res = apply_answer(&decision, Some("invalid"));
        assert!(
            matches!(res, Resolution::SkipBoth { a, b } if a.as_str() == "T-01" && b.as_str() == "T-02")
        );
    }
}
