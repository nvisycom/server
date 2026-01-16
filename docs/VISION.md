# Vision & Design Principles

## Problem Statement

Document intelligence tools typically treat files as the unit of work. This approach prevents cross-file reasoning, entity resolution across documents, and institutional memory accumulation.

Nvisy addresses this by transforming uploaded files into structured, normalized representations. The knowledge graph—not the files—is the primary asset.

## Design Principles

### 1. Structure Over Blobs

Every file type is converted into machine-readable structure containing both content and structure (headings, tables, sections, entities). Raw files are archived; structured representations are the working data.

### 2. Canonical Representation

A single internal schema normalizes all source formats. This enables comparisons across documents, unified search, and cross-file reasoning regardless of original file type.

### 3. Grounded Reasoning

Every conclusion links back to source material: file, section, exact text, and confidence score. Without provenance, enterprise users cannot validate or trust outputs.

### 4. Isolation & Trust

Cross-file intelligence requires strict isolation:
- Tenant-aware embeddings (tenant data never mixed)
- Permission-filtered retrieval (filter before search, not after)
- Comprehensive audit logs
- Provenance tracking

### 5. Time Awareness

Documents evolve. The system maintains versioned representations and supports semantic diffing (changes in meaning, not just text) and temporal queries across document history.

## Core Capabilities

| Capability | Description |
|------------|-------------|
| Reading | Parse and normalize any supported file format |
| Search | Hybrid search combining vector, symbolic, and graph queries |
| Comparison | Identify differences, conflicts, and drift across documents |
| Extraction | Pull entities, tables, claims, and structured data |

## Differentiation

The knowledge graph compounds over time. Tools that process files in isolation cannot replicate:
- Evolving cross-file graphs
- Entity resolution across time and authors
- Institutional memory accumulation
- Continuous learning from document corpus
