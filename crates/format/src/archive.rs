//! The native ZIP archive and its manifest/media entry lifecycle.

use super::{FormatError, MIMETYPE, Result, VERSION, repair};
use deckcraft_model::Presentation;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

/// Largest entry we will inflate (guards against zip bombs).
const MAX_ENTRY: u64 = 2 << 30;

#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    generator: String,
    presentation: Presentation,
}

/// Serialize a presentation.
pub fn save(p: &Presentation) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let w = |e: zip::result::ZipError| FormatError::Write(e.to_string());
        z.start_file("mimetype", stored).map_err(w)?;
        z.write_all(MIMETYPE.as_bytes()).map_err(|e| FormatError::Write(e.to_string()))?;
        let m = Manifest {
            format: "deckcraft".into(),
            version: VERSION,
            generator: format!("DeckCraft {}", env!("CARGO_PKG_VERSION")),
            presentation: p.clone(),
        };
        let json = serde_json::to_vec(&m).map_err(|e| FormatError::Write(e.to_string()))?;
        z.start_file("presentation.json", deflate).map_err(w)?;
        z.write_all(&json).map_err(|e| FormatError::Write(e.to_string()))?;
        for item in &p.media {
            if item.link.is_some() && item.data.is_empty() {
                continue;
            }
            // Already-compressed media is stored, not deflated again.
            let opts = if is_compressed(&item.content_type) { stored } else { deflate };
            z.start_file(format!("media/{}", item.id.0), opts).map_err(w)?;
            z.write_all(&item.data).map_err(|e| FormatError::Write(e.to_string()))?;
        }
        z.finish().map_err(w)?;
    }
    Ok(buf.into_inner())
}

fn is_compressed(ct: &str) -> bool {
    ct.starts_with("image/jpeg")
        || ct.starts_with("image/png")
        || ct.starts_with("video/")
        || ct.starts_with("audio/mpeg")
        || ct.starts_with("audio/mp4")
        || ct.contains("zip")
}

/// Does this look like a `.deckcraft` file?
pub fn sniff(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK") && bytes.windows(MIMETYPE.len()).take(200).any(|w| w == MIMETYPE.as_bytes())
}

fn read_entry(z: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    let f = z.by_name(name).map_err(|e| FormatError::Damaged(format!("{name}: {e}")))?;
    if f.size() > MAX_ENTRY {
        return Err(FormatError::Damaged(format!("{name} is too large")));
    }
    let mut out = Vec::with_capacity(f.size().min(64 << 20) as usize);
    f.take(MAX_ENTRY).read_to_end(&mut out).map_err(|e| FormatError::Damaged(format!("{name}: {e}")))?;
    Ok(out)
}

/// Read a presentation.
pub fn load(bytes: &[u8]) -> Result<Presentation> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| FormatError::NotOurs(e.to_string()))?;
    let mimetype = read_entry(&mut z, "mimetype").map_err(|_| FormatError::NotOurs("no mimetype".into()))?;
    if mimetype != MIMETYPE.as_bytes() {
        return Err(FormatError::NotOurs(String::from_utf8_lossy(&mimetype).into_owned()));
    }
    let json = read_entry(&mut z, "presentation.json").map_err(|_| FormatError::NotOurs("no presentation.json".into()))?;
    let m: Manifest = serde_json::from_slice(&json).map_err(|e| FormatError::Damaged(e.to_string()))?;
    if m.format != "deckcraft" {
        return Err(FormatError::NotOurs(m.format));
    }
    if m.version != VERSION {
        return Err(FormatError::NotOurs(format!("unsupported format version {}", m.version)));
    }
    let mut p = m.presentation;
    for item in &mut p.media {
        if let Ok(data) = read_entry(&mut z, &format!("media/{}", item.id.0)) {
            item.data = Arc::new(data);
        }
    }
    repair(&mut p);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(mimetype: &str, version: u32) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("mimetype", opts).unwrap();
        zip.write_all(mimetype.as_bytes()).unwrap();
        zip.start_file("presentation.json", opts).unwrap();
        let manifest = Manifest { format: "deckcraft".into(), version, generator: "test".into(), presentation: Presentation::default() };
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn rejects_previous_product_format() {
        let bytes = archive("application/vnd.storyteller.slidecraft+zip", VERSION);
        assert!(!sniff(&bytes));
        assert!(matches!(load(&bytes), Err(FormatError::NotOurs(_))));
    }

    #[test]
    fn accepts_only_current_manifest_version() {
        assert!(load(&archive(MIMETYPE, VERSION)).is_ok());
        for version in [0, VERSION + 1] {
            assert!(matches!(load(&archive(MIMETYPE, version)), Err(FormatError::NotOurs(_))));
        }
    }
}
