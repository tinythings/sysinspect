# Rust -> WASI Makefile
# Targets:
#   make        -> release
#   make devel  -> debug
#
# Result:
#   target/<profile>/lib/runtime/wasm/<bin>.wasm
#
# Install note:
#   Add target/<profile>/lib to sysinspect module path.

BIN       ?= rs-reader
WASI      ?= wasm32-wasip1
CARGO     ?= cargo
WASM_OPT  ?= wasm-opt

PROFILE   ?= release

OUT_DIR   := target/$(WASI)/$(PROFILE)
WASM_IN   := $(OUT_DIR)/$(BIN).wasm
DEST_DIR  := target/$(PROFILE)/lib/runtime/wasm
WASM_OUT  := $(DEST_DIR)/$(BIN).wasm

HAVE_WASM_OPT := $(shell command -v $(WASM_OPT) >/dev/null 2>&1 && echo yes)

.PHONY: all devel build optimize copy info clean

all:
	@$(MAKE) PROFILE=release build optimize copy info

devel:
	@$(MAKE) PROFILE=debug build optimize copy info

build:
	@echo "==> Building $(BIN) ($(PROFILE))"
	@$(CARGO) build $(if $(filter release,$(PROFILE)),--release,) --target $(WASI)

optimize:
ifeq ($(HAVE_WASM_OPT),yes)
	@echo "==> Optimizing with wasm-opt"
	@$(WASM_OPT) -Oz --all-features $(WASM_IN) -o $(WASM_IN).opt
	@mv $(WASM_IN).opt $(WASM_IN)
else
	@echo "==> wasm-opt not found, skipping optimisation"
endif

copy:
	@echo "==> Installing WASM to $(DEST_DIR)"
	@mkdir -p $(DEST_DIR)
	@cp $(WASM_IN) $(WASM_OUT)

info:
	@echo ""
	@echo "WASM ready:"
	@echo "  $(WASM_OUT)"
	@echo ""
	@echo "To use with SysInspect:"
	@echo "  add './lib' from:"
	@echo "  target/$(PROFILE)/lib"
	@echo ""

clean:
	@$(CARGO) clean
