# Open Knowledge Terminal
#
#   make            build the client (WebAssembly) and the server that embeds it
#   make install    put the `okt` binary in $(PREFIX)/bin
#   make test       unit tests
#   make parity     compare the JSON API with a running reference server (see tests/parity)
#
# Needs: rustup's wasm32-unknown-unknown target and wasm-bindgen-cli at the version in
# Cargo.lock (`cargo install wasm-bindgen-cli --version <x.y.z>`). The WCARGO / WFLAGS /
# WASM_TARGET_DIR variables let an unusual toolchain stand in (e.g. a distro rustc with
# `-Z build-std`).

PREFIX          ?= $(HOME)/.local
CARGO           ?= cargo
WCARGO          ?= $(CARGO)
WFLAGS          ?= --target wasm32-unknown-unknown
WASM_TARGET_DIR ?= $(CURDIR)/target
WASM_BINDGEN    ?= wasm-bindgen
BINDGEN_VERSION := $(shell awk '/^name = "wasm-bindgen"$$/{getline; gsub(/version = |"/,""); print}' Cargo.lock)
DIST            := okt-client/dist
WASM            := $(WASM_TARGET_DIR)/wasm32-unknown-unknown/wasm-release/okt_client.wasm

.PHONY: all client server install test parity clean

all: server

client:
	@$(WASM_BINDGEN) --version 2>/dev/null | grep -qx 'wasm-bindgen $(BINDGEN_VERSION)' || { \
	  echo "okt: needs wasm-bindgen-cli $(BINDGEN_VERSION): cargo install wasm-bindgen-cli --version $(BINDGEN_VERSION)" >&2; exit 1; }
	CARGO_TARGET_DIR=$(WASM_TARGET_DIR) $(WCARGO) build -p okt-client --profile wasm-release $(WFLAGS)
	rm -rf $(DIST) && mkdir -p $(DIST)
	$(WASM_BINDGEN) --target web --no-typescript --out-dir $(DIST) --out-name okt $(WASM)
	cp okt-client/static/index.html okt-client/static/app.css okt-client/static/main.js $(DIST)/

server: client
	$(CARGO) build --release -p okt-server

install: server
	install -d $(PREFIX)/bin
	install -m 755 target/release/okt $(PREFIX)/bin/okt

test:
	$(CARGO) test -p okt-server -p okt-pygments -p okt-client

parity:
	python3 tests/parity/api_diff.py

clean:
	$(CARGO) clean
	rm -rf $(DIST)
