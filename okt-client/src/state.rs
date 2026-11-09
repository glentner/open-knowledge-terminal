//! Global state: the boot data the server put in the page, the per-browser settings (in
//! localStorage), and `S`, the client state. Everything lives on the one UI thread; borrows are
//! kept short and never held across calls that may re-enter (focus, blur).

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use crate::arrange::{self, Node, PaneId};
use crate::dom;
use crate::model::{FilePayload, Idx, LogData, SearchFile, TableData};

// ---- boot ----------------------------------------------------------------------------------

#[derive(Default, Debug)]
pub struct Boot {
    pub kb: String,
    pub theme: String,
    pub accent: String,
    pub font: Option<String>,
    pub platform: String,
    pub app: String,
    pub version: String,
    pub build: String,
    pub watch: bool,
    pub interval: f64,
    pub editor: Option<String>,
}

fn read_boot() -> Boot {
    let v: Value = dom::try_id("boot")
        .and_then(|el| el.text_content())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Boot {
        kb: s("kb"),
        theme: s("theme"),
        accent: s("accent"),
        font: v
            .get("font")
            .and_then(|x| x.as_str())
            .filter(|x| !x.is_empty())
            .map(String::from),
        platform: s("platform"),
        app: s("app"),
        version: s("version"),
        build: s("build"),
        watch: v.get("watch").and_then(|x| x.as_bool()).unwrap_or(false),
        interval: v.get("interval").and_then(|x| x.as_f64()).unwrap_or(2.0),
        editor: v.get("editor").and_then(|x| x.as_str()).map(String::from),
    }
}

impl Boot {
    /// `BOOT.app || 'KB Terminal'`
    pub fn app_name(&self) -> &str {
        if self.app.is_empty() {
            "Open Knowledge Terminal"
        } else {
            &self.app
        }
    }
    /// The file manager's name on this platform.
    pub fn filer(&self) -> &'static str {
        match self.platform.as_str() {
            "darwin" => "Finder",
            "win32" | "windows" => "Explorer",
            _ => "the file manager",
        }
    }
}

// ---- settings ------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Settings {
    pub theme: String,
    pub accent: String,
    pub reader: Option<String>,
    pub ticker: bool,
    pub labels: bool,
    /// the splitter width before arrangements, a percentage: read (it sizes classic on a first
    /// load) and written back unchanged, so an older okt still finds it; nothing updates it now
    pub insp: f64,
    pub collapsed: IndexMap<String, bool>,
    pub collapse: Option<i64>,
    pub tree_open: IndexMap<String, bool>,
    pub table_cols: Option<IndexMap<String, Vec<String>>>,
    pub badge: Option<bool>,
    /// up to nine pinned ids, for the keys 1-9 (m pins and unpins)
    pub pins: Vec<String>,
    /// the pane arrangement in use
    pub arr: Node,
    /// the arrangement was chosen (loaded from the store, or changed by the user), so it is
    /// written; until then a save of any other setting leaves the default unsaved
    pub arr_saved: bool,
    extra: Map<String, Value>,
}

fn skey() -> String {
    format!(
        "okt:{}",
        if boot().kb.is_empty() {
            "kb"
        } else {
            &boot().kb
        }
    )
}

fn storage() -> Option<web_sys::Storage> {
    dom::win().local_storage().ok().flatten()
}

