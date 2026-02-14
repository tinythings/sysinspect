# Makefile — run all examples in this subcrate

CARGO ?= cargo
PROFILE ?= debug
RUST_LOG ?= info

# Optional extras:
#   make run-svc ARGS="--foo bar"
#   make run-all FEATURES="something" NO_DEFAULT_FEATURES=1

ARGS ?=
FEATURES ?=
NO_DEFAULT_FEATURES ?=

# Feature flags assembly
FEATURE_FLAGS :=
ifneq ($(strip $(FEATURES)),)
FEATURE_FLAGS += --features "$(FEATURES)"
endif
ifeq ($(NO_DEFAULT_FEATURES),1)
FEATURE_FLAGS += --no-default-features
endif

# Profile flags
ifeq ($(PROFILE),release)
PROFILE_FLAG := --release
else
PROFILE_FLAG :=
endif

EXAMPLES := svc loader

.PHONY: help list examples run-all $(addprefix run-,$(EXAMPLES)) clean

help:
	@echo "Targets:"
	@echo "  make list                 - list known examples"
	@echo "  make run-all              - run all examples"
	@echo "  make run-svc              - run examples/svc.rs"
	@echo "  make run-loader           - run examples/loader.rs"
	@echo ""
	@echo "Vars:"
	@echo "  PROFILE=debug|release     (default: debug)"
	@echo "  RUST_LOG=info|debug|trace (default: info)"
	@echo "  ARGS='...'                (passed after --)"
	@echo "  FEATURES='a b'            (cargo --features)"
	@echo "  NO_DEFAULT_FEATURES=1     (cargo --no-default-features)"
	@echo ""
	@echo "Examples:"
	@echo "  make run-all RUST_LOG=debug"
	@echo "  make run-svc ARGS='--config ./test.json' RUST_LOG=trace"
	@echo "  make run-loader PROFILE=release FEATURES='foo'"

list examples:
	@echo $(EXAMPLES)

run-all:
	@set -e; \
	for ex in $(EXAMPLES); do \
		echo "==> running example: $$ex"; \
		$(MAKE) --no-print-directory run-$$ex; \
	done

run-svc:
	@echo "RUST_LOG=$(RUST_LOG) PROFILE=$(PROFILE) FEATURES='$(FEATURES)'"
	@RUST_LOG=$(RUST_LOG) $(CARGO) run $(PROFILE_FLAG) $(FEATURE_FLAGS) --example svc -- $(ARGS)

run-loader:
	@echo "RUST_LOG=$(RUST_LOG) PROFILE=$(PROFILE) FEATURES='$(FEATURES)'"
	@RUST_LOG=$(RUST_LOG) $(CARGO) run $(PROFILE_FLAG) $(FEATURE_FLAGS) --example loader -- $(ARGS)

clean:
	@$(CARGO) clean
