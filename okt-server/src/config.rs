//! Knowledge-base conventions: folder families, external systems ("surfaces"), and what counts as
//! what for viewing. Everything KB-specific lives here.

/// Top-level folder -> family. Families drive graph color (rainbow themes) or shape and brightness
/// (terminal theme), and the clockwise order of sectors in the radial layout.
#[rustfmt::skip]
pub const FOLDER_FAMILY: &[(&str, &str)] = &[
    ("", "core"), ("bin", "core"), (".githooks", "core"),
    ("skill", "rules"), ("policy", "rules"),
    ("memory", "memory"), ("decision", "memory"),
    ("project", "cards"), ("system", "cards"), ("contact", "cards"), ("org", "cards"),
    ("workspace", "workspace"),
    ("engagement", "records"), ("activity", "records"), ("cv", "records"), ("note", "records"), ("idea", "records"),
];

pub fn folder_family(top: &str) -> Option<&'static str> {
    FOLDER_FAMILY
        .iter()
        .find(|(k, _)| *k == top)
        .map(|(_, v)| *v)
}

/// (id, label, legend blurb)
#[rustfmt::skip]
pub const FAMILIES: &[(&str, &str, &str)] = &[
    ("core", "Kernel & scaffold", "Root files and tooling"),
    ("rules", "Skills & policies", "How work gets done"),
    ("memory", "Memory & decisions", "Inbox, dockets, proposals, decision records"),
    ("cards", "Cards", "People, orgs, projects, systems"),
    ("workspace", "Workspaces", "Working areas for big efforts"),
    ("records", "Records & ledgers", "Meetings, reports, CV, notes, ideas"),
    ("other", "Other", "Folders not mapped to a family"),
    ("world", "External systems", "MCP connections the KB points into"),
];

/// An external system the KB reaches into. A file connects to one when its frontmatter carries one
/// of the keys with a real value (not TODO / empty), when its text names one of the tool tokens, or
/// when it links to one of the domains.
pub struct Surface {
    pub id: &'static str,
    pub label: &'static str,
    pub keys: &'static [&'static str],
    pub tokens: &'static [&'static str],
    pub domains: &'static [&'static str],
}

#[rustfmt::skip]
pub const SURFACES: &[Surface] = &[
    Surface { id: "slack", label: "Slack", keys: &["slack"], tokens: &["get-channel-history", "slack_", "search-messages"], domains: &["slack.com"] },
    Surface { id: "mail", label: "Mail", keys: &["email"], tokens: &["search-emails", "read-email", "send-email", "outlook"], domains: &["outlook.office.com", "outlook.office365.com"] },
    Surface { id: "calendar", label: "Calendar", keys: &["meetings"], tokens: &["list-events", "get-events", "calendar-events"], domains: &[] },
    Surface { id: "asana-rcac", label: "asana-rcac", keys: &[], tokens: &["asana-rcac"], domains: &["app.asana.com"] },
    Surface { id: "asana-pearc", label: "asana-pearc", keys: &[], tokens: &["asana-pearc"], domains: &[] },
    Surface { id: "github", label: "GitHub", keys: &["repos", "github"], tokens: &["get-notifications", "search_issues", "pull_request_read"], domains: &["github.com"] },
    Surface { id: "rcac-docs", label: "RCAC Docs", keys: &[], tokens: &["doc_search", "doc_load", "rcac-docs-mcp"], domains: &["docs.rcac.purdue.edu"] },
    Surface { id: "clusters", label: "Clusters", keys: &[], tokens: &["rcac-mcp"], domains: &[] },
    Surface { id: "web", label: "Web", keys: &[], tokens: &["web_search", "webfetch", "websearch"], domains: &["*"] },
];

pub fn surface_label(id: &str) -> Option<&'static str> {
    SURFACES.iter().find(|s| s.id == id).map(|s| s.label)
}

pub const ASANA_FM_KEY: &str = "asana";
pub const ASANA_DEFAULT_TENANT: &str = "asana-rcac";

/// Folders with more than this many direct files start collapsed in the graph (UI can override).
pub const DEFAULT_COLLAPSE: i64 = 25;

pub const CODE_EXT: &[&str] = &[
    ".py", ".pyi", ".sh", ".bash", ".zsh", ".fish", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx",
    ".css", ".scss", ".sql", ".go", ".rs", ".rb", ".java", ".kt", ".c", ".h", ".cc", ".cpp",
    ".hpp", ".swift", ".r", ".jl", ".lua", ".pl", ".php", ".ps1", ".html", ".htm", ".xml", ".svg",
    ".vue", ".svelte", ".awk", ".mk",
];
pub const CODE_NAMES: &[&str] = &["Makefile", "Dockerfile", "Justfile", "Rakefile"];
pub const TEXT_EXT: &[&str] = &[
    ".txt",
    ".text",
    ".csv",
    ".tsv",
    ".json",
    ".jsonl",
    ".yaml",
    ".yml",
    ".toml",
    ".ini",
    ".cfg",
    ".conf",
    ".log",
    ".env",
    ".gitignore",
    ".gitkeep",
    ".gitattributes",
    ".rst",
    ".tex",
    ".bib",
    ".ics",
    ".vtt",
    ".srt",
];
pub const MARKDOWN_EXT: &[&str] = &[".md", ".markdown", ".mdown", ".mkd"];
pub const STRUCTURED_EXT: &[&str] = &[".json", ".jsonl", ".geojson", ".yaml", ".yml", ".toml"];
pub const TABLE_EXT: &[&str] = &[".csv", ".tsv"];
pub const IMAGE_EXT: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".bmp", ".ico", ".svg",
];
pub const AUDIO_EXT: &[&str] = &[".mp3", ".wav", ".ogg", ".m4a", ".flac"];
pub const VIDEO_EXT: &[&str] = &[".mp4", ".webm", ".mov", ".m4v"];
pub const PDF_EXT: &[&str] = &[".pdf"];
pub const HTML_EXT: &[&str] = &[".html", ".htm"];
pub const OFFICE_EXT: &[&str] = &[
    ".xlsx", ".xls", ".docx", ".doc", ".pptx", ".ppt", ".key", ".numbers", ".pages", ".zip",
    ".tar", ".gz",
];

pub fn is_known_file_ext(e: &str) -> bool {
    [
        MARKDOWN_EXT,
        CODE_EXT,
        TEXT_EXT,
        IMAGE_EXT,
        PDF_EXT,
        AUDIO_EXT,
        VIDEO_EXT,
        OFFICE_EXT,
    ]
    .iter()
    .any(|set| set.contains(&e))
}

pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024; // text kept in memory for search, per file
pub const MAX_RENDER_BYTES: usize = 2 * 1024 * 1024; // larger text files are shown truncated
pub const MAX_DIFF_LINES: usize = 6000; // per commit view