impl Settings {
    fn load() -> Settings {
        let stored: Option<Map<String, Value>> = storage()
            .and_then(|st| st.get_item(&skey()).ok().flatten())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| {
                if let Value::Object(m) = v {
                    Some(m)
                } else {
                    None
                }
            });
        let width = dom::win()
            .inner_width()
            .ok()
            .and_then(|w| w.as_f64())
            .unwrap_or(1600.0);
        Settings::from_map(boot(), stored, width)
    }

    /// The settings from the stored blob, with the boot defaults. Pure, so the migration is tested
    /// natively. `width` is the window's at load: with no arrangement saved it picks the default,
    /// once per load and never on a window resize.
    fn from_map(b: &Boot, stored: Option<Map<String, Value>>, width: f64) -> Settings {
        let mut s = Settings {
            theme: if b.theme.is_empty() {
                "terminal".into()
            } else {
                b.theme.clone()
            },
            accent: if b.accent.is_empty() {
                "#ffa028".into()
            } else {
                b.accent.clone()
            },
            reader: None,
            ticker: true,
            labels: false,
            insp: 46.0,
            collapsed: IndexMap::new(),
            collapse: None,
            tree_open: IndexMap::new(),
            table_cols: None,
            badge: None,
            pins: Vec::new(),
            arr: arrange::default_for(width, None),
            arr_saved: false,
            extra: Map::new(),
        };
        let Some(m) = stored else { return s };
        let mut arr = None;
        let mut had_insp = false;
        for (k, v) in m {
            match (k.as_str(), &v) {
                ("theme", Value::String(x)) => s.theme = x.clone(),
                ("accent", Value::String(x)) => s.accent = x.clone(),
                ("reader", Value::String(x)) => s.reader = Some(x.clone()),
                ("reader", Value::Null) => s.reader = None,
                ("ticker", Value::Bool(x)) => s.ticker = *x,
                ("labels", Value::Bool(x)) => s.labels = *x,
                ("insp", Value::Number(x)) => {
                    s.insp = x.as_f64().unwrap_or(46.0);
                    had_insp = true;
                }
                // read below; one that doesn't parse is dropped, never kept in `extra`, where it
                // would be written back beside a different tree
                ("arrangement", _) => arr = Some(v),
                ("collapsed", Value::Object(o)) => {
                    s.collapsed = o
                        .iter()
                        .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                        .collect()
                }
                ("collapse", Value::Number(x)) => s.collapse = x.as_i64(),
                ("collapse", Value::Null) => s.collapse = None,
                ("treeOpen", Value::Object(o)) => {
                    s.tree_open = o
                        .iter()
                        .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                        .collect()
                }
                ("tableCols", Value::Object(o)) => {
                    s.table_cols = Some(
                        o.iter()
                            .filter_map(|(k, v)| {
                                v.as_array().map(|a| {
                                    (
                                        k.clone(),
                                        a.iter()
                                            .filter_map(|x| x.as_str().map(String::from))
                                            .collect(),
                                    )
                                })
                            })
                            .collect(),
                    )
                }
                ("badge", Value::Bool(x)) => s.badge = Some(*x),
                ("pins", Value::Array(a)) => {
                    s.pins = a
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .take(9)
                        .collect()
                }
                _ => {
                    s.extra.insert(k, v);
                }
            }
        }
        match arr
            .as_ref()
            .and_then(arrange::from_json)
            .and_then(arrange::sanitise)
        {
            Some(n) => {
                s.arr = n;
                s.arr_saved = true;
            }
            // a splitter width saved before arrangements sizes classic, when classic starts
            None => s.arr = arrange::default_for(width, had_insp.then_some(s.insp)),
        }
        s
    }

    fn to_json(&self) -> Value {
        let mut m = self.extra.clone();
        m.insert("theme".into(), json!(self.theme));
        m.insert("accent".into(), json!(self.accent));
        m.insert("reader".into(), json!(self.reader));
        m.insert("ticker".into(), json!(self.ticker));
        m.insert("labels".into(), json!(self.labels));
        m.insert("insp".into(), json!(self.insp));
        m.insert("collapsed".into(), json!(self.collapsed));
        m.insert("collapse".into(), json!(self.collapse));
        m.insert("treeOpen".into(), json!(self.tree_open));
        if let Some(tc) = &self.table_cols {
            m.insert("tableCols".into(), json!(tc));
        }
        if let Some(b) = self.badge {
            m.insert("badge".into(), json!(b));
        }
        if !self.pins.is_empty() {
            m.insert("pins".into(), json!(self.pins));
        }
        if self.arr_saved {
            m.insert("arrangement".into(), arrange::to_json(&self.arr));
        }
        Value::Object(m)
    }

    /// The reader font in effect (`settings.reader || (theme === 'terminal' ? 'mono' : 'sans')`).
    pub fn reader_font(&self) -> String {
        match &self.reader {
            Some(r) if !r.is_empty() => r.clone(),
            _ => {
                if self.theme == "terminal" {
                    "mono".into()
                } else {
                    "sans".into()
                }
            }
        }
    }
}

