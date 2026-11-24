.PHONY: check test clippy clean

check: test

test:
	cargo test --offline

clippy:
	cargo clippy --offline --all-targets

clean:
	cargo clean
