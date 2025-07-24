.DEFAULT_GOAL := generate

OPENAPI_JSON=openapi.json
OPENAPI_URL=http://localhost:4202/api-doc/openapi.json
RUST_CLIENT_OUT=./sysinspect-client
PROJECT_NAME=syswebclient
GENERATOR_BIN=.bin
GENERATOR_JAR=$(GENERATOR_BIN)/openapi-generator-cli.jar
GENERATOR_VERSION=7.14.0
DOCS_MD=$(wildcard $(RUST_CLIENT_OUT)/docs/*.md)
DOCS_HTML=$(patsubst %.md,%.html,$(DOCS_MD))


.PHONY: generate clean ensure_codegen docs

generate: $(OPENAPI_JSON) ensure_codegen
	jq '.info.license.identifier = "Apache-2.0"' $(OPENAPI_JSON) > $(OPENAPI_JSON).patched
	mv $(OPENAPI_JSON).patched $(OPENAPI_JSON)
	java -jar $(GENERATOR_JAR) generate \
	  -i $(OPENAPI_JSON) \
	  -g rust \
	  -o $(RUST_CLIENT_OUT) \
	  --skip-validate-spec \
	  --additional-properties packageName=$(PROJECT_NAME)
	cd $(RUST_CLIENT_OUT) && cargo clippy --fix --allow-dirty -- -Dwarnings
	$(MAKE) docs

$(OPENAPI_JSON):
	@if [ -f $(OPENAPI_JSON) ]; then rm -f $(OPENAPI_JSON); fi
	curl -fsSL -o $(OPENAPI_JSON) $(OPENAPI_URL)

ensure_codegen:
	@if [ ! -d $(GENERATOR_BIN) ]; then \
		mkdir -p $(GENERATOR_BIN); \
	fi
	@if [ ! -f $(GENERATOR_JAR) ]; then \
		echo "Downloading openapi-generator-cli.jar..."; \
		curl -L "https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/$(GENERATOR_VERSION)/openapi-generator-cli-$(GENERATOR_VERSION).jar" -o $(GENERATOR_JAR); \
	fi

docs:
	python3 -m venv .venv && \
	.venv/bin/pip install --upgrade pip && \
	.venv/bin/pip install mkdocs mkdocs-material && \
	.venv/bin/mkdocs build --site-dir ./html-docs

clean:
	rm -rf html-docs .venv .bin *json
