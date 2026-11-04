//! A selection of notes as a `.zip`, for someone without ccdash: markdown
//! files with the frontmatter `ccdash export` writes, and an `INDEX.md` so
//! their Claude can read it like a memory directory. A `[[link]]` to a note
//! outside the selection would lead nowhere, so it becomes that note's title.

use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use anyhow::Result;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;
use ts_rs::TS;

use super::{Note, NoteKind, frontmatter, get, wiki_links};

/// Notes in the order `INDEX.md` lists them, with their headings.
const SECTIONS: [(NoteKind, &str); 5] = [
    (NoteKind::Memory, "Memories"),
    (NoteKind::Doc, "Documents"),
    (NoteKind::Report, "Reports"),
    (NoteKind::Issue, "Issues"),
    (NoteKind::Progress, "Progress"),
];

/// A note to export.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct NoteRef {
    pub workspace: String,
    pub slug: String,
}

/// The archive of `refs`; `None` when one of them does not exist. A
/// report's HTML (under `reports`, the hub's directory) goes next to its
/// note.
pub fn archive(
    conn: &Connection,
    refs: &[NoteRef],
    reports: Option<&Path>,
) -> Result<Option<Vec<u8>>> {
    let mut notes = Vec::new();
    for r in refs {
        match get(conn, &r.workspace, &r.slug)? {
            Some(note) => notes.push(note),
            None => return Ok(None),
        }
    }
    let workspaces: HashSet<&str> = notes.iter().map(|n| n.workspace.as_str()).collect();
    let kept: HashSet<(&str, &str)> = notes
        .iter()
        .map(|n| (n.workspace.as_str(), n.slug.as_str()))
        .collect();
    let path = |n: &Note| {
        let file = format!("{}/{}.md", n.kind.name(), n.slug);
        // NOTE: a slug is only unique within its workspace
        if workspaces.len() > 1 {
            format!("{}/{file}", n.workspace)
        } else {
            file
        }
    };
    let mut files = vec![("INDEX.md".to_owned(), index(&notes, path).into_bytes())];
    for note in &notes {
        let body = spell_links(
            &note.body,
            |slug| kept.contains(&(note.workspace.as_str(), slug)),
            |slug| {
                get(conn, &note.workspace, slug)
                    .ok()
                    .flatten()
                    .map(|n| n.title)
            },
        );
        // NOTE: `extra` is the hub's bookkeeping: who closed it, which session
        let shared = Note {
            body,
            extra: json!({}),
            ..note.clone()
        };
        files.push((path(note), frontmatter(&shared).into_bytes()));
        if note.kind == NoteKind::Report
            && let Some(dir) = reports
            && let Some(html) = crate::hub::reports::export_file(dir, &note.workspace, &note.slug)
        {
            let name = path(note).trim_end_matches(".md").to_owned() + ".html";
            files.push((name, html));
        }
    }
    Ok(Some(zip(&files)))
}

fn index(notes: &[Note], path: impl Fn(&Note) -> String) -> String {
    let mut by_kind: BTreeMap<&str, Vec<&Note>> = BTreeMap::new();
    for note in notes {
        by_kind.entry(note.kind.name()).or_default().push(note);
    }
    let mut out = format!(
        "# Notes\n\n{} exported from ccdash. A `[[slug]]` link names the file `<slug>.md` here.\n",
        notes.len()
    );
    for (kind, heading) in SECTIONS {
        let Some(list) = by_kind.get(kind.name()) else {
            continue;
        };
        out.push_str(&format!("\n## {heading}\n\n"));
        for note in list {
            let card = note.card();
            let line = card.summary.as_deref().unwrap_or(card.excerpt.as_str());
            out.push_str(&format!("- [{}]({})", note.title, path(note)));
            if !line.is_empty() {
                out.push_str(&format!(": {line}"));
            }
            out.push('\n');
        }
    }
    out
}

/// `body` with each `[[link]]` that `keep` turns down spelled out: its
/// label, else the title `title_of` finds, else the slug.
fn spell_links(
    body: &str,
    keep: impl Fn(&str) -> bool,
    title_of: impl Fn(&str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut from = 0;
    for (slug, at) in wiki_links(body) {
        let Some(len) = body[at..].find("]]") else {
            continue;
        };
        let end = at + len + 2;
        out.push_str(&body[from..at]);
        if keep(&slug) {
            out.push_str(&body[at..end]);
        } else {
            let label = body[at + 2..end - 2]
                .split_once('|')
                .map(|(_, label)| label.trim())
                .filter(|label| !label.is_empty());
            match label {
                Some(label) => out.push_str(label),
                None => out.push_str(&title_of(&slug).unwrap_or(slug)),
            }
        }
        from = end;
    }
    out.push_str(&body[from..]);
    out
}

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL: u32 = 0x0605_4b50;
const VERSION: u16 = 20;
/// The names are UTF-8.
const FLAGS: u16 = 0x0800;
/// 1980-01-01, the format's first day: what matters is in the frontmatter.
const DOS_DATE: u16 = 0x0021;

/// A `.zip` of `files`, stored rather than deflated: a few kilobytes of text
/// each, and every system opens it. No entry or total can pass 4 GiB here.
fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let shared = |to: &mut Vec<u8>| {
            for field in [VERSION, FLAGS, 0, 0, DOS_DATE] {
                to.extend(field.to_le_bytes());
            }
            for field in [crc, data.len() as u32, data.len() as u32] {
                to.extend(field.to_le_bytes());
            }
            to.extend((name.len() as u16).to_le_bytes());
            to.extend(0u16.to_le_bytes());
        };
        out.extend(LOCAL_HEADER.to_le_bytes());
        shared(&mut out);
        out.extend(name.as_bytes());
        out.extend(data);

        central.extend(CENTRAL_HEADER.to_le_bytes());
        central.extend(VERSION.to_le_bytes());
        shared(&mut central);
        // comment, disk, internal and external attributes
        central.extend([0u8; 10]);
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend(&central);
    out.extend(END_OF_CENTRAL.to_le_bytes());
    out.extend([0u8; 4]);
    for _ in 0..2 {
        out.extend((files.len() as u16).to_le_bytes());
    }
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |crc, byte| {
        (0..8).fold(crc ^ u32::from(*byte), |c, _| {
            if c & 1 == 1 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_out_of_the_selection_are_spelled_out() {
        let keep = |slug: &str| slug == "here";
        let title = |slug: &str| (slug == "there").then(|| "The other note".to_owned());
        assert_eq!(
            spell_links(
                "see [[here]], [[there]], [[there|that one]] and [[gone]]; [[Not A Slug]] stays",
                keep,
                title
            ),
            "see [[here]], The other note, that one and gone; [[Not A Slug]] stays"
        );
        assert_eq!(spell_links("no links", keep, title), "no links");
        assert_eq!(spell_links("", keep, title), "");
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn the_archive_has_one_entry_per_file() {
        let data = zip(&[
            ("a.md".into(), b"one".to_vec()),
            ("b/č.md".into(), b"two".to_vec()),
        ]);
        assert_eq!(&data[..4], &LOCAL_HEADER.to_le_bytes());
        let end = data.len() - 22;
        assert_eq!(&data[end..end + 4], &END_OF_CENTRAL.to_le_bytes());
        assert_eq!(u16::from_le_bytes([data[end + 10], data[end + 11]]), 2);
    }
}
