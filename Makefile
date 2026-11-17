.PHONY: help build test check clippy fmt fmt-check doc clean docker-build helm-lint deny kind-smoke all

CARGO ?= cargo
DOCKER ?= docker
IMAGE ?= spotprice-exporter:dev

help:
	@echo "spotprice-exporter development targets"
	@echo ""
	@echo "  build          - Build release binary"
	@echo "  test           - Run all tests"
	@echo "  check          - Run cargo check"
	@echo "  clippy         - Run clippy with warnings as errors"
	@echo "  fmt            - Format code"
	@echo "  fmt-check      - Check formatting"
	@echo "  doc            - Build documentation"
	@echo "  clean          - Clean build artifacts"
	@echo "  docker-build   - Build the container image locally ($(IMAGE))"
	@echo "  deny           - Supply chain checks (cargo-deny)"
	@echo "  helm-lint      - Lint the Helm chart"
	@echo "  kind-smoke     - Deploy to a throwaway kind cluster and check metrics arrive"
	@echo "  all            - Run check, test, clippy, fmt-check"

build:
	$(CARGO) build --release

test:
	$(CARGO) test --all

check:
	$(CARGO) check --all

clippy:
	$(CARGO) clippy --all -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

doc:
	$(CARGO) doc --no-deps

clean:
	$(CARGO) clean

docker-build:
	$(DOCKER) buildx build --load -t $(IMAGE) .

deny:
	cargo deny check

helm-lint:
	helm lint charts/spotprice-exporter --set zones=X --set entsoe.existingSecret=x --set otlp.endpoint=http://x

kind-smoke:
	bash hack/kind-smoke.sh

all: check test clippy fmt-check
	@echo "All checks passed!"
