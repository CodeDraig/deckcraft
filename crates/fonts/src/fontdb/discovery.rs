//! Discover face names and variable instances from fonts on disk and in memory.

use skrifa::MetadataProvider;
use skrifa::raw::FileRef;
use skrifa::string::StringId;

pub(super) fn name(font: &skrifa::FontRef<'_>, ids: &[StringId]) -> Option<String> {
    ids.iter().find_map(|id| font.localized_strings(*id).english_or_first().map(|s| s.to_string()).filter(|s| !s.is_empty()))
}

/// A face's family and style names (the default instance's style for a variable font).
pub(super) fn face_names(f: &skrifa::FontRef<'_>) -> Option<(String, String)> {
    let family = name(f, &[StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME])?;
    let style = name(f, &[StringId::TYPOGRAPHIC_SUBFAMILY_NAME, StringId::SUBFAMILY_NAME]).unwrap_or_else(|| "Regular".into());
    Some((family, style))
}

/// (family, style) of every face in the font file at `path`, reading only its table directories
/// and `name` tables: a scan opens hundreds of font files, many of them megabytes long. A
/// variable font is cataloged under its default style; its named instances appear once it loads.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn file_face_names(path: &std::path::Path) -> Vec<(String, String)> {
    use std::io::{Read, Seek, SeekFrom};
    /// Caps on what a (possibly damaged) file can make the scan read.
    const MAX_FACES: u32 = 256;
    const MAX_NAME_TABLE: u32 = 1 << 20;
    let Ok(mut file) = std::fs::File::open(path) else { return vec![] };
    let mut read_at = |offset: u64, len: usize| -> Option<Vec<u8>> {
        let mut buf = vec![0; len];
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let be32 = |b: &[u8], at: usize| b.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_be_bytes);
    let Some(head) = read_at(0, 12) else { return vec![] };
    // A collection lists where each face's table directory starts.
    let starts: Vec<u32> = if head.starts_with(b"ttcf") {
        let n = be32(&head, 8).unwrap_or(0).min(MAX_FACES) as usize;
        read_at(12, n * 4).map(|b| b.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect()).unwrap_or_default()
    } else {
        vec![0]
    };
    starts
        .into_iter()
        .filter_map(|start| {
            let dir = read_at(start.into(), 12)?;
            let tables = u16::from_be_bytes(dir.get(4..6)?.try_into().ok()?) as usize;
            let records = read_at(u64::from(start) + 12, tables * 16)?;
            let rec = records.as_chunks::<16>().0.iter().find(|r| r.starts_with(b"name"))?;
            let (offset, len) = (be32(rec, 8)?, be32(rec, 12)?);
            if len > MAX_NAME_TABLE {
                return None;
            }
            let table = read_at(offset.into(), len as usize)?;
            // A one-table font holding just the `name` table, to read it as the font itself would.
            let mut font = Vec::with_capacity(28 + table.len());
            font.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
            font.extend_from_slice(&[0, 1, 0, 16, 0, 0, 0, 0]);
            font.extend_from_slice(b"name");
            for v in [0, 28, len] {
                font.extend_from_slice(&u32::to_be_bytes(v));
            }
            font.extend_from_slice(&table);
            face_names(&skrifa::FontRef::new(&font).ok()?)
        })
        .collect()
}

/// The platform's font folders (the system's and the user's), scanned by [`FontDb::global`].
/// Empty on wasm.
pub fn system_font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(target_arch = "wasm32") {
        return dirs;
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/Library/Fonts"].map(Into::into));
        if let Some(h) = &home {
            dirs.push(h.join("Library/Fonts"));
        }
        // Fonts macOS ships as assets (PingFang, Yu Mincho, …): com_apple_MobileAsset_Font<N>.
        for parent in ["/System/Library/AssetsV2", "/System/Library/AssetsV2/PreinstalledAssetsV2/InstallWithOs"] {
            let Ok(entries) = std::fs::read_dir(parent) else { continue };
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("com_apple_MobileAsset_Font") {
                    dirs.push(e.path());
                }
            }
        }
    } else if cfg!(windows) {
        let root = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        dirs.push(root.join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(std::path::PathBuf::from(l).join("Microsoft\\Windows\\Fonts"));
        }
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(Into::into));
        if let Some(h) = &home {
            dirs.push(h.join(".fonts"));
            dirs.push(h.join(".local/share/fonts"));
        }
    }
    dirs
}

/// One face found in a font file: index, family, style and (variable fonts) the named instance's
/// axis settings.
pub(super) type Found = (u32, String, String, Vec<([u8; 4], f32)>);

/// Parse every face in `data` (a font file or collection); a variable font yields one face per
/// named instance (InDesign lists them as styles).
pub(super) fn enumerate_faces(data: &[u8]) -> Vec<Found> {
    let count = match FileRef::new(data) {
        Ok(FileRef::Font(_)) => 1,
        Ok(FileRef::Collection(c)) => c.len(),
        Err(_) => 0,
    };
    let mut out = Vec::new();
    for i in 0..count {
        let Ok(f) = skrifa::FontRef::from_index(data, i) else { continue };
        let Some((family, style)) = face_names(&f) else { continue };
        let axes = f.axes();
        let mut named = Vec::new();
        for ni in f.named_instances().iter() {
            let Some(style) = name(&f, &[ni.subfamily_name_id()]) else { continue };
            if named.iter().any(|(_, s, _): &(u32, String, _)| s.eq_ignore_ascii_case(&style)) {
                continue;
            }
            let coords: Vec<([u8; 4], f32)> = axes.iter().zip(ni.user_coords()).map(|(a, v)| (a.tag().to_be_bytes(), v)).collect();
            named.push((i, style, coords));
        }
        if named.is_empty() {
            out.push((i, family, style, Vec::new()));
        } else {
            out.extend(named.into_iter().map(|(i, s, c)| (i, family.clone(), s, c)));
        }
    }
    out
}
