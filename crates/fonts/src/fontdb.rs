//! Font database: bundled OFL fonts, user fonts, the installed system fonts (cataloged the first
//! time a lookup needs them, native only), outline cache.

mod discovery;
use discovery::enumerate_faces;
#[cfg(not(target_arch = "wasm32"))]
use discovery::file_face_names;
pub use discovery::system_font_dirs;

mod face;
pub(crate) use face::last_resort_face;
pub use face::{FaceRef, FontFace};
use face::{FontBytes, make_face};

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use kurbo::BezPath;
use skrifa::instance::Size;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, MetadataProvider};

/// The family used when a requested family is unknown and has no substitute.
pub const FALLBACK_FAMILY: &str = "Inter";

/// The always-available last-resort face (Ubuntu Light, compiled in through
/// `epaint_default_fonts`; licence in ATTRIBUTION.md).
pub const LAST_RESORT_FAMILY: &str = "Ubuntu";

/// Compiled-in font files: the craft-fonts faces (Latin presentation fonts, then Japanese;
/// empty without `CRAFT_FONTS_DIR`), then the last-resort face.
pub fn bundled() -> Vec<&'static [u8]> {
    let mut v: Vec<&'static [u8]> = crate::CRAFT_FONTS.iter().map(|f| f.bytes).collect();
    v.push(epaint_default_fonts::UBUNTU_LIGHT);
    v
}

