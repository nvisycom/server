# Makefile for the on-premise version of api.nvisy.com

ifneq (,$(wildcard ./.env))
	include .env
	export
endif

# PostgreSQL connection URL for diesel CLI.
POSTGRES_URL ?= postgresql://postgres:postgres@localhost:5432/postgres

# Migration directories and files.
SCHEMA_OUTPUT = ./crates/nvisy-postgres/src/schema.rs
MIGRATIONS_IN_DIR = ./migrations
MIGRATIONS_OUT_DIR = ./crates/nvisy-postgres/src/migrations

# Auth secret keys.
PRIVATE_KEY_FILE = private.pem
PUBLIC_KEY_FILE = public.pem
ENCRYPTION_KEY_FILE = encryption.key

# Shell-level logger (expands to a printf that runs in the shell).
define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

.PHONY: Install-tools
install-tools: # Installs tools required for the repo.
	@$(call log,Checking Diesel CLI...)
	@if ! command -v diesel >/dev/null 2>&1; then \
		$(call log,Installing Diesel CLI with PostgreSQL support...); \
		cargo install diesel_cli --no-default-features --features postgres --locked; \
		$(call log,Diesel CLI installed successfully.); \
	else \
		$(call log,Diesel CLI already available: $$(diesel --version)); \
		$(call log,Verifying PostgreSQL support...); \
		if ! diesel --version | grep -q postgres; then \
			$(call log,Reinstalling Diesel CLI with PostgreSQL support...); \
			cargo install diesel_cli --no-default-features --features postgres --locked --force; \
		fi; \
	fi

.PHONY: install-all
install-all: install-tools ## Installs all dependencies.
	@$(call log,Making scripts executable...)
	@chmod +x scripts/*.sh
	@$(call log,Scripts made executable!)

.PHONY: generate-env
generate-env: ## Copies .env.example to .env.
	@$(call log,Copying .env.example to .env...)
	@cp ./.env.example ./.env
	@$(call log,.env file created successfully.)

.PHONY: generate-keys
generate-keys: ## Generates auth key pair and encryption key.
	@$(call log,Deleting previously generated keys...)
	@rm -f $(PRIVATE_KEY_FILE) $(PUBLIC_KEY_FILE) $(ENCRYPTION_KEY_FILE)
	@$(call log,Previously generated keys deleted.)
	@$(call log,Generating private key...)
	@openssl genpkey -algorithm ed25519 -out $(PRIVATE_KEY_FILE)
	@$(call log,Private key generated successfully.)
	@$(call log,Generating public key...)
	@openssl pkey -in $(PRIVATE_KEY_FILE) -pubout -out $(PUBLIC_KEY_FILE)
	@$(call log,Public key generated successfully.)
	@$(call log,Generating encryption key...)
	@head -c 32 /dev/urandom > $(ENCRYPTION_KEY_FILE)
	@$(call log,Encryption key generated successfully.)

.PHONY: generate-migrations
generate-migrations: ## Regenerates the Postgres migrations and database schema.
	@$(call log,Deleting embedded migrations directory...)
	@rm -rf $(MIGRATIONS_OUT_DIR)
	@$(call log,Embedded migrations directory deleted.)
	@$(call log,Deleting a generated database schema file...)
	@rm -f $(SCHEMA_OUTPUT)
	@$(call log,Database schema file deleted.)
	@$(call log,Ensuring migrations directory exists...)
	@mkdir -p $(MIGRATIONS_OUT_DIR)
	@$(call log,Copying migrations to $(MIGRATIONS_OUT_DIR)...)
	@cp -r $(MIGRATIONS_IN_DIR)/* $(MIGRATIONS_OUT_DIR)
	@$(call log,Migrations copied successfully.)
	@$(call log,Running migrations...)
	@DATABASE_URL=$(POSTGRES_URL) diesel migration run
	@$(call log,Migrations applied successfully.)
	@$(call log,Printing database schema...)
	@DATABASE_URL=$(POSTGRES_URL) diesel print-schema > $(SCHEMA_OUTPUT)
	@$(call log,Schema updated successfully in $(SCHEMA_OUTPUT))

.PHONY: clear-migrations
clear-migrations: ## Reverts all database migrations.
	@$(call log,Deleting copied migrations...)
	@rm -rf $(MIGRATIONS_OUT_DIR)
	@$(call log,Copied migrations deleted.)
	@$(call log,Reverting all migrations...)
	@while DATABASE_URL=$(POSTGRES_URL) diesel migration list | grep -q "\\[X\\]"; do \
		$(call log,Reverting migration...); \
		DATABASE_URL=$(POSTGRES_URL) diesel migration revert; \
	done
	@$(call log,All migrations reverted successfully.)

.PHONY: reset-docker
reset-docker: ## Resets Docker containers (down -v, then up -d).
	@$(call log,Stopping and removing Docker containers and volumes...)
	@docker compose -f ./docker/docker-compose.dev.yml down -v
	@$(call log,Starting Docker containers...)
	@docker compose -f ./docker/docker-compose.dev.yml up -d
	@$(call log,Docker containers reset successfully.)

.PHONY: generate-all
generate-all: generate-env generate-keys generate-migrations

.PHONY: all
all: install-all generate-all

# CI Commands (mirror GitHub Actions)
.PHONY: ci
ci: ## Runs all CI checks locally (check, fmt, clippy, test, docs).
	@$(call log,Running cargo check...)
	@cargo check --all-features --workspace
	@$(call log,Checking code formatting...)
	@cargo +nightly fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --all-targets --all-features --workspace -- -D warnings
	@$(call log,Running tests...)
	@cargo test --all-features --workspace
	@$(call log,Building documentation...)
	@RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
	@$(call log,All CI checks passed!)

.PHONY: fmt
fmt: ## Fixes code formatting.
	@$(call log,Fixing code formatting...)
	@cargo +nightly fmt --all
	@$(call log,Formatting fixed!)

# Security Commands (mirror GitHub Actions)
.PHONY: security
security: ## Runs security checks locally (cargo deny).
	@$(call log,Running cargo deny...)
	@cargo deny check all
	@$(call log,All security checks passed!)
