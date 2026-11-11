//! `tok inbox`: what waits on a person.

use anyhow::Result;
use toknila_client::Client;
use toknila_core::inbox::InboxItem;

use super::{default_project, emit};
use crate::output::w;

pub(super) fn run(c: &Client, project: Option<String>, json_out: bool) -> Result<()> {
    let project = default_project(c, project)?;
    let items = c.inbox(&project)?;
    emit(json_out, &items, |items| {
        if items.is_empty() {
            println!("nothing waits on you");
        }
        for i in items {
            print!("{}", lines(i));
        }
    });
    Ok(())
}

/// One item as text: its kind, card and title, then what it carries (a review's gist and checks).
fn lines(i: &InboxItem) -> String {
    let card = i
        .ticket
        .as_ref()
        .map_or_else(|| "-".to_owned(), ToString::to_string);
    let mut out = format!("{}  {}  {}\n", w(&i.kind), card, i.title);
    if let Some(h) = &i.hand_in {
        out.push_str(&format!("  gist: {}\n", h.gist));
        let checks = h.checks_line();
        if !checks.is_empty() {
            out.push_str(&format!("  checks: {checks}\n"));
        }
    } else if !i.text.is_empty() {
        out.push_str(&format!("  {}\n", i.text));
    }
    out
}
