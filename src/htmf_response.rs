use axum::response::{Html, IntoResponse};

pub struct HtmfResponse(pub htmf::element::Element);

impl From<htmf::element::Element> for HtmfResponse {
    fn from(value: htmf::element::Element) -> Self {
        HtmfResponse(value)
    }
}

impl IntoResponse for HtmfResponse {
    fn into_response(self) -> axum::response::Response {
        Html(self.0.to_html()).into_response()
    }
}
