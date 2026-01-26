# Nvisy Documentation

## Overview

Nvisy transforms uploaded files into structured, normalized representations that enable cross-file intelligence. The knowledge graph—not the files—is the primary asset.

## Problem

Document intelligence tools typically treat files as the unit of work. This prevents cross-file reasoning, entity resolution across documents, and institutional memory accumulation.

## Design Principles

| Principle | Description |
|-----------|-------------|
| Structure over blobs | Every file converts to machine-readable structure with content and metadata |
| Canonical representation | Single internal schema normalizes all source formats |
| Grounded reasoning | Every conclusion links to source: file, section, exact text, confidence |
| Isolation & trust | Tenant-aware embeddings, permission-filtered retrieval, audit logs |
| Time awareness | Versioned representations, semantic diffing, temporal queries |

## Core Capabilities

| Capability | Description |
|------------|-------------|
| Reading | Parse and normalize any supported file format |
| Search | Hybrid search combining vector, symbolic, and graph queries |
| Comparison | Identify differences, conflicts, and drift across documents |
| Extraction | Pull entities, tables, claims, and structured data |

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](./ARCHITECTURE.md) | System design, data model, and technology stack |
| [Intelligence](./INTELLIGENCE.md) | Cross-file reasoning, search, and extraction |
| [Providers](./PROVIDERS.md) | Data provider architecture with PyO3 |
