fmt:
	@echo "Formatting entire project..."
	@cargo +nightly fmt

test-worker:
	@echo "Running worker integration tests with 1 thread"
	@cargo test -p aether-worker -- --nocapture --test-threads=1

test-broker:
	@echo "Running broker tests with 1 thread"
	@cargo test -p aether-broker -- --nocapture --test-threads=1
