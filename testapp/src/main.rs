//! jev-driver testapp: ticket triage demo.
//!
//! Offline (default): replays a recorded API response through the full
//! typed pipeline. Live (`--live`): calls the configured endpoint via
//! `JevConfig::from_env` (`TYPESAFE_*` in the environment or a
//! workspace `.env`), overridable with `--endpoint <full URL>` and
//! `--model <id>` for local System One-compatible gateways.

use std::time::Instant;

use jev_driver::prelude::*;
use testapp::questions;
use testapp::questions::{Department, TicketTriageAnswers};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut live = false;
    let mut endpoint: Option<String> = None;
    let mut model: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| -> Result<String, Box<dyn std::error::Error>> {
            let value = args.next().ok_or(format!("{flag} needs a value"))?;
            if value.starts_with("--") {
                return Err(format!("{flag} needs a value before `{value}`").into());
            }
            Ok(value)
        };
        match arg.as_str() {
            "--live" => live = true,
            "--endpoint" => endpoint = Some(value("--endpoint")?),
            "--model" => model = Some(value("--model")?),
            other => return Err(format!("unknown flag: {other}").into()),
        }
    }
    let decision = questions::build_demo_decision()?;

    let started = Instant::now();
    let raw = if live {
        let mut config = JevConfig::from_env()?;
        if let Some(url) = &endpoint {
            config.endpoint = url
                .parse()
                .map_err(|e| format!("invalid --endpoint `{url}`: {e}"))?;
            config.api_key = None;
        }
        if let Some(model) = model {
            config.model = model;
        }
        let client = JevClient::new(config)?;
        client.evaluate_raw(&decision).await?
    } else {
        let response: WireResponse =
            serde_json::from_str(include_str!("../fixtures/triage.response.json"))?;
        Answers::from_wire(response, &decision)?
    };
    let elapsed = started.elapsed();

    let typed: TicketTriageAnswers = raw.parse_as()?;
    let owner = raw.choice("owner")?;

    println!("== jev-driver testapp: ticket triage ==");
    println!("mode: {}", if live { "live" } else { "offline fixture" });
    if let (true, Some(url)) = (live, &endpoint) {
        println!("endpoint: {url}");
    }
    println!("model: {} (in {} ms)", raw.model, elapsed.as_millis());
    println!(
        "usage: {} in / {} out tokens",
        raw.usage.input_tokens, raw.usage.output_tokens
    );
    println!();
    println!(
        "department  -> {:?}  confidence {:.2} act(0.80)={}",
        typed.department.selected,
        typed.department.confidence.get(),
        typed.department.confidence.act(0.80)
    );
    println!(
        "  P(technical)={:.2}  P(other)={:.2}",
        typed
            .department
            .probabilities
            .get(&Department::Technical)
            .get(),
        typed.department.probabilities.get(&Department::Other).get(),
    );
    println!(
        "frustration -> {:?}  score {:.2} confidence {:.2}",
        typed.frustration.nearest()?,
        typed.frustration.score(),
        typed.frustration.confidence.get(),
    );
    println!(
        "is_urgent   -> P(yes)={:.2}  yes(0.50)={}",
        typed.is_urgent.probability.get(),
        typed.is_urgent.probability.yes(0.50),
    );
    println!(
        "owner       -> {}  confidence {:.2}",
        owner.selected(),
        owner.confidence().get(),
    );
    println!();
    println!("routing: {}", questions::route(&typed));
    if typed.is_urgent.probability.yes(0.50) {
        println!("paging: on-call paged (SLA pressure)");
    }

    Ok(())
}
