//! Side-by-side (two column) display of diffs - always two columns.

use std::cmp::{max, min};

use line_numbers::{LineNumber, SingleLineSpan};
use owo_colors::{OwoColorize, Style};

use crate::classify::SemanticMap;
use crate::constants::Side;
use crate::display::context::all_matched_lines_filled;
use crate::display::hunks::{matched_lines_indexes_for_hunk, Hunk};
use crate::display::style::{
    self, apply_line_number_color, color_positions, novel_style, replace_tabs,
    split_and_apply, BackgroundColor,
};
use crate::hash::{DftHashMap, DftHashSet};
use crate::lines::{format_line_num, split_on_newlines};
use crate::options::{DisplayMode, DisplayOptions};
use crate::parse::syntax::{zip_pad_shorter, MatchedPos};
use crate::summary::FileFormat;

/// The space shown between LHS and RHS columns.
const SPACER: &str = "  ";

fn format_line_num_padded(line_num: LineNumber, column_width: usize) -> String {
    format!(
        "{:width$} ",
        line_num.as_usize() + 1,
        width = column_width - 1
    )
}

fn format_missing_line_num(
    prev_num: LineNumber,
    source_dims: &SourceDimensions,
    side: Side,
    is_continuation: bool,
    use_color: bool,
) -> String {
    let column_width = match side {
        Side::Left => source_dims.lhs_line_nums_width,
        Side::Right => source_dims.rhs_line_nums_width,
    };

    let after_end = match side {
        Side::Left => prev_num >= source_dims.lhs_max_line_in_file,
        Side::Right => prev_num >= source_dims.rhs_max_line_in_file,
    };

    let mut style = Style::new();
    if use_color {
        style = style.dimmed();
    }

    let c = if is_continuation {
        "."
    } else if after_end {
        " "
    } else {
        "."
    };

    let num_digits = prev_num.display().len();
    format!(
        "{:>width$} ",
        c.repeat(num_digits),
        width = column_width - 1
    )
    .style(style)
    .to_string()
}

/// Display `src` in a single column (e.g. a file removal or addition).
/// Kept for potential external use, but no longer called internally.
fn _display_single_column(
    display_path: &str,
    old_path: Option<&String>,
    file_format: &FileFormat,
    src_lines: &[String],
    side: Side,
    display_options: &DisplayOptions,
) -> Vec<String> {
    let column_width = format_line_num((src_lines.len() as u32).into()).len();

    let mut formatted_lines = Vec::with_capacity(src_lines.len());

    let mut header_line = String::new();
    header_line.push_str(&style::header(
        display_path,
        old_path,
        1,
        1,
        file_format,
        display_options,
    ));
    header_line.push('\n');
    formatted_lines.push(header_line);

    let mut style = Style::new();
    if display_options.use_color {
        style = novel_style(Style::new(), side, display_options.background_color);
    }

    for (i, line) in src_lines.iter().enumerate() {
        let mut formatted_line = String::with_capacity(line.len());
        formatted_line.push_str(
            &format_line_num_padded((i as u32).into(), column_width)
                .style(style)
                .to_string(),
        );
        formatted_line.push_str(line);
        formatted_lines.push(formatted_line);
    }

    formatted_lines
}

fn display_line_nums(
    lhs_line_num: Option<LineNumber>,
    rhs_line_num: Option<LineNumber>,
    source_dims: &SourceDimensions,
    display_options: &DisplayOptions,
    lhs_has_novel: bool,
    rhs_has_novel: bool,
    prev_lhs_line_num: Option<LineNumber>,
    prev_rhs_line_num: Option<LineNumber>,
) -> (String, String) {
    let display_lhs_line_num: String = match lhs_line_num {
        Some(line_num) => {
            let s = format_line_num_padded(line_num, source_dims.lhs_line_nums_width);
            apply_line_number_color(&s, lhs_has_novel, Side::Left, display_options)
        }
        None => format_missing_line_num(
            prev_lhs_line_num.unwrap_or_else(|| 1.into()),
            source_dims,
            Side::Left,
            false,
            display_options.use_color,
        ),
    };
    let display_rhs_line_num: String = match rhs_line_num {
        Some(line_num) => {
            let s = format_line_num_padded(line_num, source_dims.rhs_line_nums_width);
            apply_line_number_color(&s, rhs_has_novel, Side::Right, display_options)
        }
        None => format_missing_line_num(
            prev_rhs_line_num.unwrap_or_else(|| 1.into()),
            source_dims,
            Side::Right,
            false,
            display_options.use_color,
        ),
    };

    (display_lhs_line_num, display_rhs_line_num)
}

