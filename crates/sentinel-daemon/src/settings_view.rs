//! The config values the UI shows in Settings.

use crate::config::Config;

pub(crate) fn settings_for_ui(config: &Config) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    map.insert("test_command".to_owned(), config.test_command.join(" "));
    map.insert("backlog".to_owned(), config.backlog.clone());
    map.insert("max_parallel".to_owned(), config.max_parallel.to_string());
    map.insert(
        "task_timeout_seconds".to_owned(),
        config.task_timeout_seconds.to_string(),
    );
    map.insert("telegram_detail".to_owned(), config.telegram.detail.clone());
    map
}
