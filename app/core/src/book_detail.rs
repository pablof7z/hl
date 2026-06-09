use crate::models::{BookRoute, HighlightRecord};

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookDetailSnapshot {
    pub route: Option<BookRoute>,
    pub highlights: Vec<HighlightRecord>,
    pub error: String,
}

impl BookDetailSnapshot {
    pub fn empty() -> Self {
        Self {
            route: None,
            highlights: Vec::new(),
            error: String::new(),
        }
    }
}

pub fn snapshot(route: BookRoute, highlights: Vec<HighlightRecord>) -> BookDetailSnapshot {
    BookDetailSnapshot {
        route: Some(route),
        highlights,
        error: String::new(),
    }
}

pub fn error_snapshot(route: Option<BookRoute>, error: impl ToString) -> BookDetailSnapshot {
    BookDetailSnapshot {
        route,
        highlights: Vec::new(),
        error: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> BookRoute {
        BookRoute {
            catalog_id: "isbn:9780143127550".to_string(),
            isbn: "9780143127550".to_string(),
        }
    }

    #[test]
    fn empty_snapshot_has_no_route_or_error() {
        let snapshot = BookDetailSnapshot::empty();

        assert!(snapshot.route.is_none());
        assert!(snapshot.highlights.is_empty());
        assert!(snapshot.error.is_empty());
    }

    #[test]
    fn error_snapshot_preserves_route_and_clears_rows() {
        let snapshot = error_snapshot(Some(route()), "cache miss");

        assert_eq!(snapshot.route.unwrap().catalog_id, "isbn:9780143127550");
        assert!(snapshot.highlights.is_empty());
        assert_eq!(snapshot.error, "cache miss");
    }
}