// Sizes used when displaying a hunk.
#[derive(Debug)]
struct SourceDimensions {
    lhs_content_display_width: usize,
    rhs_content_display_width: usize,
    lhs_line_nums_width: usize,
    rhs_line_nums_width: usize,
    lhs_max_line_in_file: LineNumber,
    rhs_max_line_in_file: LineNumber,
}

impl SourceDimensions {
    fn new(
        terminal_width: usize,
        lhs_max_line_visible: LineNumber,
        rhs_max_line_visible: LineNumber,
        lhs_max_line_in_file: LineNumber,
        rhs_max_line_in_file: LineNumber,
        lhs_content_max_width: usize,
        rhs_content_max_width: usize,
    ) -> Self {
        let lhs_line_nums_width = format_line_num(lhs_max_line_visible).len();
        let rhs_line_nums_width = format_line_num(rhs_max_line_visible).len();

        let content_max_width = max(lhs_content_max_width, rhs_content_max_width);
        let content_max_width = max(content_max_width, 25);

        let width_without_truncation = lhs_line_nums_width
            + content_max_width
            + SPACER.len()
            + rhs_line_nums_width
            + content_max_width;
        let display_width = min(terminal_width, width_without_truncation);

        assert!(
            display_width > SPACER.len(),
            "Terminal total width should not overflow"
        );
        let lhs_total_width = (display_width - SPACER.len()) / 2;

        let lhs_content_width = if lhs_line_nums_width < lhs_total_width {
            lhs_total_width - lhs_line_nums_width
        } else {
            1
        };

        let rhs_content_width = max(
            1,
            display_width as isize
                - lhs_total_width as isize
                - SPACER.len() as isize
                - rhs_line_nums_width as isize,
        ) as usize;

        let content_width = min(lhs_content_width, rhs_content_width);

        Self {
            lhs_content_display_width: content_width,
            rhs_content_display_width: content_width,
            lhs_line_nums_width,
            rhs_line_nums_width,
            lhs_max_line_in_file,
            rhs_max_line_in_file,
        }
    }
}

pub(crate) fn lines_with_novel(
    lhs_mps: &[MatchedPos],
    rhs_mps: &[MatchedPos],
) -> (DftHashSet<LineNumber>, DftHashSet<LineNumber>) {
    let lhs_lines_with_novel: DftHashSet<LineNumber> = lhs_mps
        .iter()
        .filter(|mp| mp.kind.is_novel())
        .map(|mp| mp.pos.line)
        .collect();
    let rhs_lines_with_novel: DftHashSet<LineNumber> = rhs_mps
        .iter()
        .filter(|mp| mp.kind.is_novel())
        .map(|mp| mp.pos.line)
        .collect();

    (lhs_lines_with_novel, rhs_lines_with_novel)
}

