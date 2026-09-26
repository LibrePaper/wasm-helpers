.DEFAULT_GOAL := help
.PHONY: help test fuzz fmt clean

help:  ## Display this help screen
	@printf "\033[1mAvailable commands:\033[0m\n\n"
	@grep -hE '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}' | sort

test:  ## Run the shared tests
	@cargo test

FUZZ_SECONDS ?= 60
fuzz:  ## Fuzz diff and merge (requires nightly and cargo-fuzz)
	@cd fuzz && cargo +nightly fuzz run -s none merge -- -max_total_time=$(FUZZ_SECONDS)

fmt:
	@cargo fmt

clean:
	@rm -rf target
