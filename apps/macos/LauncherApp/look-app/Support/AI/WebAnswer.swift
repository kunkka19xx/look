import Foundation

/// An instant, web-sourced answer (à la Spotlight's knowledge card). Returned
/// finished - no streaming - because it's a cached lookup, not generation.
///
/// The data now comes from the shared `look_answers` Rust crate via
/// `EngineBridge`; this struct is the decoded Swift view of it.
struct WebAnswer: Sendable {
    let text: String
    let source: String
    let url: URL?
    let imageURL: URL?
}
