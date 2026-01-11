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

# Make-level logger (evaluated by make; does not invoke the shell).
define make-log
$(info [$(shell date '+%Y-%m-%d %H:%M:%S')] [MAKE] [$(MAKECMDGOALS)] $(1))
endef

# Shell-level logger (expands to a printf that runs in the shell).
define shell-log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

.PHONY: Install-tools
install-tools: # Installs tools required for the repo.
	$(call make-log,Checking Diesel CLI...)
	@# Use a shell if-block; call $(call shell-log,...) inside so shell sees a valid command.
	@if ! command -v diesel >/dev/null 2>&1; then \
		$(call shell-log,Installing Diesel CLI with PostgreSQL support...); \
		cargo install diesel_cli --no-default-features --features postgres --locked; \
		$(call shell-log,Diesel CLI installed successfully.); \
	else \
		$(call shell-log,Diesel CLI already available: $$(diesel --version)); \
		$(call shell-log,Verifying PostgreSQL support...); \
		if ! diesel --version | grep -q postgres; then \
			$(call shell-log,Reinstalling Diesel CLI with PostgreSQL support...); \
			cargo install diesel_cli --no-default-features --features postgres --locked --force; \
		fi; \
	fi

.PHONY: install-all
install-all: install-tools # Installs all dependencies.
	$(call make-log,Making scripts executable...)
	@chmod +x scripts/*.sh
	$(call make-log,Scripts made executable!)

.PHONY: generate-env
generate-env: ## Copies .env.example to .env.
	$(call make-log,Copying .env.example to .env...)
	@cp ./.env.example ./.env
	$(call make-log,.env file created successfully.)

.PHONY: generate-keys
generate-keys: ## Generates a private and public auth key pair.
	$(call make-log,Deleting previously generated keys...)
	@rm -f $(PRIVATE_KEY_FILE) $(PUBLIC_KEY_FILE)
	$(call make-log,Previously generated keys deleted.)

	$(call make-log,Generating private key...)
	@openssl genpkey -algorithm ed25519 -out $(PRIVATE_KEY_FILE)
	$(call make-log,Private key generated successfully.)

	$(call make-log,Generating public key...)
	@openssl pkey -in $(PRIVATE_KEY_FILE) -pubout -out $(PUBLIC_KEY_FILE)
	$(call make-log,Public key generated successfully.)

.PHONY: generate-migrations
generate-migrations: ## Regenerates the Postgres migrations and database schema.
	$(call make-log,Deleting embedded migrations directory...)
	@rm -rf $(MIGRATIONS_OUT_DIR)
	$(call make-log,Embedded migrations directory deleted.)
	$(call make-log,Deleting a generated database schema file...)
	@rm -f $(SCHEMA_OUTPUT)
	$(call make-log,Database schema file deleted.)

	$(call make-log,Ensuring migrations directory exists...)
	@mkdir -p $(MIGRATIONS_OUT_DIR)
	$(call make-log,Copying migrations to $(MIGRATIONS_OUT_DIR)...)
	@cp -r $(MIGRATIONS_IN_DIR)/* $(MIGRATIONS_OUT_DIR)
	$(call make-log,Migrations copied successfully.)

	$(call make-log,Running migrations...)
	@DATABASE_URL=$(POSTGRES_URL) diesel migration run
	$(call make-log,Migrations applied successfully.)
	$(call make-log,Printing database schema...)
	@DATABASE_URL=$(POSTGRES_URL) diesel print-schema > $(SCHEMA_OUTPUT)
	$(call make-log,Schema updated successfully in $(SCHEMA_OUTPUT))

.PHONY: clear-migrations
clear-migrations: ## Reverts all database migrations.
	$(call make-log,Deleting copied migrations...)
	@rm -rf $(MIGRATIONS_OUT_DIR)
	$(call make-log,Copied migrations deleted.)

	$(call make-log,Reverting all migrations...)
	@while DATABASE_URL=$(POSTGRES_URL) diesel migration list | grep -q "\\[X\\]"; do \
		$(call shell-log,Reverting migration...); \
		DATABASE_URL=$(POSTGRES_URL) diesel migration revert; \
	done
	$(call make-log,All migrations reverted successfully.)

.PHONY: generate-all
generate-all: generate-env generate-keys generate-migrations

.PHONY: all
all: install-all generate-all

# CI Commands (mirror GitHub Actions)
.PHONY: ci
ci: ## Runs all CI checks locally (check, fmt, clippy, test, docs).
	$(call make-log,Running cargo check...)
	@cargo check --all-features --workspace
	$(call make-log,Checking code formatting...)
	@cargo +nightly fmt --all -- --check
	$(call make-log,Running clippy...)
	@cargo clippy --all-targets --all-features --workspace -- -D warnings
	$(call make-log,Running tests...)
	@cargo test --all-features --workspace
	$(call make-log,Building documentation...)
	@RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
	$(call make-log,All CI checks passed!)

.PHONY: fmt
fmt: ## Fixes code formatting.
	$(call make-log,Fixing code formatting...)
	@cargo +nightly fmt --all
	$(call make-log,Formatting fixed!)

# Security Commands (mirror GitHub Actions)
.PHONY: security
security: ## Runs security checks locally (audit, deny).
	$(call make-log,Running cargo deny...)
	@cargo deny check all
	$(call make-log,All security checks passed!)
