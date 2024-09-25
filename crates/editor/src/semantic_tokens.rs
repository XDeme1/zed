use crate::Editor;
use gpui::{rgba, HighlightStyle, ViewContext, ViewInputHandler};
use multi_buffer::Anchor;
use util::ResultExt;

pub(super) fn refresh_semantic_tokens(
    editor: &mut Editor,
    cx: &mut ViewContext<Editor>,
) -> Option<()> {
    let project = editor.project.clone()?;
    let multibuffer = editor.buffer.read(cx);
    let snapshot = multibuffer.snapshot(cx);
    let buffer = multibuffer.buffer(editor.selections.newest_anchor().start.buffer_id?)?;
    editor.semantic_tokens_task = Some(cx.spawn(|editor, mut cx| async move {
        let (buf_id, tokens) = project
            .update(&mut cx, |project, cx| {
                let snapshop = buffer.read(cx).snapshot();
                let buffer_id = snapshop.remote_id();
                let task = project.semantic_tokens(&buffer, cx);
                let tokens = move || async move {
                    let tokens = task.await.log_err().unwrap();
                    tokens
                };
                (buffer_id, tokens())
            })
            .log_err()
            .unwrap();
        let tokens = tokens.await;
        editor.update(&mut cx, |editor, cx| {
            let multibuffer = editor.buffer().read(cx);
            let buffer = multibuffer.buffer(buf_id).unwrap();
            let snapshot = buffer.read(cx).snapshot();
            let mut ranges = Vec::new();
            for token in tokens.tokens {
                for (excerpt_id, excerpt_range) in multibuffer.excerpts_for_buffer(&buffer, cx) {
                    let start = token
                        .range
                        .start
                        .max(&excerpt_range.context.start, &snapshot);
                    let end = token.range.end.min(&excerpt_range.context.end, &snapshot);
                    if start.cmp(&end, &snapshot).is_ge() {
                        continue;
                    }
                    let range = Anchor {
                        buffer_id: Some(buf_id),
                        excerpt_id,
                        text_anchor: start,
                    }..Anchor {
                        buffer_id: Some(buf_id),
                        excerpt_id,
                        text_anchor: end,
                    };
                    ranges.push(range);
                }
            }
            enum TI {}
            editor.highlight_text::<TI>(
                ranges,
                HighlightStyle {
                    background_color: Some(rgba(0xFFFFFFFF).into()),
                    ..Default::default()
                },
                cx,
            );
        });
    }));
    Some(())
}
