use crate::store::Store;

pub fn format_summary(store: &Store) -> String {
    let today = store.today();
    let total = today.total_tokens();
    format!(
        "today: {} tokens across {} sessions ({}in {}out {}cached)\n",
        compact(total),
        today.session_count,
        compact(today.input_tokens),
        compact(today.output_tokens),
        compact(today.cache_creation_tokens + today.cache_read_tokens),
    )
}

pub fn format_daily(store: &Store, days: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<12} {:>10} {:>10} {:>12} {:>8}\n",
        "Date", "Input", "Output", "Cache", "Sessions"
    ));
    out.push_str(&"-".repeat(56));
    out.push('\n');
    for day in store.by_day(days) {
        out.push_str(&format!(
            "{:<12} {:>10} {:>10} {:>12} {:>8}\n",
            day.date,
            compact(day.input_tokens),
            compact(day.output_tokens),
            compact(day.cache_creation_tokens + day.cache_read_tokens),
            day.session_count,
        ));
    }
    out
}

pub fn format_projects(store: &Store) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<28} {:>12} {:>10} {:>12}\n",
        "Project", "Tokens", "Sessions", "Last Used"
    ));
    out.push_str(&"-".repeat(66));
    out.push('\n');
    for p in store.by_project() {
        let total =
            p.input_tokens + p.output_tokens + p.cache_creation_tokens + p.cache_read_tokens;
        let ago = format_ago(p.last_used);
        out.push_str(&format!(
            "{:<28} {:>12} {:>10} {:>12}\n",
            p.name,
            compact(total),
            p.session_count,
            ago,
        ));
    }
    out
}

pub fn format_sessions(_store: &Store) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<28} {:>12} {:>10}\n",
        "Session", "Tokens", "Records"
    ));
    out.push_str(&"-".repeat(52));
    out.push('\n');
    out.push_str("(session detail coming soon)\n");
    out
}

pub fn compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_ago(time: chrono::DateTime<chrono::Utc>) -> String {
    let diff = chrono::Utc::now() - time;
    if diff.num_days() > 0 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}m ago", diff.num_minutes().max(1))
    }
}
