﻿use std::future::Future;
use std::sync::Arc;
use std::pin::Pin;
use bytes::Bytes;
use http::Response;

use crate::app::{
    HttpContext,
    results::Results
};

pub mod mapping;

pub type Next = Arc<dyn Fn(Arc<HttpContext>) -> Pin<Box<dyn Future<Output = http::Result<Response<Bytes>>> + Send>> + Send + Sync>;
pub(crate) type Middleware = Arc<dyn Fn(Arc<HttpContext>, Next) -> Pin<Box<dyn Future<Output = http::Result<Response<Bytes>>> + Send>> + Send + Sync>;

pub(crate) struct Middlewares {
    pipeline: Vec<Middleware>
}

impl Middlewares {
    pub(crate) fn new() -> Self {
        Self { pipeline: Vec::new() }
    }

    pub(crate) async fn execute(&self, ctx: Arc<HttpContext>) -> http::Result<Response<Bytes>> {
        let next = self.compose();
        next(Arc::clone(&ctx)).await
    }

    fn compose(&self) -> Next {
        // Check if the pipeline is empty or not as a safeguard.
        if self.pipeline.is_empty() {
            // Return a default handler if there is actually nothing in the pipeline.
            return Arc::new(|_ctx| Box::pin(async { Results::not_found() }));
        }

        // Fetching the last middleware which is the request handler to be the initial `next`.
        let request_handler = self.pipeline.last().unwrap().clone();
        let mut next: Next = Arc::new(move |ctx| {
            let handler = request_handler.clone();
            // Call the last middleware, ignoring its `next` argument with an empty placeholder
            Box::pin(async move {
                (handler)(ctx, Arc::new(|_| Box::pin(async { Results::not_found() }))).await
            })
        });

        for mw in self.pipeline.iter().rev().skip(1) {
            let current_mw: Middleware = mw.clone();
            let prev_next: Next = next.clone();

            next = Arc::new(move |ctx: Arc<HttpContext>| {
                let current_mw = current_mw.clone();
                let prev_next = prev_next.clone();
                Box::pin(async move {
                    (current_mw)(ctx, prev_next).await
                })
            });
        }
        next
    }
}

