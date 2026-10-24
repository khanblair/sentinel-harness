//! Engine methods that change settings and answer questions. Each one says who did it, because a
//! phone or a Telegram chat may do less than the computer (`docs/mobile.md`, Security).

use sentinel_core::api::PermissionsUpdate;
use sentinel_core::{
    Approval, ApprovalId, ApprovalState, BrainMode, Capability, Cost, Decision, DecisionId,
    DecisionState, Event, PermissionMode, Routing, ToolId,
};

use crate::engine::{Engine, EngineError, Result, lock};

/// Who is asking. The computer may do everything. A phone or Telegram may not raise risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub name: String,
    pub from_pc: bool,
}

impl Caller {
    pub fn pc(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            from_pc: true,
        }
    }

    pub fn phone(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            from_pc: false,
        }
    }
}

const ALL_CAPABILITIES: [Capability; 6] = [
    Capability::Read,
    Capability::Write,
    Capability::Ext,
    Capability::Tests,
    Capability::Shell,
    Capability::Net,
];

impl Engine {
    /// Lower the budget cap. There is no way to raise it, from any caller.
    pub fn lower_cap(&self, text: &str, caller: &Caller) -> Result<Cost> {
        let new_cap = Cost::parse_usd(text).map_err(|e| EngineError::Invalid(e.to_string()))?;
        let config = self.config();
        let current = self.cap(&config);
        if new_cap > current {
            return Err(EngineError::Refused(format!(
                "The cap can only be lowered. {new_cap} is above the current cap {current}."
            )));
        }
        self.update_config(|c| c.cap_usd = usd_text(new_cap))?;
        if let Some(budget) = lock(&self.inner.live).budget.clone() {
            budget
                .lower_cap(new_cap)
                .map_err(|e| EngineError::Refused(e.to_string()))?;
        }
        let total = self.total_cost(&self.live_summary().spent)?;
        self.emit(Event::CostUpdate {
            total,
            cap: new_cap,
        })?;
        self.emit(Event::DeviceAction {
            device: sentinel_core::DeviceId::new(&caller.name),
            action: format!("lowered the cap to {new_cap}"),
        })?;
        Ok(new_cap)
    }

    pub fn set_permission_mode(&self, mode: PermissionMode, caller: &Caller) -> Result<()> {
        if mode == PermissionMode::Bypass && !caller.from_pc {
            return Err(EngineError::Refused(
                "Bypass permissions can only be switched on at the computer.".to_owned(),
            ));
        }
        self.update_config(|c| c.permission_mode = mode)?;
        self.emit(Event::PermissionChanged { mode })?;
        Ok(())
    }

    pub fn update_permissions(&self, update: &PermissionsUpdate, caller: &Caller) -> Result<()> {
        if let Some(mode) = update.mode {
            self.set_permission_mode(mode, caller)?;
        }
        for row in update.capabilities.iter().flatten() {
            for capability in ALL_CAPABILITIES {
                let allowed = row.allowed.contains(&capability);
                self.inner
                    .store
                    .set_capability(&row.agent, capability, allowed)?;
            }
        }
        if update.capabilities.is_some() {
            self.emit(Event::PermissionChanged {
                mode: self.config().permission_mode,
            })?;
        }
        Ok(())
    }

    pub fn set_routing(&self, routing: Routing) -> Result<()> {
        self.update_config(|c| c.routing = routing)?;
        Ok(())
    }

    pub fn toggle_tool(&self, id: &ToolId, enabled: bool) -> Result<()> {
        self.inner.store.set_tool_enabled(id, enabled)?;
        self.update_config(|c| {
            c.enabled_tools.retain(|t| t != id);
            if enabled {
                c.enabled_tools.push(id.clone());
            }
        })?;
        if let Some(tool) = self
            .inner
            .store
            .list_tools()?
            .into_iter()
            .find(|t| &t.id == id)
        {
            self.emit(Event::ToolStatusChanged {
                tool: tool.id,
                status: tool.status,
            })?;
        }
        Ok(())
    }

