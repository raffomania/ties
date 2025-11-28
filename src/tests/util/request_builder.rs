use axum::{
    Router,
    body::Body,
    http::{self, HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, request},
};
use http_body_util::BodyExt;
use mime_guess::mime;
use serde::Serialize;
use tower::{Service, ServiceExt};
use visdom::Vis;

use super::dom::assert_form_matches;
use crate::tests::util::html_decode::html_decode;

pub struct RequestBuilder {
    router: axum::Router,
    /// This is the HTTP status that we expect the backend to return.
    /// If it returns a different status, we'll panic.
    expected_status: StatusCode,
    request: request::Builder,
    logged_in_cookie: Option<String>,
}

impl RequestBuilder {
    pub fn new(router: &Router, logged_in_cookie: Option<String>) -> Self {
        RequestBuilder {
            router: router.clone(),
            expected_status: StatusCode::OK,
            request: Request::builder(),
            logged_in_cookie,
        }
    }

    pub fn expect_status(mut self, expected: StatusCode) -> Self {
        self.expected_status = expected;
        self
    }

    pub fn header<V>(mut self, key: HeaderName, val: V) -> Self
    where
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.request = self.request.header(key, val);
        self
    }

    pub async fn post<Input>(mut self, url: &str, input: &Input) -> TestResponse
    where
        Input: Serialize,
    {
        if let Some(cookie) = &self.logged_in_cookie {
            self.request = self.request.header(axum::http::header::COOKIE, cookie);
        }

        let request = self
            .request
            .method(http::Method::POST)
            .uri(url)
            .header(
                http::header::CONTENT_TYPE,
                mime::APPLICATION_WWW_FORM_URLENCODED.as_ref(),
            )
            .body(serde_qs::to_string(input).unwrap())
            .unwrap();

        let response = ServiceExt::<Request<Body>>::ready(&mut self.router)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();

        tracing::debug!("{:?}", response.headers());

        Self::assert_expected_status(self.expected_status, &response, "GET", url);

        TestResponse {
            response,
            new_request_builder: RequestBuilder::new(&self.router, self.logged_in_cookie),
        }
    }

    pub async fn get(mut self, url: &str) -> TestResponse {
        if let Some(cookie) = &self.logged_in_cookie {
            self.request = self.request.header(axum::http::header::COOKIE, cookie);
        }

        let request = self.request.uri(url).body(Body::empty()).unwrap();

        let response = ServiceExt::<Request<Body>>::ready(&mut self.router)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();

        tracing::debug!("{:?}", response.headers());

        Self::assert_expected_status(self.expected_status, &response, "GET", url);

        TestResponse {
            response,
            new_request_builder: RequestBuilder::new(&self.router, self.logged_in_cookie),
        }
    }

    fn assert_expected_status(
        expected_status: StatusCode,
        response: &Response<Body>,
        method: &str,
        url: &str,
    ) {
        assert_eq!(
            response.status(),
            expected_status,
            "expected {expected_status}: {method} {url}"
        );
    }
}

pub struct TestResponse {
    response: Response<Body>,
    new_request_builder: RequestBuilder,
}

impl TestResponse {
    #[expect(dead_code)]
    pub async fn dom(self) -> visdom::types::Elements<'static> {
        let body = self
            .response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        Vis::load(String::from_utf8(body).unwrap()).unwrap()
    }

    pub fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }

    pub async fn test_page(self) -> TestPage {
        let body = self
            .response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        let dom = Vis::load(String::from_utf8(body).unwrap()).unwrap();

        TestPage {
            dom,
            // TODO this doesn't persist the previous login cookie
            request_builder: self.new_request_builder,
        }
    }
}

pub struct TestPage {
    pub dom: visdom::types::Elements<'static>,
    pub request_builder: RequestBuilder,
}

impl TestPage {
    pub async fn fill_form<I: Serialize>(self, form_selector: &str, input: &I) -> TestResponse {
        let form = self.dom.find(form_selector);
        let method = form
            .attr("method")
            .map_or("post".to_string(), |val| val.to_string())
            .to_lowercase();

        let action = form
            .attr("action")
            .expect("Missing action attribute for form {form:?}")
            .to_string();
        assert_form_matches(&form, &input);

        match method.as_str() {
            "post" => self.request_builder.post(&action, input).await,
            "get" => {
                let queries = serde_qs::to_string(input).expect("Failed to serialize input");
                let url = format!("{action}?{queries}");
                self.request_builder.get(&url).await
            }
            _ => panic!(
                "Unsupported method {method} for form with action {:?}",
                form.attr("action")
            ),
        }
    }

    pub fn expect_status(mut self, expected: StatusCode) -> Self {
        self.request_builder = self.request_builder.expect_status(expected);
        self
    }

    pub async fn visit_link(self, text_contains: &str) -> TestPage {
        let url = self
            .dom
            .find("a")
            .filter_by(|_, a| a.html().contains(text_contains))
            .attr("href")
            .unwrap()
            .to_string();
        let url = html_decode(&url);

        self.request_builder.get(&url).await.test_page().await
    }
}
