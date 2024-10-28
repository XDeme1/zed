use std::{
    cmp::{Ordering, Reverse},
    iter,
    ops::Range,
};

use sum_tree::SumTree;
use text::{Anchor, Bias, FromAnchor, PointUtf16, ToOffset};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticToken {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTokenEntry<T> {
    range: Range<T>,
    kind: SemanticToken,
}

#[derive(Clone, Debug)]
pub struct SemanticTokenSet {
    semantic_tokens: SumTree<SemanticTokenEntry<Anchor>>,
}

#[derive(Clone, Debug)]
pub struct Summary {
    start: Anchor,
    end: Anchor,
    min_start: Anchor,
    max_end: Anchor,
    count: usize,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            start: Anchor::MIN,
            end: Anchor::MAX,
            min_start: Anchor::MAX,
            max_end: Anchor::MIN,
            count: 0,
        }
    }
}

impl sum_tree::Summary for Summary {
    type Context = text::BufferSnapshot;

    fn zero(_: &Self::Context) -> Self {
        Default::default()
    }

    fn add_summary(&mut self, other: &Self, buffer: &Self::Context) {
        if other.min_start.cmp(&self.min_start, buffer).is_lt() {
            self.min_start = other.min_start;
        }
        if other.max_end.cmp(&self.max_end, buffer).is_gt() {
            self.max_end = other.max_end;
        }
        self.start = other.start;
        self.end = other.end;
        self.count += other.count;
    }
}

impl sum_tree::Item for SemanticTokenEntry<Anchor> {
    type Summary = Summary;

    fn summary(&self) -> Self::Summary {
        Summary {
            start: self.range.start,
            end: self.range.end,
            min_start: self.range.start,
            max_end: self.range.end,
            count: 1,
        }
    }
}

impl SemanticTokenEntry<Anchor> {
    /// Converts the [DiagnosticEntry] to a different buffer coordinate type.
    pub fn resolve<O: FromAnchor>(&self, buffer: &text::BufferSnapshot) -> SemanticTokenEntry<O> {
        SemanticTokenEntry {
            range: O::from_anchor(&self.range.start, buffer)
                ..O::from_anchor(&self.range.end, buffer),
            kind: self.kind.clone(),
        }
    }
}

impl SemanticTokenSet {
    pub fn from_sorted_entries<I>(iter: I, buffer: &text::BufferSnapshot) -> Self
    where
        I: IntoIterator<Item = SemanticTokenEntry<Anchor>>,
    {
        Self {
            semantic_tokens: SumTree::from_iter(iter, buffer),
        }
    }

    pub fn new<I>(iter: I, buffer: &text::BufferSnapshot) -> Self
    where
        I: IntoIterator<Item = SemanticTokenEntry<PointUtf16>>,
    {
        let mut entries = iter.into_iter().collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| (entry.range.start, Reverse(entry.range.end)));
        Self {
            semantic_tokens: SumTree::from_iter(
                entries.into_iter().map(|entry| SemanticTokenEntry {
                    range: buffer.anchor_before(entry.range.start)
                        ..buffer.anchor_before(entry.range.end),
                    kind: entry.kind,
                }),
                buffer,
            ),
        }
    }
    pub fn range<'a, T, O>(
        &'a self,
        range: Range<T>,
        buffer: &'a text::BufferSnapshot,
        inclusive: bool,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = SemanticTokenEntry<O>>
    where
        T: 'a + ToOffset,
        O: FromAnchor,
    {
        let end_bias = if inclusive { Bias::Right } else { Bias::Left };
        let range = buffer.anchor_before(range.start)..buffer.anchor_at(range.end, end_bias);
        let mut cursor = self.semantic_tokens.filter::<_, ()>(buffer, {
            move |summary: &Summary| {
                let start_cmp = range.start.cmp(&summary.max_end, buffer);
                let end_cmp = range.end.cmp(&summary.min_start, buffer);
                if inclusive {
                    start_cmp <= Ordering::Equal && end_cmp >= Ordering::Equal
                } else {
                    start_cmp == Ordering::Less && end_cmp == Ordering::Greater
                }
            }
        });

        if reversed {
            cursor.prev(buffer);
        } else {
            cursor.next(buffer);
        }
        iter::from_fn({
            move || {
                if let Some(semantic_token) = cursor.item() {
                    if reversed {
                        cursor.prev(buffer);
                    } else {
                        cursor.next(buffer);
                    }
                    Some(semantic_token.resolve(buffer))
                } else {
                    None
                }
            }
        })
    }
}
