use std::collections::HashMap;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::Serialize;

use crate::shared::errdef::Error;

pub const DEFAULT_PER_PAGE: i64 = 10;
pub const MAX_PER_PAGE: i64 = 100;

/// Query string binding for every `Index` endpoint. Unknown query parameters
/// become filters, which is what the Go `CustomBinder` did.
#[derive(Debug, Clone)]
pub struct ListRequest {
    pub page: i64,
    pub per_page: i64,
    pub sort_by: String,
    pub order: String,
    pub filters: HashMap<String, String>,
}

impl Default for ListRequest {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: DEFAULT_PER_PAGE,
            sort_by: String::new(),
            order: String::new(),
            filters: HashMap::new(),
        }
    }
}

impl ListRequest {
    pub fn from_query(query: &str) -> Self {
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(query).unwrap_or_default();

        let mut request = ListRequest::default();

        for (key, value) in pairs {
            match key.as_str() {
                "page" => request.page = value.parse().unwrap_or(1),
                "per_page" => request.per_page = value.parse().unwrap_or(DEFAULT_PER_PAGE),
                "sort_by" => request.sort_by = value,
                "order" => request.order = value,
                _ => {
                    request.filters.insert(key, value);
                }
            }
        }

        request.normalize();
        request
    }

    fn normalize(&mut self) {
        if self.page < 1 {
            self.page = 1;
        }
        if self.per_page < 1 || self.per_page > MAX_PER_PAGE {
            self.per_page = DEFAULT_PER_PAGE;
        }
        if self.order != "asc" && self.order != "desc" {
            self.order.clear();
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }

    pub fn filter(&self, key: &str) -> Option<&str> {
        self.filters
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ListRequest {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ListRequest::from_query(parts.uri.query().unwrap_or("")))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetaData {
    pub total_count: i64,
    pub total_pages: i64,
    pub current_page: i64,
    pub per_page: i64,
    pub next_page: Option<i64>,
    pub previous_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Data<T> {
    pub data: Vec<T>,
    pub meta: MetaData,
}

impl<T> Data<T> {
    pub fn new(data: Vec<T>, total_count: i64, current_page: i64, per_page: i64) -> Self {
        let per_page = per_page.max(1);

        let mut total_pages = total_count / per_page;
        if total_count % per_page != 0 {
            total_pages += 1;
        }

        let next_page = Some(current_page + 1).filter(|page| *page <= total_pages);
        let previous_page =
            Some(current_page - 1).filter(|page| *page >= 1 && *page <= total_pages);

        Self {
            data,
            meta: MetaData {
                total_count,
                total_pages,
                current_page,
                per_page,
                next_page,
                previous_page,
            },
        }
    }

    /// Maps the payload while keeping the pagination metadata untouched.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Data<U> {
        Data {
            data: self.data.into_iter().map(f).collect(),
            meta: self.meta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_out_of_range_pagination() {
        let request = ListRequest::from_query("page=0&per_page=500&order=sideways");

        assert_eq!(request.page, 1);
        assert_eq!(request.per_page, DEFAULT_PER_PAGE);
        assert!(request.order.is_empty());
    }

    #[test]
    fn collects_unknown_query_params_as_filters() {
        let request = ListRequest::from_query("page=2&status=Active&role=Admin");

        assert_eq!(request.page, 2);
        assert_eq!(request.filter("status"), Some("Active"));
        assert_eq!(request.filter("role"), Some("Admin"));
    }

    #[test]
    fn builds_page_links() {
        let page = Data::new(vec![1, 2, 3], 25, 2, 10);

        assert_eq!(page.meta.total_pages, 3);
        assert_eq!(page.meta.next_page, Some(3));
        assert_eq!(page.meta.previous_page, Some(1));
    }
}