/// Calculate positions of highlights on both sides.
fn highlight_positions(
    background: BackgroundColor,
    syntax_highlight: bool,
    file_format: &FileFormat,
    lhs_mps: &[MatchedPos],
    rhs_mps: &[MatchedPos],
    lhs_semantic_map: Option<&SemanticMap>,
    rhs_semantic_map: Option<&SemanticMap>,
) -> (
    DftHashMap<LineNumber, Vec<(SingleLineSpan, Style)>>,
    DftHashMap<LineNumber, Vec<(SingleLineSpan, Style)>>,
) {
    let lhs_positions = color_positions(
        Side::Left,
        background,
        syntax_highlight,
        file_format,
        lhs_mps,
        lhs_semantic_map,
    );
    let mut lhs_styles: DftHashMap<LineNumber, Vec<(SingleLineSpan, Style)>> =
        DftHashMap::default();
    for (span, style) in lhs_positions {
        let styles = lhs_styles.entry(span.line).or_insert_with(Vec::new);
        styles.push((span, style));
    }

    let rhs_positions = color_positions(
        Side::Right,
        background,
        syntax_highlight,
        file_format,
        rhs_mps,
        rhs_semantic_map,
    );
    let mut rhs_styles: DftHashMap<LineNumber, Vec<(SingleLineSpan, Style)>> =
        DftHashMap::default();
    for (span, style) in rhs_positions {
        let styles = rhs_styles.entry(span.line).or_insert_with(Vec::new);
        styles.push((span, style));
    }

    (lhs_styles, rhs_styles)
}

fn highlight_as_novel(
    line_num: Option<LineNumber>,
    lines: &[&str],
    opposite_line_num: Option<LineNumber>,
    lines_with_novel: &DftHashSet<LineNumber>,
) -> bool {
    if let Some(line_num) = line_num {
        if lines_with_novel.contains(&line_num) {
            return true;
        }

        let line_content = lines.get(line_num.as_usize()).map(|s| str::trim(s));
        if line_content == Some("") && opposite_line_num.is_none() {
            return true;
        }
    }

    false
}

fn visible_content_max_len_in_bytes(
    lhs_src: &str,
    rhs_src: &str,
    hunks: &[Hunk],
    num_context_lines: u32,
) -> (usize, usize) {
    let mut lhs_displayed_lines: DftHashSet<usize> = DftHashSet::default();
    let mut rhs_displayed_lines: DftHashSet<usize> = DftHashSet::default();

    for hunk in hunks {
        let mut min_lhs_line: Option<LineNumber> = None;
        let mut max_lhs_line: Option<LineNumber> = None;
        let mut min_rhs_line: Option<LineNumber> = None;
        let mut max_rhs_line: Option<LineNumber> = None;

        for (lhs_line, rhs_line) in &hunk.lines {
            if let Some(lhs_line) = lhs_line {
                if let Some(current_min) = min_lhs_line {
                    min_lhs_line = Some(min(current_min, *lhs_line));
                } else {
                    min_lhs_line = Some(*lhs_line);
                }

                if let Some(current_max) = max_lhs_line {
                    max_lhs_line = Some(max(current_max, *lhs_line));
                } else {
                    max_lhs_line = Some(*lhs_line);
                }
            }

            if let Some(rhs_line) = rhs_line {
                if let Some(current_min) = min_rhs_line {
                    min_rhs_line = Some(min(current_min, *rhs_line));
                } else {
                    min_rhs_line = Some(*rhs_line);
                }

                if let Some(current_max) = max_rhs_line {
                    max_rhs_line = Some(max(current_max, *rhs_line));
                } else {
                    max_rhs_line = Some(*rhs_line);
                }
            }

            if let (Some(min_lhs_line), Some(max_lhs_line)) = (min_lhs_line, max_lhs_line) {
                let min_lhs_plus_padding =
                    max(0, min_lhs_line.0 as isize - num_context_lines as isize) as usize;
                let max_lhs_plus_padding = max_lhs_line.0 as usize + num_context_lines as usize;
                for lhs_line_num in min_lhs_plus_padding..=max_lhs_plus_padding {
                    lhs_displayed_lines.insert(lhs_line_num);
                }
            }

            if let (Some(min_rhs_line), Some(max_rhs_line)) = (min_rhs_line, max_rhs_line) {
                let min_rhs_plus_padding =
                    max(0, min_rhs_line.0 as isize - num_context_lines as isize) as usize;
                let max_rhs_plus_padding = max_rhs_line.0 as usize + num_context_lines as usize;
                for rhs_line_num in min_rhs_plus_padding..=max_rhs_plus_padding {
                    rhs_displayed_lines.insert(rhs_line_num);
                }
            }
        }
    }

    let mut lhs_content_max_width: usize = 0;
    let mut rhs_content_max_width: usize = 0;

    for (lhs_i, lhs_line) in lhs_src.lines().enumerate() {
        if lhs_displayed_lines.contains(&lhs_i) {
            lhs_content_max_width = max(lhs_content_max_width, lhs_line.len());
        }
    }
    for (rhs_i, rhs_line) in rhs_src.lines().enumerate() {
        if rhs_displayed_lines.contains(&rhs_i) {
            rhs_content_max_width = max(rhs_content_max_width, rhs_line.len());
        }
    }

    (lhs_content_max_width, rhs_content_max_width)
}

