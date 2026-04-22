//! "Recent books across all my communities" — iOS-only capture-flow feature,
//! not in the webapp. Indexed from nostrdb for instant display in the book
//! picker.

use crate::errors::CoreError;
use crate::models::ArtifactRecord;

/// 1. Joined group IDs are derived from cached kind:39001/39002 events.
/// 2. Query nostrdb for kind:11 artifact shares within those groups.
/// 3. Keep artifacts where `source == "book"` OR the reference tag is `i=isbn:…`.
/// 4. Dedupe by `(reference_tag_name, reference_tag_value)`, keeping the
///    most-recent occurrence.
/// 5. Sort by `created_at` desc, cap at `limit`.
pub async fn get_recent_books(_limit: u32) -> Result<Vec<ArtifactRecord>, CoreError> {
    todo!()
}
