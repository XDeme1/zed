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
    let buffers = multibuffer.all_buffers();
    editor.semantic_tokens_task = Some(cx.spawn(|editor, mut cx| async move {
        cx.background_executor()
            .timer(DOCUMENT_HIGHLIGHTS_DEBOUNCE_TIMEOUT)
            .await;

        let tokens = project
            .update(&mut cx, |project, cx| {
                let mut tasks = vec![];
                for buffer in buffers {
                    let snapshot = buffer.read(cx).snapshot();
                    let buffer_id = snapshot.remote_id();
                    let task = project.semantic_tokens(&buffer, cx);
                    let tokens = move || async move {
                        let tokens = task.await.log_err().unwrap();
                        (buffer_id, tokens)
                    };
                    tasks.push(tokens());
                }
                tasks
            })
            .log_err()
            .unwrap();
        let tokens = futures::future::join_all(tokens).await;
        let _ = editor.update(&mut cx, |editor, cx| {
            for (buf_id, semantic_tokens) in tokens {
                let multibuffer = editor.buffer().read(cx);
                let buffer = multibuffer.buffer(buf_id).unwrap();
                let snapshot = buffer.read(cx).snapshot();
                let mut result: BTreeMap<SemanticTokenId, Vec<std::ops::Range<Anchor>>> =
                    BTreeMap::new();
                for token in semantic_tokens.tokens {
                    let token_to_ts = match token.kind.as_str() {
                        "function" => SemanticTokenId::Function,
                        "method" => SemanticTokenId::Method,
                        "variable" => SemanticTokenId::Variable,
                        "property" => SemanticTokenId::Property,
                        "parameter" => SemanticTokenId::Parameter,
                        "macro" => SemanticTokenId::Macro,
                        "enumMember" => SemanticTokenId::EnumMember,
                        "enum" => SemanticTokenId::Enum,
                        "class" => SemanticTokenId::Class,
                        "struct" => SemanticTokenId::Struct,
                        "type" => SemanticTokenId::Type,
                        "typeParameter" => SemanticTokenId::TypeParameter,
                        "comment" => SemanticTokenId::Comment,
                        _ => continue,
                    };
                    let mut ranges = Vec::new();
                    for (excerpt_id, excerpt_range) in multibuffer.excerpts_for_buffer(&buffer, cx)
                    {
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
                    if let Some(a) = result.get_mut(&token_to_ts) {
                        a.append(&mut ranges);
                    } else {
                        result.insert(token_to_ts, ranges);
                    }
                }
                enum SemanticTokenFunction {}
                enum SemanticTokenMethod {}
                enum SemanticTokenProperty {}
                enum SemanticTokenVariable {}
                enum SemanticTokenParameter {}
                enum SemanticTokenMacro {}
                enum SemanticTokenStruct {}
                enum SemanticTokenClass {}
                enum SemanticTokenEnum {}
                enum SemanticTokenEnumMember {}
                enum SemanticTokenType {}
                enum SemanticTokenTypeParameter {}
                enum SemanticTokenComment {}
                for (key, ranges) in result {
                    let hi = editor.style().map(|style| style.syntax.get(key.as_str()));
                    let id = match key {
                        SemanticTokenId::Function => TypeId::of::<SemanticTokenFunction>(),
                        SemanticTokenId::Method => TypeId::of::<SemanticTokenMethod>(),
                        SemanticTokenId::Property => TypeId::of::<SemanticTokenProperty>(),
                        SemanticTokenId::Variable => TypeId::of::<SemanticTokenVariable>(),
                        SemanticTokenId::Parameter => TypeId::of::<SemanticTokenParameter>(),
                        SemanticTokenId::Macro => TypeId::of::<SemanticTokenMacro>(),
                        SemanticTokenId::Struct => TypeId::of::<SemanticTokenStruct>(),
                        SemanticTokenId::Class => TypeId::of::<SemanticTokenClass>(),
                        SemanticTokenId::Enum => TypeId::of::<SemanticTokenEnum>(),
                        SemanticTokenId::EnumMember => TypeId::of::<SemanticTokenEnumMember>(),
                        SemanticTokenId::Type => TypeId::of::<SemanticTokenType>(),
                        SemanticTokenId::TypeParameter => {
                            TypeId::of::<SemanticTokenTypeParameter>()
                        }
                        SemanticTokenId::Comment => TypeId::of::<SemanticTokenComment>(),
                    };
                    hi.map(|hi| {
                        editor.highlight_semantic_text(id, ranges, hi, cx);
                    });
                }
            }
        });
    }));
    Some(())
}
