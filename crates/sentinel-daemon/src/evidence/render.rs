//! The evidence as Markdown. The builder in `evidence.rs` makes the data, this file shows it.

use sentinel_core::Cost;

use super::{
    AgentEvidence, ClaimRow, Collisions, CostEvidence, Evidence, Honesty, LISTED_PATHS,
    WaveEvidence,
};

/// The evidence as Markdown, to follow the report. Plain words, no emoji, no em dashes.
pub fn render_evidence(evidence: &Evidence) -> String {
    let mut out = String::from("\n## Evidence\n\n");
    push_agents(&mut out, &evidence.agents);
    push_waves(&mut out, &evidence.waves);
    push_collisions(&mut out, &evidence.collisions);
    push_cost(&mut out, &evidence.cost);
    push_claims(&mut out, &evidence.claims);
    push_honesty(&mut out, &evidence.honesty);
    out
}

fn push_agents(out: &mut String, agents: &[AgentEvidence]) {
    out.push_str("### Agents\n\n");
    if agents.is_empty() {
        out.push_str("No agent started a task.\n\n");
        return;
    }
    out.push_str("| Agent | Tasks started | Verified | Spend | Spend source |\n");
    out.push_str("|-------|---------------|----------|-------|--------------|\n");
    for row in agents {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.agent,
            row.tasks_started,
            row.tasks_verified,
            money(row.spend),
            row.spend_source
        ));
    }
    out.push('\n');
}

fn push_waves(out: &mut String, waves: &WaveEvidence) {
    out.push_str(&format!(
        "Waves: {}. Most tasks in one wave: {}. Parallel share: {:.2} (tasks running at once, on average).\n\n",
        waves.count, waves.most_tasks, waves.parallel_share
    ));
}

fn push_collisions(out: &mut String, collisions: &Collisions) {
    out.push_str("### Collisions\n\n");
    out.push_str("| Check | Count |\n|-------|-------|\n");
    out.push_str(&format!("| Writes blocked | {} |\n", collisions.writes_blocked));
    out.push_str(&format!("| Writes restored | {} |\n", collisions.writes_restored));
    out.push_str(&format!("| Tampers blocked | {} |\n", collisions.tampers_blocked));
    out.push('\n');
    if !collisions.blocked_paths.is_empty() {
        out.push_str(&format!(
            "Blocked paths (first {LISTED_PATHS}): {}\n\n",
            collisions.blocked_paths.join(", ")
        ));
    }
}

fn push_cost(out: &mut String, cost: &CostEvidence) {
    out.push_str("### Cost\n\n");
    out.push_str("| Spent | Cap | Used | Tasks verified | Per verified task |\n");
    out.push_str("|-------|-----|------|----------------|-------------------|\n");
    let used = cost
        .percent_of_cap
        .map_or_else(|| "no cap".to_owned(), |p| format!("{p}%"));
    let per_verified = cost
        .per_verified
        .map_or_else(|| "none".to_owned(), money);
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n",
        money(cost.spent),
        money(cost.cap),
        used,
        cost.tasks_verified,
        per_verified
    ));
    out.push('\n');
    let threshold = cost
        .highest_threshold
        .map_or_else(|| "none reached".to_owned(), |p| format!("{p}%"));
    out.push_str(&format!("Highest budget threshold: {threshold}.\n\n"));
}

fn push_claims(out: &mut String, claims: &[ClaimRow]) {
    out.push_str("### Claims\n\n");
    out.push_str(
        "A finished task is the agent's claim of done. The check columns are what the harness \
         found. \"not checked\" means no check was stored.\n\n",
    );
    if claims.is_empty() {
        out.push_str("No tasks.\n\n");
        return;
    }
    out.push_str(
        "| Task | Finished | Tests pass | New test | Reviewer ok | Tamper clear | Status | Reason |\n",
    );
    out.push_str(
        "|------|----------|------------|----------|-------------|--------------|--------|--------|\n",
    );
    for claim in claims {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            claim.task,
            yes_no(Some(claim.finished)),
            yes_no(claim.tests_pass),
            yes_no(claim.new_test),
            yes_no(claim.reviewer_ok),
            yes_no(claim.tamper_clear),
            claim.status.label(),
            claim.reason.replace('|', "\\|")
        ));
    }
    out.push('\n');
}

fn push_honesty(out: &mut String, honesty: &Honesty) {
    out.push_str("### Honesty\n\n");
    out.push_str(&format!(
        "{} Verified, {} Partial, {} Blocked, {} Skipped, {} Failed.\n\n",
        honesty.verified, honesty.partial, honesty.blocked, honesty.skipped, honesty.failed
    ));
    out.push_str(&format!(
        "Claims not confirmed: {}. These tasks finished but were not Verified.\n\n",
        honesty.claims_not_confirmed
    ));
}

fn yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "not checked",
    }
}

/// Dollars as the report shows them, with `est` when Sentinel guessed the figure.
fn money(cost: Cost) -> String {
    if cost.is_estimated() {
        format!("{cost} est")
    } else {
        cost.to_string()
    }
}