pub(crate) fn print(
    hunks: &[Hunk],
    display_options: &DisplayOptions,
    display_path: &str,
    old_path: Option<&String>,
    file_format: &FileFormat,
    lhs_src: &str,
    rhs_src: &str,
    lhs_mps: &[MatchedPos],
    rhs_mps: &[MatchedPos],
    lhs_semantic_map: Option<&SemanticMap>,
    rhs_semantic_map: Option<&SemanticMap>,
) {
    let (lhs_content_max_width, rhs_content_max_width) = visible_content_max_len_in_bytes(
        lhs_src,
        rhs_src,
        hunks,
        display_options.num_context_lines,
    );

    let (lhs_highlights, rhs_highlights) = if display_options.use_color {
        highlight_positions(
            display_options.background_color,
            display_options.syntax_highlight,
            file_format,
            lhs_mps,
            rhs_mps,
            lhs_semantic_map,
            rhs_semantic_map,
        )
    } else {
        (DftHashMap::default(), DftHashMap::default())
    };

    let (lhs_lines_with_novel, rhs_lines_with_novel) = lines_with_novel(lhs_mps, rhs_mps);

    let mut prev_lhs_line_num = None;
    let mut prev_rhs_line_num = None;

    let mut lhs_lines = split_on_newlines(lhs_src).collect::<Vec<_>>();
    let mut rhs_lines = split_on_newlines(rhs_src).collect::<Vec<_>>();

    if lhs_lines.last() == Some(&"") && lhs_lines.len() > 1 {
        lhs_lines.pop();
    }
    if rhs_lines.last() == Some(&"") && rhs_lines.len() > 1 {
        rhs_lines.pop();
    }

    let matched_lines = all_matched_lines_filled(lhs_mps, rhs_mps, &lhs_lines, &rhs_lines);
    let mut matched_lines_to_print = &matched_lines[..];

    let mut lhs_max_visible_line = 1.into();
    let mut rhs_max_visible_line = 1.into();

    if let Some(hunk) = hunks.last() {
        let (start_i, end_i) = matched_lines_indexes_for_hunk(
            matched_lines_to_print,
            hunk,
            display_options.num_context_lines as usize,
        );
        let aligned_lines = &matched_lines_to_print[start_i..end_i];

        for (lhs_line_num, rhs_line_num) in aligned_lines.iter().rev() {
            if let Some(lhs_line_num) = *lhs_line_num {
                lhs_max_visible_line = max(lhs_max_visible_line, lhs_line_num);
            }
            if let Some(rhs_line_num) = *rhs_line_num {
                rhs_max_visible_line = max(rhs_max_visible_line, rhs_line_num);
            }

            if lhs_max_visible_line > 1.into() && rhs_max_visible_line > 1.into() {
                break;
            }
        }
    }

    let lhs_max_line_in_file = LineNumber(lhs_lines.len().saturating_sub(1) as u32);
    let rhs_max_line_in_file = LineNumber(rhs_lines.len().saturating_sub(1) as u32);

    lhs_max_visible_line = LineNumber(min(
        lhs_max_visible_line.0 + display_options.num_context_lines,
        lhs_max_line_in_file.0,
    ));
    rhs_max_visible_line = LineNumber(min(
        rhs_max_visible_line.0 + display_options.num_context_lines,
        rhs_max_line_in_file.0,
    ));

    let source_dims = SourceDimensions::new(
        display_options.terminal_width,
        lhs_max_visible_line,
        rhs_max_visible_line,
        lhs_max_line_in_file,
        rhs_max_line_in_file,
        lhs_content_max_width,
        rhs_content_max_width,
    );

    for (i, hunk) in hunks.iter().enumerate() {
        println!(
            "{}",
            style::header(
                display_path,
                old_path,
                i + 1,
                hunks.len(),
                file_format,
                display_options
            )
        );

        let (start_i, end_i) = matched_lines_indexes_for_hunk(
            matched_lines_to_print,
            hunk,
            display_options.num_context_lines as usize,
        );
        let aligned_lines = &matched_lines_to_print[start_i..end_i];
        matched_lines_to_print = &matched_lines_to_print[start_i..];

        // REMOVED: no_lhs_changes/no_rhs_changes short-circuit logic
        // Always render side-by-side regardless of change distribution

        for (lhs_line_num, rhs_line_num) in aligned_lines {
            let lhs_line_novel = highlight_as_novel(
                *lhs_line_num,
                &lhs_lines,
                *rhs_line_num,
                &lhs_lines_with_novel,
            );
            let rhs_line_novel = highlight_as_novel(
                *rhs_line_num,
                &rhs_lines,
                *lhs_line_num,
                &rhs_lines_with_novel,
            );

            let (display_lhs_line_num, display_rhs_line_num) = display_line_nums(
                *lhs_line_num,
                *rhs_line_num,
                &source_dims,
                display_options,
                lhs_line_novel,
                rhs_line_novel,
                prev_lhs_line_num,
                prev_rhs_line_num,
            );

            // ALWAYS use side-by-side rendering path
            let lhs_line = match lhs_line_num {
                Some(lhs_line_num) => split_and_apply(
                    lhs_lines[lhs_line_num.as_usize()],
                    source_dims.lhs_content_display_width,
                    display_options.tab_width,
                    lhs_highlights.get(lhs_line_num).unwrap_or(&vec![]),
                    Side::Left,
                ),
                None => vec![" ".repeat(source_dims.lhs_content_display_width)],
            };
            let rhs_line = match rhs_line_num {
                Some(rhs_line_num) => split_and_apply(
                    rhs_lines[rhs_line_num.as_usize()],
                    source_dims.rhs_content_display_width,
                    display_options.tab_width,
                    rhs_highlights.get(rhs_line_num).unwrap_or(&vec![]),
                    Side::Right,
                ),
                None => vec![" ".repeat(source_dims.rhs_content_display_width)], // Fixed: was empty string
            };

            for (i, (lhs_line, rhs_line)) in zip_pad_shorter(&lhs_line, &rhs_line)
                .into_iter()
                .enumerate()
            {
                let lhs_line = lhs_line
                    .unwrap_or_else(|| " ".repeat(source_dims.lhs_content_display_width));
                let rhs_line = rhs_line
                    .unwrap_or_else(|| " ".repeat(source_dims.rhs_content_display_width)); // Fixed: was empty

                let lhs_num: String = if i == 0 {
                    display_lhs_line_num.clone()
                } else {
                    let mut s = format_missing_line_num(
                        lhs_line_num
                            .unwrap_or_else(|| prev_lhs_line_num.unwrap_or_else(|| 10.into())),
                        &source_dims,
                        Side::Left,
                        true,
                        display_options.use_color,
                    );
                    if let Some(line_num) = lhs_line_num {
                        s = apply_line_number_color(
                            &s,
                            lhs_lines_with_novel.contains(line_num),
                            Side::Left,
                            display_options,
                        );
                    }
                    s
                };
                let rhs_num: String = if i == 0 {
                    display_rhs_line_num.clone()
                } else {
                    let mut s = format_missing_line_num(
                        rhs_line_num
                            .unwrap_or_else(|| prev_rhs_line_num.unwrap_or_else(|| 10.into())),
                        &source_dims,
                        Side::Right,
                        true,
                        display_options.use_color,
                    );
                    if let Some(line_num) = rhs_line_num {
                        s = apply_line_number_color(
                            &s,
                            rhs_lines_with_novel.contains(line_num),
                            Side::Right,
                            display_options,
                        );
                    }
                    s
                };

                println!("{}{}{}{}{}", lhs_num, lhs_line, SPACER, rhs_num, rhs_line);
            }

            if lhs_line_num.is_some() {
                prev_lhs_line_num = *lhs_line_num;
            }
            if rhs_line_num.is_some() {
                prev_rhs_line_num = *rhs_line_num;
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::options::DEFAULT_TERMINAL_WIDTH;
    use crate::parse::guess_language::Language;
    use crate::syntax::{AtomKind, MatchKind, TokenKind};

    #[test]
    fn test_width_calculations() {
        let source_dims = SourceDimensions::new(
            DEFAULT_TERMINAL_WIDTH,
            1.into(),
            10.into(),
            1.into(),
            10.into(),
            9999,
            9999,
        );

        assert_eq!(source_dims.lhs_line_nums_width, 2);
        assert_eq!(source_dims.rhs_line_nums_width, 3);
    }

    #[test]
    fn test_format_missing_line_num() {
        let source_dims = SourceDimensions::new(
            DEFAULT_TERMINAL_WIDTH,
            1.into(),
            1.into(),
            1.into(),
            1.into(),
            9999,
            9999,
        );

        assert_eq!(
            format_missing_line_num(0.into(), &source_dims, Side::Left, false, true),
            ". ".dimmed().to_string()
        );
        assert_eq!(
            format_missing_line_num(0.into(), &source_dims, Side::Left, false, false),
            ". ".to_owned()
        );
    }

    #[test]
    fn test_format_missing_line_num_at_end() {
        let source_dims =
            SourceDimensions::new(80, 1.into(), 1.into(), 1.into(), 1.into(), 9999, 9999);

        assert_eq!(
            format_missing_line_num(1.into(), &source_dims, Side::Left, false, true),
            "  ".dimmed().to_string()
        );
        assert_eq!(
            format_missing_line_num(1.into(), &source_dims, Side::Left, false, false),
            "  ".to_owned()
        );
    }

    #[test]
    fn test_display_single_column() {
        let res_lines = _display_single_column(
            "foo.py",
            None,
            &FileFormat::SupportedLanguage(Language::Python),
            &["print(123)\n".to_owned()],
            Side::Right,
            &DisplayOptions::default(),
        );
        let res = res_lines.join("");
        assert!(res.len() > 10);
    }

    #[test]
    fn test_display_hunks() {
        let lhs_mps = [MatchedPos {
            kind: MatchKind::Novel {
                highlight: TokenKind::Atom(AtomKind::Normal),
            },
            pos: SingleLineSpan {
                line: 0.into(),
                start_col: 0,
                end_col: 3,
            },
        }];

        let rhs_mps = [MatchedPos {
            kind: MatchKind::Novel {
                highlight: TokenKind::Atom(AtomKind::Normal),
            },
            pos: SingleLineSpan {
                line: 0.into(),
                start_col: 0,
                end_col: 3,
            },
        }];

        let mut novel_lhs = DftHashSet::default();
        novel_lhs.insert(0.into());
        let mut novel_rhs = DftHashSet::default();
        novel_rhs.insert(0.into());

        let hunks = [Hunk {
            novel_lhs,
            novel_rhs,
            lines: vec![(Some(0.into()), Some(0.into()))],
        }];

        print(
            &hunks,
            &DisplayOptions::default(),
            "foo-new.el",
            None,
            &FileFormat::SupportedLanguage(Language::EmacsLisp),
            "foo",
            "bar",
            &lhs_mps,
            &rhs_mps,
            None,
            None,
        );
    }
}