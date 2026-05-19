# whistleblower-registry — SPEL program for the on-chain CID registry.
#
# Quick start:
#   make build idl deploy
#   make cli ARGS="anchor_single --cid bafy0001 --metadata_hash 0x... --block_time 1700000000"
#   make benchmark   # writes docs/benchmarks/cu.md

SHELL := /bin/bash
STATE_FILE := .registry-state
IDL_FILE := idl/whistleblower-registry-idl.json
PROGRAMS_DIR := methods/guest/target/riscv32im-risc0-zkvm-elf/docker
PROGRAM_BIN := $(PROGRAMS_DIR)/whistleblower_registry.bin

-include $(STATE_FILE)

.PHONY: help build idl cli deploy setup inspect status clean benchmark

help:
	@echo "whistleblower-registry — SPEL Program"
	@echo ""
	@echo "  make build       Build the guest binary (needs risc0 toolchain)"
	@echo "  make idl         Generate IDL from program source"
	@echo "  make cli ARGS=   Run the IDL-driven CLI (reads spel.toml for config)"
	@echo "  make deploy      Deploy program to sequencer"
	@echo "  make setup       Create anchorer account"
	@echo "  make benchmark   Measure CU for single + 50-CID batch anchor"
	@echo "  make inspect     Show ProgramId for built binary"
	@echo "  make status      Show saved state and binary info"
	@echo "  make clean       Remove saved state"

build:
	cargo risczero build --manifest-path methods/guest/Cargo.toml
	@echo "✅ Guest binary: $(PROGRAM_BIN)"

idl:
	@mkdir -p idl
	spel generate-idl methods/guest/src/bin/whistleblower_registry.rs > $(IDL_FILE)
	@echo "✅ IDL → $(IDL_FILE)"

cli:
	cargo run -p whistleblower-registry-cli -- $(ARGS)

deploy:
	@test -f "$(PROGRAM_BIN)" || (echo "ERROR: Binary not found. Run 'make build' first."; exit 1)
	wallet deploy-program $(PROGRAM_BIN)
	@echo "✅ Program deployed"

inspect:
	cargo run -p whistleblower-registry-cli -- inspect $(PROGRAM_BIN)

setup:
	@echo "Creating anchorer account…"
	$(eval ANCHORER := $(shell wallet account new public 2>&1 | sed -n 's/.*Public\/\([A-Za-z0-9]*\).*/\1/p'))
	@echo "Anchorer: $(ANCHORER)"
	@grep -v '^ANCHORER=' $(STATE_FILE) 2>/dev/null > $(STATE_FILE).tmp || true
	@echo 'ANCHORER=$(ANCHORER)' >> $(STATE_FILE).tmp
	@mv $(STATE_FILE).tmp $(STATE_FILE)
	lgs wallet topup "Public/$(ANCHORER)"
	@echo "✅ Anchorer saved + funded"

benchmark:
	@bash ../../scripts/benchmark.sh

status:
	@echo "whistleblower-registry status"
	@echo "──────────────────────────────────────"
	@if [ -f "$(STATE_FILE)" ]; then cat $(STATE_FILE); else echo "(no state — run 'make setup')"; fi
	@echo ""
	@ls -la $(PROGRAM_BIN) 2>/dev/null || echo "  $(PROGRAM_BIN): NOT BUILT (run 'make build')"
	@ls -la $(IDL_FILE) 2>/dev/null || echo "  $(IDL_FILE): NOT GENERATED (run 'make idl')"

clean:
	rm -f $(STATE_FILE) $(STATE_FILE).tmp
