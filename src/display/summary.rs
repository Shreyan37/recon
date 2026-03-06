//! Summary display for --summarize mode.
//!
//! Prints a structured breakdown of behavioral vs cosmetic changes per file,
//! rather than the full side-by-side or inline diff.

use crate::classify::ChangeSpan;
use crate::display::style;
use crate::options::DisplayOptions;
use crate::summary::{DiffResult, FileFormat};
use owo_colors::OwoColorize;

/// Format a `ChangeSpan` for display.
///
/// Single-line: "Line 7 (col 4-38)"
/// Multi-line:  "Lines 7-9"
///
/// For multi-line spans we omit the column range because the start_col/end_col
/// only reflect the first line and showing them for a 3-line deletion would
/// be misleading.
fn format_span(span: &ChangeSpan) -> String {
    // Line numbers are 0-indexed internally; add 1 for display.
    let start = span.start_line + 1;
    let end = span.end_line + 1;

    if start == end {
        format!("Line {} (col {}-{})", start, span.start_col, span.end_col)
    } else {
        format!("Lines {}-{}", start, end)
    }
}

pub fn print_summary(diff_result: &DiffResult, display_options: &DisplayOptions) {
    // Mirror the normal path's skip-unchanged behaviour so that directory
    // diffs with --summarize don't emit a block for every unchanged file.
    if !diff_result.has_syntactic_changes && !display_options.print_unchanged {
        return;
    }

    // style::header signature (from style.rs):
    //   header(display_path: &str, extra_info: Option<&String>,
    //          hunk_num: usize, hunk_total: usize,
    //          file_format: &FileFormat, display_options: &DisplayOptions)
    println!(
        "{}",
        style::header(
            &diff_result.display_path,
            diff_result.extra_info.as_ref(),
            1,
            1,
            &diff_result.file_format,
            display_options,
        )
    );

    // Summarization requires a parsed token structure. Plain text and fallback
    // files (graph limit exceeded, too many parse errors, etc.) have no token
    // structure, so report availability and fall back to a simple has-changes
    // indicator rather than silently printing "No changes."
    match &diff_result.file_format {
        FileFormat::TextFallback { reason } => {
            println!("Summarization not available ({}).", reason);
            println!(
                "{}",
                if diff_result.has_syntactic_changes {
                    "Has changes."
                } else {
                    "No changes."
                }
            );
            println!();
            return;
        }
        FileFormat::PlainText => {
            println!("Summarization not available (plain text file).");
            println!(
                "{}",
                if diff_result.has_syntactic_changes {
                    "Has changes."
                } else {
                    "No changes."
                }
            );
            println!();
            return;
        }
        FileFormat::Binary => {
            match diff_result.has_byte_changes {
                Some((lhs_len, rhs_len)) => {
                    println!("Binary file modified ({} -> {} bytes).", lhs_len, rhs_len);
                }
                None => {
                    println!("No changes.");
                }
            }
            println!();
            return;
        }
        FileFormat::SupportedLanguage(_) => {}
    }

    let behavioral_count = diff_result.behavioral_changes.len();
    let cosmetic_count = diff_result.cosmetic_changes.len();

    if behavioral_count == 0 && cosmetic_count == 0 {
        println!("No changes.");
        println!();
        return;
    }

    if cosmetic_count > 0 && behavioral_count == 0 {
        println!("No behavioral changes.");
        println!();
        return;
    }

    if behavioral_count > 0 {
        let behavior_label = if display_options.summarize_colors && display_options.use_color {
            format!(
                "{} behavioral {}:",
                behavioral_count.to_string().red(),
                if behavioral_count == 1 { "change" } else { "changes" }
            )
        } else {
            format!(
                "{} behavioral {}:",
                behavioral_count,
                if behavioral_count == 1 { "change" } else { "changes" }
            )
        };
        println!("{}", behavior_label);
        for span in &diff_result.behavioral_changes {
            println!("  {}", format_span(span));
        }
    }

    if cosmetic_count > 0 {
        let cosmetic_label = if display_options.summarize_colors && display_options.use_color {
            format!(
                "{} cosmetic {}:",
                cosmetic_count.to_string().blue(),
                if cosmetic_count == 1 { "change" } else { "changes" }
            )
        } else {
            format!(
                "{} cosmetic {}:",
                cosmetic_count,
                if cosmetic_count == 1 { "change" } else { "changes" }
            )
        };
        println!("{}", cosmetic_label);
        for span in &diff_result.cosmetic_changes {
            println!("  {}", format_span(span));
        }
    }

    println!();
}