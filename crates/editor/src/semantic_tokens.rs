use crate::Editor;
use gpui::ViewContext;
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
        println!("{tokens:?}");
        editor.update(&mut cx, |editor, cx| {
            let buffer = editor.buffer.read(cx).buffer(buf_id).unwrap().read(cx);
            buffer.set_text("TESTE", cx);
        });
    }));
    Some(())
}
