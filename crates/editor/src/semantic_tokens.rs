use std::{any::TypeId, collections::BTreeMap};

use crate::{Editor, SemanticTokenId, DOCUMENT_HIGHLIGHTS_DEBOUNCE_TIMEOUT};
use gpui::ViewContext;
use multi_buffer::Anchor;
use util::ResultExt;

pub(super) fn refresh_semantic_tokens(
    editor: &mut Editor,
    cx: &mut ViewContext<Editor>,
) -> Option<()> {
    let project = editor.project.clone()?;
    let multibuffer = editor.buffer.read(cx);
    let snapshot = multibuffer.snapshot(cx);
    let mut buffers = vec![];
    for selection in editor.selections.all::<usize>(cx) {
        let start_position = snapshot.anchor_before(selection.head());
        let end_position = snapshot.anchor_after(selection.tail());
        if start_position.buffer_id != end_position.buffer_id || end_position.buffer_id.is_none() {
            // Throw away selections spanning multiple buffers.
            continue;
        }
        if let Some(buffer) = end_position.buffer_id.and_then(|id| multibuffer.buffer(id)) {
            buffers.push(buffer);
        }
    }
    editor.semantic_tokens_task = Some(cx.spawn(|editor, mut cx| async move {
        cx.background_executor()
            .timer(DOCUMENT_HIGHLIGHTS_DEBOUNCE_TIMEOUT)
            .await;
        let tokens = project
            .update(&mut cx, |project, cx| {
                let mut tasks = vec![];
                for buffer in buffers {
                    let snapshot = buffer.read(cx).snapshot();
                    let id = snapshot.remote_id();
                    let task = project.semantic_tokens(&buffer, cx);
                    let tokens = move || async move {
                        let tokens = task.await.log_err()?;
                        Some((id, tokens))
                    };
                    tasks.push(tokens());
                }
                tasks
            })
            .log_err()?;

        let _tokens = futures::future::join_all(tokens).await;
        Some(())
    }));
    Some(())
}
