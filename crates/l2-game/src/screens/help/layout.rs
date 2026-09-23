use l2_formats::hlp::Topic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub x: i32,
    pub w: i32,
    pub text: String,
    /// The `|CONTEXT` hash of the hotspot this word is part of.
    pub jump: Option<u32>,
}

pub type Row = Vec<Word>;

/// Word-wraps a topic's lines to `width`; a blank row after each paragraph [D].
pub fn layout(topic: &Topic, width: i32, measure: &dyn Fn(&str) -> i32) -> Vec<Row> {
    let space = measure(" ").max(4);
    let mut rows = Vec::new();
    for line in &topic.lines {
        let (mut row, mut x): (Row, i32) = (Vec::new(), 0);
        for span in &line.spans {
            for text in span.text.split_whitespace() {
                let w = measure(text);
                if !row.is_empty() && x + w > width {
                    rows.push(std::mem::take(&mut row));
                    x = 0;
                }
                row.push(Word { x, w, text: text.to_string(), jump: span.jump });
                x += w + space;
            }
        }
        rows.push(row);
        rows.push(Vec::new());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_formats::hlp::{Line, Span};

    /// Ablate the `x + w > width` wrap and the second word stays on row 0.
    #[test]
    fn a_hotspot_keeps_its_hash_across_a_wrap() {
        let span = |t: &str, jump| Span { text: t.into(), jump };
        let topic = Topic {
            title: "T".into(),
            lines: vec![Line { spans: vec![span("aaaa ", None), span("bbbb cc", Some(7))] }],
        };
        let rows = layout(&topic, 10, &|s: &str| s.len() as i32);
        let words: Vec<(usize, &str, Option<u32>)> = rows
            .iter()
            .enumerate()
            .flat_map(|(r, row)| row.iter().map(move |w| (r, w.text.as_str(), w.jump)))
            .collect();
        assert_eq!(words, vec![(0, "aaaa", None), (1, "bbbb", Some(7)), (1, "cc", Some(7))]);
    }
}
