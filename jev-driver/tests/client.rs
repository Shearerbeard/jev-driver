//! Client tests against the scripted FakeTransport: body assembly, retry
//! on 429/529, retry exhaustion, and cancellation. Never touches the
//! network.

use std::sync::Arc;

use jev_driver::fake::FakeTransport;
use jev_driver::prelude::*;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn noul_question() -> BuiltQuestion {
    DynChoice::new("team", Instructions::text("which team"))
        .option("red", Instructions::text("the red team"))
        .option("blue", Instructions::text("the blue team"))
        .build()
        .unwrap_or_else(|e| panic!("dyn choice must build: {e}"))
}

fn ok_response() -> JevResult<WireResponse> {
    let value = json!({
        "model": "jev-1.13.0",
        "answers": {
            "team": {
                "type": "choice",
                "choice": "red",
                "probabilities": { "red": 0.6, "blue": 0.4 },
                "confidence": 0.6
            }
        },
        "usage": { "input_tokens": 100, "output_tokens": 5 }
    });
    Ok(serde_json::from_value(value).unwrap_or_else(|e| panic!("fixture must parse: {e}")))
}

fn decision() -> Decision {
    Decision::builder()
        .state(json!({ "status": "deploying" }))
        .and_then(|builder| builder.with_question(noul_question()))
        .and_then(DecisionBuilder::build)
        .unwrap_or_else(|e| panic!("decision must build: {e}"))
}

fn fast_retry() -> RetryConfig {
    RetryConfig {
        max_retries: 3,
        initial_interval: std::time::Duration::from_millis(1),
        max_interval: std::time::Duration::from_millis(5),
        multiplier: 2.0,
    }
}

#[tokio::test]
async fn success_sends_pinned_model_and_parses_answers() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(FakeTransport::new(vec![ok_response()]));
    let client = JevClient::with_transport(transport.clone(), "jev-1.13.0", fast_retry());

    let answers = client.evaluate_raw(&decision()).await?;
    assert_eq!(answers.model, "jev-1.13.0", "model echoes back");
    assert_eq!(answers.usage.input_tokens, 100, "usage round-trips");

    let team = answers.choice("team")?;
    assert_eq!(team.selected, "red", "selected option parses");

    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "exactly one request on success");
    assert!(
        requests[0].contains("\"model\":\"jev-1.13.0\""),
        "request body carries the pinned model"
    );
    assert!(
        requests[0].contains("\"team\""),
        "request body carries the question id"
    );
    Ok(())
}

#[tokio::test]
async fn rate_limits_retry_until_success() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(FakeTransport::new(vec![
        Err(JevError::RateLimited { retries: 0 }),
        Err(JevError::RateLimited { retries: 0 }),
        ok_response(),
    ]));
    let client = JevClient::with_transport(transport.clone(), "jev-1.13.0", fast_retry());

    let answers = client.evaluate_raw(&decision()).await?;
    assert_eq!(
        answers.choice("team")?.selected,
        "red",
        "third attempt wins"
    );
    assert_eq!(transport.requests().len(), 3, "three attempts total");
    Ok(())
}

#[tokio::test]
async fn retries_exhausted_surfaces_rate_limit() -> Result<(), Box<dyn std::error::Error>> {
    let scripted = (0..5)
        .map(|_| Err(JevError::RateLimited { retries: 0 }))
        .collect::<Vec<_>>();
    let transport = Arc::new(FakeTransport::new(scripted));
    let retry = RetryConfig {
        max_retries: 2,
        ..fast_retry()
    };
    let client = JevClient::with_transport(transport.clone(), "jev-1.13.0", retry);

    let err = client
        .evaluate_raw(&decision())
        .await
        .expect_err("exhausted retries must surface");
    match err {
        JevError::RateLimited { retries } => {
            assert_eq!(retries, 2, "error reports the configured retry budget");
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
    assert_eq!(
        transport.requests().len(),
        3,
        "initial attempt plus two retries"
    );
    Ok(())
}

#[tokio::test]
async fn cancellation_beats_backoff() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(FakeTransport::new(vec![
        Err(JevError::Overloaded { retries: 0 }),
        ok_response(),
    ]));
    let client = JevClient::with_transport(transport, "jev-1.13.0", fast_retry());

    let token = CancellationToken::new();
    token.cancel();

    let err = client
        .evaluate_raw_with_cancellation(&decision(), &token)
        .await
        .expect_err("a cancelled token must short-circuit the loop");
    assert!(
        matches!(err, JevError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn unauthorized_is_not_retried() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(FakeTransport::new(vec![Err(JevError::Unauthorized)]));
    let client = JevClient::with_transport(transport.clone(), "jev-1.13.0", fast_retry());

    let err = client
        .evaluate_raw(&decision())
        .await
        .expect_err("auth errors are terminal");
    assert!(
        matches!(err, JevError::Unauthorized),
        "expected Unauthorized, got {err:?}"
    );
    assert_eq!(
        transport.requests().len(),
        1,
        "no retries burned on auth errors"
    );
    Ok(())
}