static SAVE: dom::Debounce = dom::Debounce::new(200);

thread_local! {
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

/// `saveSettings()` (debounced 200 ms).
pub fn save_settings() {
    PENDING.set(true);
    SAVE.call(write_settings);
}

fn write_settings() {
    PENDING.set(false);
    let text = cfg().to_json().to_string();
    if let Some(st) = storage() {
        let _ = st.set_item(&skey(), &text);
    }
}

/// Write a save still waiting out its debounce, now (on `pagehide`, so a reload keeps it).
pub fn flush_settings() {
    if PENDING.get() {
        write_settings();
    }
}

// ---- client state (S) ------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Live {
    Ok,
    Err,
    Off,
    Busy,
}

#[derive(Clone)]
pub enum ListData {
    Find {
        title: String,
        meta: String,
        files: Vec<SearchFile>,
        names: Vec<String>,
        query: String,
    },
    Paths {
        title: String,
        meta: String,
        items: Vec<ListItem>,
    },
}

#[derive(Clone)]
pub struct ListItem {
    pub id: String,
    pub ln: Option<i64>,
    pub label: String,
    pub meta: String,
}

/// A cached `/api/file` response: shared while in flight, parsed once.
pub enum FileEntry {
    Pending(js_sys::Promise),
    Ready(Rc<FilePayload>),
}

pub enum HistEntry {
    Pending(js_sys::Promise),
    Ready(Rc<Vec<crate::model::Commit>>),
}

pub struct State {
    pub idx: Option<Rc<Idx>>,
    /// the current one of the five main views: what the focused pane shows, or, while a pane
    /// showing the Inspector has focus, the last of the five that had it (the hash's `v`)
    pub view: &'static str,
    /// the focused pane, which the view keys act on
    pub focus: PaneId,
    pub sel: Option<String>,
    pub tab: String,
    pub commit: Option<String>,
    pub rev: Option<String>,
    pub mode: String,
    pub blame: bool,
    pub jump: Option<i64>,
    /// after n/N opens a file at a FIND match, mark the first (1) or last (-1) match
    /// on the jumped-to line as the current one
    pub find_cur: Option<i64>,
    pub keep: Option<i64>,
    pub mark: Option<String>,
    pub log_data: Option<Rc<LogData>>,
    pub log_filter: String,
    pub log_prefix: Option<String>,
    pub log_day: Option<String>,
    pub table: Option<Rc<TableData>>,
    pub table_folder: Option<String>,
    pub table_recursive: bool,
    pub table_sort: Option<(String, i32)>,
    pub table_filter: String,
    pub table_cols: Option<Vec<String>>,
    pub list: Option<Rc<ListData>>,
    pub list_query: Option<String>,
    pub files: IndexMap<String, FileEntry>,
    pub hist: HashMap<String, HistEntry>,
    pub live: Live,
    pub new_commits: HashSet<String>,
    pub tree_auto: HashSet<String>,
    pub tree_revealed: Option<String>,
    pub restoring: bool,
    pub started: Option<f64>,
    pub travel_nav: Option<(Option<String>, Option<String>)>,
    pub data_sort: Option<(usize, i32)>,
}

impl State {
    fn new() -> State {
        State {
            idx: None,
            view: "graph",
            focus: 0,
            sel: None,
            tab: "overview".into(),
            commit: None,
            rev: None,
            mode: "rendered".into(),
            blame: false,
            jump: None,
            find_cur: None,
            keep: None,
            mark: None,
            log_data: None,
            log_filter: String::new(),
            log_prefix: None,
            log_day: None,
            table: None,
            table_folder: None,
            table_recursive: false,
            table_sort: None,
            table_filter: String::new(),
            table_cols: None,
            list: None,
            list_query: None,
            files: IndexMap::new(),
            hist: HashMap::new(),
            live: Live::Ok,
            new_commits: HashSet::new(),
            tree_auto: HashSet::new(),
            tree_revealed: None,
            restoring: false,
            started: None,
            travel_nav: None,
            data_sort: None,
        }
    }
}

