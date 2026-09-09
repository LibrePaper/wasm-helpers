.DEFAULT_GOAL := help
.PHONY: help test fmt clean

help:  ## Display this help screen
	@printf "\033[1mAvailable commands:\033[0m\n\n"
	@grep -hE '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}' | sort

test:  ## Run the shared tests
	@cargo test

fmt:
	@cargo fmt

clean:
	@rm -rf target
