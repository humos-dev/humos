//! Regex-based secret redaction applied before content enters the index.
//!
//! This is NOT a complete solution. It catches common token shapes that
//! routinely show up in terminal session logs (cat .env, curl headers, etc).
//! A motivated attacker with access to the index file on disk can still see
//! raw session JSONLs by reading the original files. The purpose is to stop
//! an MCP client from trivially calling search_sessions("sk-") and
//! exfiltrating credentials.
//!
//! Set `HUMOS_INDEX_REDACT=off` to disable (useful for debugging).

use regex::Regex;

pub struct Redactor {
    patterns: Vec<(Regex, &'static str)>,
}

impl Redactor {
    pub fn new() -> Self {
        let specs: Vec<(&str, &'static str)> = vec![
            (r"sk-proj-[A-Za-z0-9_\-]{20,}", "[REDACTED:openai]"),
            (r"sk-ant-[A-Za-z0-9_\-]{20,}", "[REDACTED:anthropic]"),
            (r"sk-[A-Za-z0-9_\-]{20,}", "[REDACTED:llm-key]"),
            (r"AKIA[0-9A-Z]{16}", "[REDACTED:aws]"),
            (r"ASIA[0-9A-Z]{16}", "[REDACTED:aws-session]"),
            (r"ghp_[A-Za-z0-9]{30,}", "[REDACTED:github]"),
            (r"ghs_[A-Za-z0-9]{30,}", "[REDACTED:github]"),
            (r"gho_[A-Za-z0-9]{30,}", "[REDACTED:github]"),
            (r"ghu_[A-Za-z0-9]{30,}", "[REDACTED:github]"),
            (r"ghr_[A-Za-z0-9]{30,}", "[REDACTED:github]"),
            (r"xox[baprs]-[A-Za-z0-9\-]{10,}", "[REDACTED:slack]"),
            (r"(?i)Bearer\s+[A-Za-z0-9_\-\.=]{20,}", "[REDACTED:bearer]"),
            (
                r"-----BEGIN [A-Z ]+PRIVATE KEY-----[\s\S]*?-----END [A-Z ]+PRIVATE KEY-----",
                "[REDACTED:private-key]",
            ),
            (r"AIza[0-9A-Za-z_\-]{35}", "[REDACTED:google]"),
        ];
        let patterns = specs
            .into_iter()
            .map(|(pat, repl)| (Regex::new(pat).expect("valid redact regex"), repl))
            .collect();
        Self { patterns }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (re, replacement) in &self.patterns {
            out = re.replace_all(&out, *replacement).into_owned();
        }
        out
    }

    pub fn is_disabled_by_env() -> bool {
        std::env::var("HUMOS_INDEX_REDACT")
            .map(|v| v.eq_ignore_ascii_case("off") || v == "0" || v.eq_ignore_ascii_case("false"))
            .unwrap_or(false)
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openai_project_key() {
        let r = Redactor::new();
        let input = "OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz1234567890";
        let out = r.redact(input);
        assert!(out.contains("[REDACTED:openai]"));
        assert!(!out.contains("sk-proj-abc"));
    }

    #[test]
    fn redacts_anthropic_key() {
        let r = Redactor::new();
        let out = r.redact("export ANTHROPIC_API_KEY=sk-ant-api03-deadbeefdeadbeefdeadbeef");
        assert!(out.contains("[REDACTED:anthropic]"));
    }

    #[test]
    fn redacts_aws_access_key() {
        let r = Redactor::new();
        let out = r.redact("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE");
        assert!(out.contains("[REDACTED:aws]"));
    }

    #[test]
    fn redacts_github_personal_token() {
        let r = Redactor::new();
        let out = r.redact("token: ghp_abcdefghijklmnopqrstuvwxyz0123456789AB");
        assert!(out.contains("[REDACTED:github]"));
    }

    #[test]
    fn redacts_bearer_header() {
        let r = Redactor::new();
        let out = r.redact("Authorization: Bearer abcd1234567890XYZabcd1234567890XYZ");
        assert!(out.contains("[REDACTED:bearer]"));
    }

    #[test]
    fn redacts_private_key_block() {
        let r = Redactor::new();
        let input = "stuff before\n-----BEGIN RSA PRIVATE KEY-----\nMIIBIjANBgkq\n-----END RSA PRIVATE KEY-----\nstuff after";
        let out = r.redact(input);
        assert!(out.contains("[REDACTED:private-key]"));
        assert!(!out.contains("MIIBIjANBgkq"));
    }

    #[test]
    fn redacts_slack_tokens() {
        let r = Redactor::new();
        let out = r.redact("SLACK_TOKEN=xoxb-1234567890-abcdefghij");
        assert!(out.contains("[REDACTED:slack]"));
    }

    #[test]
    fn redacts_multiple_secrets_in_one_string() {
        let r = Redactor::new();
        let input = "AKIAIOSFODNN7EXAMPLE and sk-ant-api03-deadbeefdeadbeef01234567";
        let out = r.redact(input);
        assert!(out.contains("[REDACTED:aws]"));
        assert!(out.contains("[REDACTED:anthropic]"));
    }

    #[test]
    fn does_not_redact_short_strings_that_look_keyish() {
        let r = Redactor::new();
        let out = r.redact("sk-short and AKIA-too-short");
        assert_eq!(out, "sk-short and AKIA-too-short");
    }

    #[test]
    fn empty_string_passes_through() {
        let r = Redactor::new();
        assert_eq!(r.redact(""), "");
    }

    #[test]
    fn env_disable_flag_is_off_insensitive() {
        std::env::set_var("HUMOS_INDEX_REDACT", "OFF");
        assert!(Redactor::is_disabled_by_env());
        std::env::set_var("HUMOS_INDEX_REDACT", "off");
        assert!(Redactor::is_disabled_by_env());
        std::env::remove_var("HUMOS_INDEX_REDACT");
        assert!(!Redactor::is_disabled_by_env());
    }
}