/// Open substitutes for families documents often ask for but we can't ship (metric-compatible
/// where one exists), tried in order after the family itself.
pub fn substitutes(family: &str) -> &'static [&'static str] {
    match family.to_ascii_lowercase().as_str() {
        "calibri" | "calibri light" => &["Carlito", "Liberation Sans", "Arial", "Inter"],
        "cambria" | "cambria math" => &["Caladea", "Liberation Serif", "Times New Roman", "Merriweather"],
        "arial" | "helvetica" | "helvetica neue" | "arial nova" => &["Liberation Sans", "Arial", "Helvetica Neue", "Helvetica", "Inter"],
        "times new roman" | "times" => &["Liberation Serif", "Times New Roman", "Times", "Merriweather"],
        "courier new" | "courier" | "consolas" | "menlo" => &["Liberation Mono", "Courier New", "Menlo", "Courier"],
        "aptos" | "aptos display" | "aptos narrow" | "segoe ui" | "segoe ui light" | "segoe ui semibold" | "tahoma" | "verdana" => {
            &["Inter", "Helvetica Neue", "Arial", "Liberation Sans"]
        }
        "georgia" | "garamond" | "book antiqua" | "palatino" | "palatino linotype" | "constantia" => &["Merriweather", "Liberation Serif", "Georgia"],
        "century gothic" | "avenir" | "avenir next" | "futura" | "gill sans" | "gill sans mt" | "trebuchet ms" | "corbel" | "candara" => {
            &["Montserrat", "Poppins", "Avenir Next", "Inter"]
        }
        "franklin gothic" | "franklin gothic book" | "franklin gothic medium" | "univers" => &["Open Sans", "Inter"],
        "open sans" | "lato" | "roboto" | "montserrat" | "poppins" | "nunito sans" => &["Inter"],
        "playfair display" | "merriweather" | "source serif 4" | "source serif pro" => &["Merriweather", "Liberation Serif", "Georgia"],
        "source sans 3" | "source sans pro" => &["Open Sans", "Inter"],
        "impact" | "arial black" => &["Montserrat", "Inter"],
        _ => &[],
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
struct CatalogEntry {
    family: String,
    style: String,
    path: std::path::PathBuf,
}

/// Process-wide font database.
pub struct FontDb {
    faces: RwLock<Vec<Arc<FontFace>>>,
    outlines: Mutex<HashMap<(u32, u32), Arc<BezPath>>>,
    #[cfg(not(target_arch = "wasm32"))]
    catalog: RwLock<Vec<CatalogEntry>>,
    /// The folders the system font scan reads (the platform's font folders for [`FontDb::global`]).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    font_dirs: Vec<std::path::PathBuf>,
    /// Set once the font folders have been scanned. Lookups by family name wait for the first
    /// scan, so what they find doesn't depend on what ran before them.
    #[cfg(not(target_arch = "wasm32"))]
    cataloged: std::sync::OnceLock<()>,
    /// System fallback state: enabled, and characters no system font covers.
    #[cfg(not(target_arch = "wasm32"))]
    sys: Mutex<SysFallback>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct SysFallback {
    enabled: bool,
    misses: std::collections::HashSet<char>,
}

/// Families tried (when installed) for characters the loaded fonts lack: CJK, symbols, emoji.
#[cfg(not(target_arch = "wasm32"))]
const SYSTEM_FALLBACKS: &[&str] = &[
    "Helvetica Neue",
    "Arial",
    "Segoe UI",
    "Noto Sans",
    "DejaVu Sans",
    "PingFang SC",
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Apple SD Gothic Neo",
    "Heiti SC",
    "STHeiti",
    "Microsoft YaHei",
    "Yu Gothic",
    "Malgun Gothic",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Arial Unicode MS",
    "Apple Symbols",
    "Segoe UI Symbol",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
    "Noto Emoji",
    "Segoe UI Emoji",
    "Apple Color Emoji",
    "Noto Color Emoji",
];

const OUTLINE_CACHE_MAX: usize = 50_000;

/// The named style part of a style with axis settings (`Bold {wght:650}` → `Bold`).
pub fn base_style(style: &str) -> &str {
    style.split('{').next().unwrap_or(style).trim()
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Weight implied by a style name.
fn style_weight(style: &str) -> f32 {
    let s = norm(style);
    const TABLE: &[(&str, f32)] = &[
        ("extralight", 200.0),
        ("ultralight", 200.0),
        ("semibold", 600.0),
        ("demibold", 600.0),
        ("extrabold", 800.0),
        ("ultrabold", 800.0),
        ("hairline", 100.0),
        ("thin", 100.0),
        ("light", 300.0),
        ("medium", 500.0),
        ("bold", 700.0),
        ("black", 900.0),
        ("heavy", 900.0),
    ];
    TABLE.iter().find(|(k, _)| s.contains(k)).map(|(_, w)| *w).unwrap_or(400.0)
}

fn style_italic(style: &str) -> bool {
    let s = norm(style);
    s.contains("italic") || s.contains("oblique") || s == "it"
}

impl FontDb {
    /// A database holding the bundled fonts, then the craft-fonts Japanese faces (Mincho first,
    /// so they are the fallback for Japanese text after the requested and bundled fonts; empty
    /// without `CRAFT_FONTS_DIR`), whose system font scan reads `font_dirs`.
    pub fn with_font_dirs(font_dirs: Vec<std::path::PathBuf>) -> Self {
        let mut faces = Vec::new();
        for data in bundled() {
            for (i, family, style, coords) in enumerate_faces(data) {
                if let Some(f) = make_face(FontBytes::Static(data), i, family, style, coords) {
                    faces.push(Arc::new(f));
                }
            }
        }
        Self {
            faces: RwLock::new(faces),
            outlines: Mutex::new(HashMap::new()),
            #[cfg(not(target_arch = "wasm32"))]
            catalog: RwLock::new(Vec::new()),
            font_dirs,
            #[cfg(not(target_arch = "wasm32"))]
            cataloged: std::sync::OnceLock::new(),
            #[cfg(not(target_arch = "wasm32"))]
            sys: Mutex::new(SysFallback { enabled: true, ..Default::default() }),
        }
    }

    /// Enable or disable the lazy system-font fallback for characters the loaded fonts lack
    /// (native only; on by default).
    pub fn set_system_fallback(&self, on: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.sys.lock().unwrap_or_else(|e| e.into_inner()).enabled = on;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = on;
    }

    /// Process-wide database: the bundled fonts, then the installed system fonts, cataloged the
    /// first time a lookup by family name needs them (or ahead of time by
    /// [`FontDb::scan_in_background`]).
    pub fn global() -> &'static FontDb {
        static DB: std::sync::OnceLock<FontDb> = std::sync::OnceLock::new();
        DB.get_or_init(|| FontDb::with_font_dirs(system_font_dirs()))
    }

    fn read_faces(&self) -> std::sync::RwLockReadGuard<'_, Vec<Arc<FontFace>>> {
        self.faces.read().unwrap_or_else(|e| e.into_inner())
    }

    /// The system font catalog, scanned first if it hasn't been yet.
    #[cfg(not(target_arch = "wasm32"))]
    fn read_catalog(&self) -> std::sync::RwLockReadGuard<'_, Vec<CatalogEntry>> {
        self.ensure_catalog();
        self.catalog.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Family names available (loaded plus cataloged system fonts), sorted and deduplicated.
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.read_faces().iter().map(|f| f.family.clone()).collect();
        #[cfg(not(target_arch = "wasm32"))]
        v.extend(self.read_catalog().iter().map(|c| c.family.clone()));
        v.sort_by_key(|a| a.to_lowercase());
        v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        v
    }

    /// Style names available for `family` (Regular first, then by weight).
    pub fn styles(&self, family: &str) -> Vec<String> {
        let mut v: Vec<(bool, f32, String)> = self
            .read_faces()
            .iter()
            .filter(|f| f.family.eq_ignore_ascii_case(family) && !f.style.contains('{'))
            .map(|f| (f.italic, f.weight, f.style.clone()))
            .collect();
        #[cfg(not(target_arch = "wasm32"))]
        for c in self.read_catalog().iter() {
            if c.family.eq_ignore_ascii_case(family) && !v.iter().any(|(_, _, s)| s.eq_ignore_ascii_case(&c.style)) {
                v.push((style_italic(&c.style), style_weight(&c.style), c.style.clone()));
            }
        }
        v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        v.dedup_by(|a, b| a.2 == b.2);
        v.into_iter().map(|t| t.2).collect()
    }

    /// Add a user font (TTF/OTF/TTC bytes). Returns the number of faces added (0 if unparseable or
    /// every face was already present).
    pub fn add_font(&self, bytes: Vec<u8>) -> usize {
        let data = Arc::new(bytes);
        let mut added = 0;
        for (i, family, style, coords) in enumerate_faces(&data) {
            if self.read_faces().iter().any(|f| f.family.eq_ignore_ascii_case(&family) && f.style.eq_ignore_ascii_case(&style)) {
                continue;
            }
            if let Some(f) = make_face(FontBytes::Owned(data.clone()), i, family, style, coords) {
                self.faces.write().unwrap_or_else(|e| e.into_inner()).push(Arc::new(f));
                added += 1;
            }
        }
        added
    }

    /// Catalog the installed fonts unless that has been done: the first caller scans the font
    /// folders, any other waits for that scan to finish.
    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_catalog(&self) {
        self.cataloged.get_or_init(|| {
            self.scan_font_dirs();
        });
    }

    /// Catalog the installed fonts on a background thread, so the first lookup by family name
    /// (opening a file, the font menus) doesn't wait for the scan. A no-op once they are
    /// cataloged, and on wasm.
    pub fn scan_in_background(&'static self) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.cataloged.get().is_none() {
            // A failed spawn leaves the scan to the first lookup that needs it.
            let _ = std::thread::Builder::new().name("font-scan".into()).spawn(move || self.ensure_catalog());
        }
    }

    /// Scan the font folders again (fonts installed or removed since), cataloging the faces
    /// found (native only). Font data is loaded when a cataloged family is first resolved.
    /// Returns the number of faces cataloged.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_system_fonts(&self) -> usize {
        // The first scan, or a rescan once it is done (never both at once).
        let mut first = None;
        self.cataloged.get_or_init(|| first = Some(self.scan_font_dirs()));
        first.unwrap_or_else(|| self.scan_font_dirs())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scan_font_dirs(&self) -> usize {
        let mut found = Vec::new();
        let mut stack = self.font_dirs.clone();
        // Each folder once, however links lead back to it.
        let mut visited = std::collections::HashSet::new();
        while let Some(d) = stack.pop() {
            if !visited.insert(std::fs::canonicalize(&d).unwrap_or_else(|_| d.clone())) {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
                if !matches!(ext.as_deref(), Some("ttf" | "otf" | "ttc" | "otc")) {
                    continue;
                }
                for (family, style) in file_face_names(&p) {
                    found.push(CatalogEntry { family, style, path: p.clone() });
                }
            }
        }
        let n = found.len();
        log::debug!("cataloged {n} system font faces");
        *self.catalog.write().unwrap_or_else(|e| e.into_inner()) = found;
        n
    }

    /// Load the files of the installed `family`. Returns whether any face was added.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_cataloged(&self, family: &str) -> bool {
        let paths: Vec<std::path::PathBuf> = {
            let cat = self.read_catalog();
            let mut p: Vec<_> = cat.iter().filter(|c| c.family.eq_ignore_ascii_case(family)).map(|c| c.path.clone()).collect();
            p.sort();
            p.dedup();
            p
        };
        let mut any = false;
        for p in paths {
            if let Ok(data) = std::fs::read(&p) {
                any |= self.add_font(data) > 0;
            }
        }
        any
    }

    /// Resolve a family + style to a face, falling back to the closest style of the family, then to
    /// an open substitute ([`substitutes`]), then to Inter. Installed system fonts are found by name
    /// whatever ran before.
    pub fn face(&self, family: &str, style: &str) -> Arc<FontFace> {
        if let Some(f) = self.try_family(family, style) {
            return f;
        }
        for sub in substitutes(family) {
            if let Some(f) = self.try_family(sub, style) {
                return f;
            }
        }
        for sub in [FALLBACK_FAMILY, "Helvetica Neue", "Arial", "Liberation Sans", "DejaVu Sans", "Segoe UI"] {
            if let Some(f) = self.try_family(sub, style) {
                return f;
            }
        }
        self.find(LAST_RESORT_FAMILY, "Regular")
            .or_else(|| self.find(FALLBACK_FAMILY, "Regular"))
            .or_else(|| self.read_faces().first().cloned())
            .unwrap_or_else(last_resort_face)
    }

    fn try_family(&self, family: &str, style: &str) -> Option<Arc<FontFace>> {
        if let Some(f) = self.find(family, style) {
            return Some(f);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.load_cataloged(family)
            && let Some(f) = self.find(family, style)
        {
            return Some(f);
        }
        None
    }

    /// Is `family` available (loaded, or installed on the system)?
    pub fn has_family(&self, family: &str) -> bool {
        if self.is_loaded(family) {
            return true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.read_catalog().iter().any(|c| c.family.eq_ignore_ascii_case(family)) {
            return true;
        }
        false
    }

    fn is_loaded(&self, family: &str) -> bool {
        self.read_faces().iter().any(|f| f.family.eq_ignore_ascii_case(family))
    }

    /// A variable font's axes: (tag, name, min, default, max), empty for static fonts.
    pub fn axes(&self, family: &str, style: &str) -> Vec<(String, String, f32, f32, f32)> {
        let face = self.face(family, base_style(style));
        let Some(f) = face.skrifa() else { return vec![] };
        f.axes()
            .iter()
            .map(|a| {
                let tag = String::from_utf8_lossy(&a.tag().to_be_bytes()).to_string();
                let name = f.localized_strings(a.name_id()).english_or_first().map(|s| s.to_string()).unwrap_or_else(|| tag.clone());
                (tag, name, a.min_value(), a.default_value(), a.max_value())
            })
            .collect()
    }

    /// `Style {wght:650,wdth:90}`: the named style's font at those axis values, made on first use.
    fn instance(&self, family: &str, style: &str) -> Option<Arc<FontFace>> {
        let open = style.find('{')?;
        let base = self.find(family, style[..open].trim())?;
        if base.skrifa().is_none_or(|f| f.axes().is_empty()) {
            return Some(base);
        }
        let mut coords = base.coords.clone();
        for kv in style[open + 1..].trim_end_matches('}').split(',') {
            let (k, v) = kv.split_once(':')?;
            let k = k.trim().as_bytes();
            let v: f32 = v.trim().parse().ok()?;
            if k.len() != 4 {
                return None;
            }
            let tag = [k[0], k[1], k[2], k[3]];
            match coords.iter_mut().find(|(t, _)| *t == tag) {
                Some(c) => c.1 = v,
                None => coords.push((tag, v)),
            }
        }
        let f = make_face(base.bytes.clone(), base.index, base.family.clone(), style.to_string(), coords)?;
        let f = Arc::new(f);
        self.faces.write().unwrap_or_else(|e| e.into_inner()).push(f.clone());
        Some(f)
    }

    fn find(&self, family: &str, style: &str) -> Option<Arc<FontFace>> {
        if style.contains('{') {
            {
                let faces = self.read_faces();
                if let Some(f) = faces.iter().find(|f| f.family.eq_ignore_ascii_case(family) && f.style == style) {
                    return Some(f.clone());
                }
            }
            return self.instance(family, style);
        }
        let faces = self.read_faces();
        let cands: Vec<&Arc<FontFace>> = faces.iter().filter(|f| f.family.eq_ignore_ascii_case(family)).collect();
        if cands.is_empty() {
            return None;
        }
        let ns = norm(style);
        if let Some(f) = cands.iter().find(|f| norm(&f.style) == ns) {
            return Some((*f).clone());
        }
        let (tw, ti) = (style_weight(style), style_italic(style));
        cands
            .iter()
            .min_by(|a, b| {
                let sa = (a.weight - tw).abs() + if a.italic != ti { 1000.0 } else { 0.0 };
                let sb = (b.weight - tw).abs() + if b.italic != ti { 1000.0 } else { 0.0 };
                sa.total_cmp(&sb)
            })
            .map(|f| (*f).clone())
    }

    /// First face (fallback family first, then load order) that covers `c`; on native, system
    /// fonts are cataloged and loaded lazily the first time no loaded face covers a character.
    pub fn fallback_for(&self, c: char, exclude: u32) -> Option<Arc<FontFace>> {
        if let Some(f) = self.loaded_fallback(c, exclude) {
            return Some(f);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.system_fallback(c) {
            return self.loaded_fallback(c, exclude);
        }
        None
    }

    fn loaded_fallback(&self, c: char, exclude: u32) -> Option<Arc<FontFace>> {
        let faces = self.read_faces();
        let mut order: Vec<&Arc<FontFace>> = faces.iter().filter(|f| f.id != exclude).collect();
        order.sort_by_key(|f| (!f.family.eq_ignore_ascii_case(FALLBACK_FAMILY), f.italic, (f.weight - 400.0).abs() as i32));
        order.into_iter().find(|f| f.covers(c)).cloned()
    }

    /// Load a system font covering `c` (preferred fallback families first, then any cataloged
    /// file under 40 MB). Returns true if one was loaded. Misses are remembered.
    #[cfg(not(target_arch = "wasm32"))]
    fn system_fallback(&self, c: char) -> bool {
        if c.is_control() || c.is_whitespace() {
            return false;
        }
        {
            let sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());
            if !sys.enabled || sys.misses.contains(&c) {
                return false;
            }
        }
        let covered = |db: &FontDb| db.read_faces().iter().any(|f| f.covers(c));
        let cataloged: Vec<String> = self.read_catalog().iter().map(|e| e.family.clone()).collect();
        for fam in SYSTEM_FALLBACKS {
            if !self.is_loaded(fam) && cataloged.iter().any(|f| f.eq_ignore_ascii_case(fam)) {
                self.load_cataloged(fam);
                if covered(self) {
                    return true;
                }
            }
        }
        let mut paths: Vec<std::path::PathBuf> = self.catalog.read().unwrap_or_else(|e| e.into_inner()).iter().map(|e| e.path.clone()).collect();
        paths.sort();
        paths.dedup();
        for p in paths {
            if std::fs::metadata(&p).map(|m| m.len() > 40 << 20).unwrap_or(true) {
                continue;
            }
            let Ok(data) = std::fs::read(&p) else { continue };
            let hit =
                enumerate_faces(&data).iter().any(|(i, _, _, _)| skrifa::FontRef::from_index(&data, *i).is_ok_and(|f| f.charmap().map(c).is_some()));
            if hit && self.add_font(data) > 0 && covered(self) {
                return true;
            }
        }
        self.sys.lock().unwrap_or_else(|e| e.into_inner()).misses.insert(c);
        false
    }

    /// Glyph outline in font units, y-down (flipped), cached per (face, glyph).
    pub fn outline(&self, face: &FontFace, gid: u32) -> Arc<BezPath> {
        let key = (face.id, gid);
        if let Some(p) = self.outlines.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return p.clone();
        }
        let mut pen = FlipPen(BezPath::new());
        if let Some(f) = face.skrifa()
            && let Some(g) = f.outline_glyphs().get(GlyphId::new(gid))
        {
            let _ = g.draw(DrawSettings::unhinted(Size::unscaled(), face.location()), &mut pen);
        }
        let p = Arc::new(pen.0);
        let mut cache = self.outlines.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= OUTLINE_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, p.clone());
        p
    }
}

struct FlipPen(BezPath);

impl OutlinePen for FlipPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, -y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, -y as f64));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to((cx0 as f64, -cy0 as f64), (x as f64, -y as f64));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to((cx0 as f64, -cy0 as f64), (cx1 as f64, -cy1 as f64), (x as f64, -y as f64));
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}
