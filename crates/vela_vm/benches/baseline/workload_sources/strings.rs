pub(crate) const STRING_METHODS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..80 {
        let label = "quest.done";
        let upper = label.to_upper();
        let replaced = label.replace(".", "_");
        let repeated = "ab".repeat(3);
        let parts = "alpha,beta".split(",");
        let pair = "count=3".split_once("=").unwrap_or([]);
        let lines = "alpha\nbeta".split_lines();
        let words = "alpha beta".split_whitespace();
        let sliced = "hello".slice(1, 4);
        let ch = '\0';
        let ch_index = 0;
        for candidate in "quest" {
            if ch_index == 1 {
                ch = candidate;
            }
            ch_index += 1;
        }
        let found = "daily_quest".find("quest").unwrap_or(-1);
        let stripped_prefix = "event:quest".strip_prefix("event:").unwrap_or("");
        let stripped_suffix = "quest.done".strip_suffix(".done").unwrap_or("");
        let parsed = "42".parse_i64().unwrap_or(0);
        let parsed_bool = "true".parse_bool().unwrap_or(false);

        if upper != "QUEST.DONE"
            || replaced != "quest_done"
            || repeated != "ababab"
            || parts.len() != 2
            || pair.len() != 2
            || lines.len() != 2
            || words.len() != 2
            || sliced != "ell"
            || ch != 'u'
            || found != 6
            || stripped_prefix != "quest"
            || stripped_suffix != "quest"
            || parsed != 42
            || !parsed_bool
        {
            return 0;
        }

        total += upper.len()
            + replaced.len()
            + repeated.len()
            + parts.join("").len()
            + pair.join("").len()
            + lines.join("").len()
            + words.join("").len()
            + sliced.len()
            + if ch == 'u' { 1 } else { 0 }
            + found
            + parsed
            + tick - tick;
    }
    return total;
}
"#;

pub(crate) const STRING_TRANSFORMS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let lower = "ALPHA.beta".to_lower();
        let upper = "alpha.beta".to_upper();
        let trimmed = "  signal.ready  ".trim();
        let trimmed_start = "  signal.ready".trim_start();
        let trimmed_end = "signal.ready  ".trim_end();

        if lower != "alpha.beta"
            || upper != "ALPHA.BETA"
            || trimmed != "signal.ready"
            || trimmed_start != "signal.ready"
            || trimmed_end != "signal.ready"
        {
            return 0;
        }

        total += lower.len()
            + upper.len()
            + trimmed.len()
            + trimmed_start.len()
            + trimmed_end.len()
            + tick - tick;
    }
    return total;
}
"#;

pub(crate) const STRING_SPLITTING_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let parts = "alpha,beta,gamma,delta".split(",");
        let pair = "count=42".split_once("=").unwrap_or(["", ""]);
        let lines = "alpha\nbeta\r\ngamma".split_lines();
        let words = " alpha\tbeta\ngamma ".split_whitespace();

        if parts.len() != 4
            || parts[2] != "gamma"
            || pair[0] != "count"
            || pair[1] != "42"
            || lines.len() != 3
            || words.join("|") != "alpha|beta|gamma"
        {
            return 0;
        }

        total += parts.join("").len()
            + pair.join("").len()
            + lines.join("").len()
            + words.join("").len()
            + tick - tick;
    }
    return total;
}
"#;

pub(crate) const STRING_PARSING_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let parsed = "42".parse_i64().unwrap_or(0);
        let negative = "-7".parse_i64().unwrap_or(0);
        let rate = "1.5".parse_f64().unwrap_or(0.0);
        let exponent = "2.5e1".parse_f64().unwrap_or(0.0);
        let enabled = "true".parse_bool().unwrap_or(false);
        let disabled = "false".parse_bool().unwrap_or(true);

        if parsed != 42
            || negative != -7
            || rate != 1.5
            || exponent != 25.0
            || !enabled
            || disabled
            || !option::is_none("bad".parse_i64())
            || !option::is_none("yes".parse_bool())
        {
            return 0;
        }
        total += parsed + negative + tick - tick;
    }
    return total;
}
"#;

pub(crate) const STRING_OPTIONS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let text = "event:alpha.done";
        let found = text.find("alpha").unwrap_or(-1);
        let ch = '\0';
        let ch_index = 0;
        for candidate in text {
            if ch_index == 6 {
                ch = candidate;
            }
            ch_index += 1;
        }
        let prefix = text.strip_prefix("event:").unwrap_or("");
        let suffix = text.strip_suffix(".done").unwrap_or("");

        if found != 6
            || ch != 'a'
            || prefix != "alpha.done"
            || suffix != "event:alpha"
            || !option::is_none(text.find("missing"))
            || !option::is_none(text.strip_prefix("wrong:"))
            || !option::is_none(text.strip_suffix(".miss"))
        {
            return 0;
        }
        total += found + prefix.len() + suffix.len() + if ch == 'a' { 1 } else { 0 } + tick - tick;
    }
    return total;
}
"#;
