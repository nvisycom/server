# Database Migrations

This directory contains all database migrations for the application,
implementing a comprehensive, production-ready schema with advanced features for
scalability, security, and maintainability.

## Guidelines

- Migrations are append-only. Once a migration is merged into the `main` branch,
  do not modify it.
- Migrations in `migrations/` must be idempotent, ensuring they can be run
  multiple times without causing issues.
- Self-hosted service users should update role passwords manually after running
  all migrations.
- Production releases are created by publishing a new GitHub release from the
  `main` branch.

## Corresponding Down Migrations

Each migration includes a comprehensive down migration that:

- Drops objects in reverse dependency order
- Includes all created objects (tables, types, functions, views, triggers)
- Uses IF EXISTS to prevent errors during rollback

## Best Practices

### 1. Naming Conventions

- **Tables**: `snake_case`, descriptive nouns
- **Columns**: `snake_case`, clear and concise
- **Indexes**: `table_purpose_idx` format
- **Constraints**: `table_column_constraint_type` format
- **Functions**: `snake_case` with descriptive verbs
- **Enums**: `UPPER_CASE` with descriptive names

### 2. Data Types and Sizing

- **UUIDs**: Primary keys for external references
- **BIGSERIAL**: Internal sequential IDs where needed
- **TEXT**: Variable length strings with CHECK constraints for limits
- **JSONB**: Structured data with size limits
- **TIMESTAMPTZ**: All timestamps with timezone awareness
- **DECIMAL**: Precise numeric values for financial data

### 3. Relationship Management

- **Foreign Keys**: Always include appropriate CASCADE/SET NULL rules
- **Self-referencing**: Support for hierarchical structures where needed
- **Many-to-many**: Explicit junction tables with additional metadata
- **Soft relationships**: References that survive deletions where appropriate

### 4. Error Handling and Resilience

- **Graceful failures**: Functions that handle errors appropriately
- **Transaction safety**: All operations designed for ACID compliance
- **Rollback support**: Complete down migrations for all changes
- **Data preservation**: Soft deletion patterns to prevent data loss
