//! A project's conversation with its assistant, as use cases: the person says something, the
//! assistant answers with `say`, Toknila reports what the assistant did, and the window and the
//! CLI read the record in pages. A card's discussion stays in the card's thread; the reader
//! port returns it as part of the conversation, so no message is stored twice.

use serde::{Deserialize, Serialize};
use toknila_core::conversation::{self, About, Author, Message};
use toknila_core::events::DomainEvent;
use toknila_core::{Project, ProjectId, TicketKey};

use crate::{AppError, AppResult, ErrorKind, Reader, UnitOfWork, planner, resolve};

impl From<conversation::Refusal> for AppError {
    fn from(r: conversation::Refusal) -> Self {
        let kind = match r {
            conversation::Refusal::NotYours { .. } => ErrorKind::Forbidden,
            _ => ErrorKind::BadRequest,
        };
        AppError::new(kind, r.to_string())
    }
}

/// What the person sends from the assistant's screen.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewPersonMessage {
    pub text: String,
    /// Names of the files that went with it.
    #[serde(default)]
    pub attachments: Vec<String>,
    /// The chain the message is about (the window's context chip); a card's words go to the
    /// card's discussion instead.
    #[serde(default)]
    pub about: Option<About>,
}

/// What the assistant says with its `say` tool.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Say {
    pub text: String,
    /// The card (key, id or title) or chain (name) the words are about, or `roadmap`.
    #[serde(default)]
    pub about: Option<String>,
}

/// A page of the conversation, oldest first.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationPage {
    pub messages: Vec<Message>,
    /// There are older messages than the first one here: ask again with `before` set to its id.
    pub more: bool,
}

fn store<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    author: Author,
    text: String,
    about: Option<About>,
    attachments: Vec<String>,
) -> AppResult<Message> {
    let m = Message {
        id: conversation::next_id(),
        author,
        text,
        about,
        attachments,
        thread: None,
        at: uow.now(),
    };
    uow.insert_conversation_message(project, &m)?;
    uow.emit(&DomainEvent::ConversationMessage {
        project,
        message: m.clone(),
    })?;
    Ok(m)
}

/// A person says something to the project's assistant: the message is stored before anything
/// reaches the planner's terminal, which is the host's to type.
pub fn say_as_person<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &Project,
    input: NewPersonMessage,
) -> AppResult<Message> {
    conversation::check_writer(uow.actor(), Author::Person)?;
    let text = conversation::clean_text(&input.text)?;
    let files = input
        .attachments
        .into_iter()
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect();
    let about = match input.about {
        None => None,
        Some(About::Chain { name }) => {
            let c = planner::chain(&*uow, project.id, &name)
                .map_err(|_| AppError::not_found(format!("chain {name}")))?;
            Some(About::Chain { name: c.name })
        }
        Some(About::Roadmap) => Some(About::Roadmap),
        Some(About::Card { .. }) => {
            return Err(AppError::new(
                ErrorKind::BadRequest,
                "a message about a card goes to the card's discussion",
            ));
        }
    };
    store(uow, project.id, Author::Person, text, about, files)
}

/// What `about` names in the planner's project: a card, else a chain; a card or chain of another
/// project is refused.
fn about_of<R: Reader + ?Sized>(r: &R, project: ProjectId, reference: &str) -> AppResult<About> {
    match planner::card(r, project, reference) {
        Ok(t) => Ok(About::Card { key: t.key }),
        Err(e) if e.kind == ErrorKind::NotFound => match planner::chain(r, project, reference) {
            Ok(c) => Ok(About::Chain { name: c.name }),
            // No card or chain of that name: "roadmap" names the roadmap.
            Err(_) if reference.eq_ignore_ascii_case("roadmap") => Ok(About::Roadmap),
            Err(_) => Err(AppError::not_found(format!("card or chain {reference}"))),
        },
        Err(e) => Err(e),
    }
}

/// The assistant answers, asks or tells the person something: its `say` tool. Only the
/// project's planner may.
pub fn say_as_assistant<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    input: Say,
) -> AppResult<Message> {
    conversation::check_writer(uow.actor(), Author::Assistant)?;
    let text = conversation::clean_text(&input.text)?;
    let about = match input
        .about
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        Some(r) => Some(about_of(&*uow, project, r)?),
        None => None,
    };
    store(uow, project, Author::Assistant, text, about, Vec::new())
}

/// Toknila reports what the assistant did, as a line between the messages. Never refused: it
/// rides along with the command it reports.
pub fn line<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    text: String,
    about: Option<About>,
) -> AppResult<()> {
    store(uow, project, Author::Toknila, text, about, Vec::new()).map(|_| ())
}

/// A line about a card.
pub fn line_about_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    text: String,
    key: TicketKey,
) -> AppResult<()> {
    line(uow, project, text, Some(About::Card { key }))
}

/// A page of the conversation of the project `reference` (id or key): the newest `limit`
/// messages older than the message `before`, oldest first.
pub fn page<R: Reader + ?Sized>(
    r: &R,
    reference: &str,
    before: Option<&str>,
    limit: Option<usize>,
) -> AppResult<ConversationPage> {
    let p = resolve::project(r, reference)?;
    let size = conversation::page_size(limit);
    let mut messages = r.conversation_page(p.id, before, size + 1)?;
    let more = messages.len() > size;
    if more {
        messages.remove(0);
    }
    Ok(ConversationPage { messages, more })
}