thread_local! {
    static BOOT: &'static Boot = Box::leak(Box::new(read_boot()));
    static SETTINGS: &'static RefCell<Settings> = Box::leak(Box::new(RefCell::new(Settings::load())));
    static STATE: &'static RefCell<State> = Box::leak(Box::new(RefCell::new(State::new())));
}

pub fn boot() -> &'static Boot {
    BOOT.with(|b| *b)
}

/// The settings (read).
pub fn cfg() -> Ref<'static, Settings> {
    SETTINGS.with(|s| *s).borrow()
}

/// The settings (write). Call [`save_settings`] after.
pub fn cfg_mut() -> RefMut<'static, Settings> {
    SETTINGS.with(|s| *s).borrow_mut()
}

/// `S` (read).
pub fn s() -> Ref<'static, State> {
    STATE.with(|s| *s).borrow()
}

/// `S` (write).
pub fn s_mut() -> RefMut<'static, State> {
    STATE.with(|s| *s).borrow_mut()
}

/// The index, if loaded (a cheap `Rc` clone; no borrow is held).
pub fn idx() -> Option<Rc<Idx>> {
    s().idx.clone()
}

pub fn sel() -> Option<String> {
    s().sel.clone()
}

/// `collapseN()`
pub fn collapse_n() -> i64 {
    if let Some(c) = cfg().collapse {
        return c;
    }
    idx().and_then(|i| i.collapse).unwrap_or(25)
}

/// `isCollapsed(id)`
pub fn is_collapsed(idx: &Idx, id: &str) -> bool {
    let Some(n) = idx.node(id) else { return false };
    if n.t != "dir" {
        return false;
    }
    if let Some(p) = cfg().collapsed.get(id) {
        return *p;
    }
    let c = collapse_n();
    c > 0 && idx.dinfo(id).map(|d| d.direct).unwrap_or(0.0) > c as f64
}

/// `currentFolder()`
pub fn current_folder() -> String {
    let (Some(idx), Some(sel)) = (idx(), sel()) else {
        return ".".into();
    };
    let Some(n) = idx.node(&sel) else {
        return ".".into();
    };
    match n.t.as_str() {
        "dir" => n.id.clone(),
        "file" => n.parent.clone().unwrap_or_else(|| ".".into()),
        _ => ".".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrangement_is_written_only_once_chosen() {
        let b = Boot::default();
        // a first load: the width default, unsaved, and a save of another setting keeps it so
        let mut s = Settings::from_map(&b, None, 1600.0);
        assert!(!s.arr_saved);
        assert_eq!(s.arr, arrange::default_for(1600.0, None));
        s.labels = true;
        let j = s.to_json();
        assert!(j.get("arrangement").is_none());
        assert_eq!(j["labels"], json!(true));
        assert_eq!(j["insp"], json!(46.0), "insp is still written");
        // a pre-arrangement blob: insp sizes classic at the width that starts classic, and stays
        let old = json!({"theme": "dark", "insp": 30});
        let s2 = Settings::from_map(&b, old.as_object().cloned(), 1600.0);
        assert!(!s2.arr_saved);
        assert_eq!(s2.arr, arrange::default_for(1600.0, Some(30.0)));
        assert_eq!(s2.to_json()["insp"], json!(30.0));
        assert!(s2.to_json().get("arrangement").is_none());
        let narrow = Settings::from_map(&b, old.as_object().cloned(), 899.0);
        assert_eq!(narrow.arr, arrange::stacked());
        // chosen: written, and read back as saved at any width
        s.arr_saved = true;
        let j = s.to_json();
        assert!(j.get("arrangement").is_some());
        let back = Settings::from_map(&b, j.as_object().cloned(), 700.0);
        assert!(back.arr_saved);
        assert_eq!(back.arr, s.arr);
        assert_eq!(back.to_json()["arrangement"], j["arrangement"]);
        // one that doesn't parse is dropped, not carried in `extra`
        let bad = json!({"theme": "dark", "arrangement": "classic", "future": 1});
        let s3 = Settings::from_map(&b, bad.as_object().cloned(), 1600.0);
        assert!(!s3.arr_saved);
        assert!(s3.to_json().get("arrangement").is_none());
        assert_eq!(s3.to_json()["future"], json!(1));
    }
}
