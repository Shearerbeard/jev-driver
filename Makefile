.PHONY: check fmt fmt-check clippy test red live live-local

# Default gate: lint-wave checks over the library crates (testapp is the
# red-first acceptance harness and is gated separately via `make red`).
check: fmt-check clippy test

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --features test-support -- -D warnings

# Clippy including test code (aspirational, mirrors agent-driver-rs)
clippy-tests:
	cargo clippy --all-targets --features test-support -- -D warnings

test:
	cargo test

# Red-first acceptance harness: expected to FAIL until all implementation
# stages land; required green from Stage 4 onward.
red:
	cargo test -p testapp

# Live smoke against api.typesafe.ai (reads .env / TYPESAFE_API_KEY).
live:
	cargo run -p testapp --bin triage -- --live

# Live smoke against a local System One-compatible gateway, e.g.
# JEV_LOCAL_ENDPOINT=http://localhost:8080/v1/systemone JEV_LOCAL_MODEL=kev-k5
live-local:
	cargo run -p testapp --bin triage -- --live $(if $(JEV_LOCAL_ENDPOINT),--endpoint "$(JEV_LOCAL_ENDPOINT)") $(if $(JEV_LOCAL_MODEL),--model "$(JEV_LOCAL_MODEL)")
