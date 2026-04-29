//! Y.Doc lifecycle + `.md` ↔ CRDT bridging.
//!
//! Top-level pure helpers ([`create_doc`], [`apply_update_to_doc`],
//! [`fill_doc_with_markdown`], [`replace_doc_content`]) sit here so both
//! [`manager::YDocManager`] and [`doc_state`]'s hydrate/merge routines can
//! share them without cyclic deps.

pub mod doc_state;
pub mod manager;

use yrs::{Doc, GetString, OffsetKind, Options, Text, TextRef, Transact};

use crate::error::{AppError, AppResult};

/// Y.Text fragment name shared by all modules.
///
/// Must match the frontend `@swarmnote/editor` default fragment name (see
/// `packages/editor/src/createEditor.ts`).
pub(crate) const FRAGMENT_NAME: &str = "document";

/// Create an empty Y.Doc with a pre-registered Y.Text fragment and Utf16
/// offset. All Y.Docs **must** use `OffsetKind::Utf16` to match JavaScript
/// yjs — the yrs default (`OffsetKind::Bytes`) overflows on CJK characters.
pub fn create_doc() -> Doc {
    let doc = Doc::with_options(Options {
        offset_kind: OffsetKind::Utf16,
        ..Default::default()
    });
    doc.get_or_insert_text(FRAGMENT_NAME);
    doc
}

/// Decode a v1 binary update and apply it to a Y.Doc.
pub fn apply_update_to_doc(doc: &Doc, data: &[u8], context: &'static str) -> AppResult<()> {
    let update =
        yrs::updates::decoder::Decode::decode_v1(data).map_err(|e| AppError::YjsDecode {
            context,
            reason: e.to_string(),
        })?;
    doc.transact_mut()
        .apply_update(update)
        .map_err(|e| AppError::YjsApply {
            context,
            reason: e.to_string(),
        })?;
    Ok(())
}

/// Compute blake3 hash of text content for the `file_hash` DB column.
pub fn content_hash(text: &str) -> Vec<u8> {
    blake3::hash(text.as_bytes()).as_bytes().to_vec()
}

/// Read the current Markdown content from a Y.Doc's Y.Text fragment.
pub fn doc_to_markdown(doc: &Doc) -> String {
    let text: TextRef = doc.get_or_insert_text(FRAGMENT_NAME);
    let txn = doc.transact();
    text.get_string(&txn)
}

/// Fill an empty Y.Doc with the given Markdown content.
///
/// Must only be called on a freshly created Doc (empty Y.Text). If the
/// Y.Text already has content, this is a no-op — caller should use
/// [`replace_doc_content`] instead.
pub fn fill_doc_with_markdown(doc: &Doc, md: &str) {
    let text = doc.get_or_insert_text(FRAGMENT_NAME);
    let mut txn = doc.transact_mut();
    if text.len(&txn) != 0 {
        return;
    }
    text.insert(&mut txn, 0, md);
}

/// Mutate the Y.Doc's Y.Text so it ends up matching `new_md`, using minimal
/// insert/delete ops that preserve CRDT history (concurrent edits are
/// merged, not overwritten).
///
/// Diff is computed at the UTF-16 code unit level, matching the Y.Text
/// offset semantics (`OffsetKind::Utf16`).
pub fn replace_doc_content(doc: &Doc, new_md: &str) {
    let text = doc.get_or_insert_text(FRAGMENT_NAME);
    let mut txn = doc.transact_mut();
    let old = text.get_string(&txn);
    if old == new_md {
        return;
    }

    let old_u16: Vec<u16> = old.encode_utf16().collect();
    let new_u16: Vec<u16> = new_md.encode_utf16().collect();

    let ops = similar::capture_diff_slices(similar::Algorithm::Myers, &old_u16, &new_u16);

    // Walk in reverse so earlier offsets remain valid as the Y.Text mutates.
    for op in ops.iter().rev() {
        match *op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                text.remove_range(&mut txn, old_index as u32, old_len as u32);
            }
            similar::DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                let chunk = &new_u16[new_index..new_index + new_len];
                let s = String::from_utf16_lossy(chunk);
                text.insert(&mut txn, old_index as u32, &s);
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                text.remove_range(&mut txn, old_index as u32, old_len as u32);
                let chunk = &new_u16[new_index..new_index + new_len];
                let s = String::from_utf16_lossy(chunk);
                text.insert(&mut txn, old_index as u32, &s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::ReadTxn;

    fn doc_text(doc: &Doc) -> String {
        doc_to_markdown(doc)
    }

    #[test]
    fn fill_empty_doc() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "# Hello\n");
        assert_eq!(doc_text(&doc), "# Hello\n");
    }

    #[test]
    fn fill_is_idempotent_when_nonempty() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "first");
        fill_doc_with_markdown(&doc, "second");
        assert_eq!(doc_text(&doc), "first");
    }

    #[test]
    fn replace_with_identical_content_is_noop() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "hello");
        let sv_before = doc.transact().state_vector();
        replace_doc_content(&doc, "hello");
        let sv_after = doc.transact().state_vector();
        assert_eq!(sv_before, sv_after);
    }

    #[test]
    fn replace_appends_content() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "hello");
        replace_doc_content(&doc, "hello world");
        assert_eq!(doc_text(&doc), "hello world");
    }

    #[test]
    fn replace_deletes_content() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "hello world");
        replace_doc_content(&doc, "hello");
        assert_eq!(doc_text(&doc), "hello");
    }

    #[test]
    fn replace_middle_change() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "abcdef");
        replace_doc_content(&doc, "aXcef");
        assert_eq!(doc_text(&doc), "aXcef");
    }

    #[test]
    fn replace_handles_cjk() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "你好世界");
        replace_doc_content(&doc, "你好新世界");
        assert_eq!(doc_text(&doc), "你好新世界");
    }

    #[test]
    fn replace_full_rewrite() {
        let doc = create_doc();
        fill_doc_with_markdown(&doc, "old content");
        replace_doc_content(&doc, "completely new");
        assert_eq!(doc_text(&doc), "completely new");
    }

    #[test]
    fn concurrent_edits_merge_via_replace() {
        // Simulate two devices: A edits locally, B receives an external file
        // change. The diff-based replace must not wipe A's concurrent edit.
        let doc_a = create_doc();
        fill_doc_with_markdown(&doc_a, "hello");

        let sv = doc_a.transact().state_vector();
        let update_a = doc_a
            .transact()
            .encode_state_as_update_v1(&yrs::StateVector::default());

        let doc_b = create_doc();
        apply_update_to_doc(&doc_b, &update_a, "clone").unwrap();

        // A appends " world"
        {
            let text = doc_a.get_or_insert_text(FRAGMENT_NAME);
            let mut txn = doc_a.transact_mut();
            let len = text.len(&txn);
            text.insert(&mut txn, len, " world");
        }
        let update_from_a = doc_a.transact().encode_state_as_update_v1(&sv);

        // B receives external file change: "hello!"
        replace_doc_content(&doc_b, "hello!");

        // A's update arrives at B
        apply_update_to_doc(&doc_b, &update_from_a, "from A").unwrap();

        // Both edits should survive.
        let merged = doc_to_markdown(&doc_b);
        assert!(merged.contains("world"), "merged: {merged}");
        assert!(merged.contains('!'), "merged: {merged}");
    }
}
