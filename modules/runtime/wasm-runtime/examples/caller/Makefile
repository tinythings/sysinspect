MODULE := caller
SRC    := main.go
OUTDIR := build/lib/runtime/wasm

# You MUST set this: WASM_COMPILER=go or WASM_COMPILER=tinygo
WASM_COMPILER ?=

GO     ?= go
TINYGO ?= tinygo
WASM_OPT ?= wasm-opt   # optional (Binaryen). If missing, we skip it.

DEBUG_WASM   := $(OUTDIR)/$(MODULE)_debug.wasm
RELEASE_WASM := $(OUTDIR)/$(MODULE).wasm

.PHONY: help debug release clean check

help:
	@echo "Usage:"
	@echo "  WASM_COMPILER=go     make debug|release"
	@echo "  WASM_COMPILER=tinygo make debug|release"
	@echo
	@echo "Targets:"
	@echo "  debug    - build debuggable WASI wasm"
	@echo "  release  - build smallest WASI wasm we can reasonably squeeze"
	@echo "  check    - show tool availability"
	@echo "  clean    - remove build artifacts"

check:
	@echo "WASM_COMPILER=$(WASM_COMPILER)"
	@echo "go:     $$(command -v $(GO) >/dev/null 2>&1 && $(GO) version || echo MISSING)"
	@echo "tinygo: $$(command -v $(TINYGO) >/dev/null 2>&1 && $(TINYGO) version || echo MISSING)"
	@echo "wasm-opt (optional): $$(command -v $(WASM_OPT) >/dev/null 2>&1 && $(WASM_OPT) --version || echo MISSING)"

# --- guardrails (cry if unset / invalid) ---
define REQUIRE_COMPILER
	@if [ -z "$(WASM_COMPILER)" ]; then \
		echo "ERROR: WASM_COMPILER is not set. Use WASM_COMPILER=go or WASM_COMPILER=tinygo"; \
		exit 2; \
	fi
	@if [ "$(WASM_COMPILER)" != "go" ] && [ "$(WASM_COMPILER)" != "tinygo" ]; then \
		echo "ERROR: WASM_COMPILER must be 'go' or 'tinygo' (got: $(WASM_COMPILER))"; \
		exit 2; \
	fi
endef

debug: clean
	@mkdir -p $(OUTDIR)
	@$(REQUIRE_COMPILER)
	@if [ "$(WASM_COMPILER)" = "tinygo" ]; then \
		command -v $(TINYGO) >/dev/null 2>&1 || (echo "ERROR: tinygo missing"; exit 2); \
		$(TINYGO) build -target=wasi -o $(DEBUG_WASM) -debug -opt=1 $(SRC); \
	else \
		command -v $(GO) >/dev/null 2>&1 || (echo "ERROR: go missing"; exit 2); \
		GOOS=wasip1 GOARCH=wasm $(GO) build -o $(DEBUG_WASM) $(SRC); \
	fi
	@echo "Built debug: $(DEBUG_WASM)"

release: clean
	@mkdir -p $(OUTDIR)
	@$(REQUIRE_COMPILER)
	@if [ "$(WASM_COMPILER)" = "tinygo" ]; then \
		command -v $(TINYGO) >/dev/null 2>&1 || (echo "ERROR: tinygo missing"; exit 2); \
		$(TINYGO) build -target=wasi -o $(RELEASE_WASM) -no-debug -opt=z -panic=trap $(SRC); \
	else \
		command -v $(GO) >/dev/null 2>&1 || (echo "ERROR: go missing"; exit 2); \
		GOOS=wasip1 GOARCH=wasm $(GO) build -o $(RELEASE_WASM) -trimpath -ldflags="-s -w" $(SRC); \
	fi
	@# Optional: squeeze harder with wasm-opt if available
	@if command -v $(WASM_OPT) >/dev/null 2>&1; then \
		echo "Running wasm-opt for extra shrink..."; \
		$(WASM_OPT) --all-features -Oz --strip-debug --strip-producers $(RELEASE_WASM) -o $(RELEASE_WASM); \
	else \
		echo "wasm-opt not found; skipping extra squeezing."; \
	fi
	@echo "Built release: $(RELEASE_WASM)"

run:
	@echo "Running module (test preview)..."
	@echo '{"config": {"path.sharelib": "${PWD}/build"}, "args": {"rt.mod": "caller"}}' | ../../../../../target/debug/runtime/wasm-runtime | jq


clean:
	rm -rf $(OUTDIR)
	rm -rf build
