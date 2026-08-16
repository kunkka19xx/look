//! Joining the meeting a calendar event points at.
//!
//! A Teams, Zoom, or Meet invite already carries everything needed to join, so
//! no API and no network are involved: the link is sitting in the event's own
//! fields. Pure text logic, kept here rather than in a shell, so every platform
//! that grows a calendar source inherits the same answer.
//!
//! Three jobs, one per file: read the request out of the words (`grammar`),
//! find the link inside an invite (`link`), and choose which meeting the
//! request means (`select`).

mod grammar;
mod link;
mod select;

pub use grammar::{JoinRequest, join_query};
pub use link::{JoinLink, Provider, find_join_link};
pub use select::{
    EventInput, IMMINENT_WINDOW_S, JoinOutcome, JoinableMeeting, join_outcome, joinable_meetings,
    next_joinable,
};