    /// The Brain mode. A phone may only move toward `Off`. The live-run rule keeps `Learn` out.
    pub fn set_brain_mode(&self, mode: BrainMode, caller: &Caller) -> Result<BrainMode> {
        let current = self.config().brain_mode;
        if !caller.from_pc && !current.phone_may_set(mode) {
            return Err(EngineError::Refused(
                "A phone can only make the Brain stricter. Loosening it needs the computer."
                    .to_owned(),
            ));
        }
        if self.config().hackathon_mode && mode == BrainMode::Learn {
            return Err(EngineError::Refused(
                "Learn mode is off while the live-run privacy rules apply.".to_owned(),
            ));
        }
        self.update_config(|c| c.brain_mode = mode)?;
        Ok(mode)
    }

    /// Answer a decision, such as two contradicting tasks. `answer` is a task id or `skip_both`.
    pub fn answer_decision(
        &self,
        id: &DecisionId,
        answer: &str,
        caller: &Caller,
    ) -> Result<Decision> {
        let pending = self.inner.store.list_decisions(&[DecisionState::Pending])?;
        let decision = pending
            .into_iter()
            .find(|d| &d.id == id)
            .ok_or_else(|| EngineError::NotFound(format!("pending decision {id}")))?;
        let valid = answer == "skip_both" || decision.tasks.iter().any(|t| t.as_str() == answer);
        if !valid {
            return Err(EngineError::Invalid(format!(
                "{answer} is not one of the choices"
            )));
        }
        let saved = self.inner.store.answer_decision(
            id,
            DecisionState::Answered,
            Some(answer),
            "human",
            self.now(),
        )?;
        if let Some(waiter) = lock(&self.inner.live).decisions.remove(id) {
            // The run loop may have moved on already: a closed channel is fine.
            let _ = waiter.send(answer.to_owned());
        }
        self.emit(Event::DeviceAction {
            device: sentinel_core::DeviceId::new(&caller.name),
            action: format!("answered {id}"),
        })?;
        Ok(saved)
    }

    /// Allow or deny an approval card.
    pub fn answer_approval(
        &self,
        id: &ApprovalId,
        allow: bool,
        caller: &Caller,
    ) -> Result<Approval> {
        let state = if allow {
            ApprovalState::Allowed
        } else {
            ApprovalState::Denied
        };
        let saved = self
            .inner
            .store
            .answer_approval(id, state, &caller.name, self.now())?;
        if let Some(waiter) = lock(&self.inner.live).approvals.remove(id) {
            let _ = waiter.send(allow);
        }
        self.emit(Event::ApprovalResolved {
            approval: id.clone(),
            state,
            decided_by: caller.name.clone(),
        })?;
        Ok(saved)
    }

    /// Put text in the stream, to Sentinel or to one agent. A message never changes which files a
    /// task has reserved.
    pub fn send_message(&self, text: &str, to: Option<ToolId>, caller: &Caller) -> Result<()> {
        if text.trim().is_empty() {
            return Err(EngineError::Invalid("The message is empty.".to_owned()));
        }
        self.emit(Event::Message {
            from: caller.name.clone(),
            to,
            text: text.trim().to_owned(),
        })?;
        Ok(())
    }
}

/// `1.5` dollars as `1.50`, `0.009` as `0.009`: at least two decimals, no trailing zeros past that.
pub(crate) fn usd_text(cost: Cost) -> String {
    let micro = cost.micro_usd();
    let whole = micro / 1_000_000;
    let mut fraction = format!("{:06}", micro % 1_000_000);
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!("{whole}.{fraction}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usd_text_keeps_two_decimals_and_drops_trailing_zeros() {
        assert_eq!(usd_text(Cost::from_micro_usd(1_500_000, false)), "1.50");
        assert_eq!(usd_text(Cost::from_micro_usd(9_000, false)), "0.009");
        assert_eq!(usd_text(Cost::from_micro_usd(2_000_000, false)), "2.00");
        assert_eq!(usd_text(Cost::from_micro_usd(1_234_567, false)), "1.234567");
    }

    #[test]
    fn usd_text_round_trips_through_parse() {
        for micro in [0, 1, 10_000, 1_500_000, 7_654_321] {
            let cost = Cost::from_micro_usd(micro, false);
            assert_eq!(Cost::parse_usd(&usd_text(cost)).unwrap().micro_usd(), micro);
        }
    }
}
