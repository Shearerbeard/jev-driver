.PHONY: check fmt fmt-check clippy test red live

# Default gate: lint-wave checks over the library crates (testapp is the
# red-first acceptance harness and is gated separately via `make red`).
check: fmt-check clippy test

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

# Red-first acceptance harness: expected to FAIL until all implementation
# stages land; required green from Stage 4 onward.
red:
	cargo test -p testapp

# Live smoke against api.typesafe.ai (reads .env / TYPESAFE_API_KEY).
live:
	cargo run -p testapp --bin triage -- --live
