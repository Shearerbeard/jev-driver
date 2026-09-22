//! Scripted transport for offline tests (feature `test-support`).

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::client::Transport;
use crate::error::{JevError, JevResult};
use crate::wire::WireResponse;

/// Serves pre-scripted results in order and records every request body.
pub struct FakeTransport {
    scripted: Mutex<VecDeque<JevResult<WireResponse>>>,
    requests: Mutex<Vec<String>>,
}

impl FakeTransport {
    /// Creates the transport with a result queue.
    pub fn new(scripted: Vec<JevResult<WireResponse>>) -> Self {
        Self {
            scripted: Mutex::new(scripted.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every request body sent so far, in order.
    pub fn requests(&self) -> Vec<String> {
        self.requests
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Transport for FakeTransport {
    async fn post_systemone(&self, body: String) -> JevResult<WireResponse> {
        {
            let mut requests = self
                .requests
                .lock()
                .map_err(|_| JevError::Transport("fake transport poisoned".to_owned()))?;
            requests.push(body);
        }
        let next = {
            let mut queue = self
                .scripted
                .lock()
                .map_err(|_| JevError::Transport("fake transport poisoned".to_owned()))?;
            queue.pop_front()
        };
        next.unwrap_or_else(|| {
            Err(JevError::Transport(
                "fake transport exhausted: not enough scripted results".to_owned(),
            ))
        })
    }
}
