# Intelligence Layer

## Cross-Document Linking

Related content across files must be explicitly linked. This is relationship modeling, not retrieval.

| Technique | Purpose |
|-----------|---------|
| Entity resolution | Same person/company across files |
| Concept embeddings | Same idea, different wording |
| Citation graphs | What references what |
| Contradiction detection | Conflicting statements across documents |

## Hybrid Search

Vector search alone is insufficient for cross-file queries.

| Layer | Purpose | Example |
|-------|---------|---------|
| Vector search | Semantic similarity | "Find clauses about liability" |
| Symbolic filters | Dates, types, authors | "After 2021", "Type: NDA" |
| Graph traversal | Relationships | "Related to Company X" |

A query like "Show me all NDA clauses after 2021 that conflict with policy X" requires all three layers.

## Temporal Intelligence

| Capability | Description |
|------------|-------------|
| Versioned representations | Track document evolution |
| Semantic diffing | Changes in meaning, not just text |
| Temporal queries | "What changed since last quarter?" |
| Change attribution | Who changed what and when |

## Grounded Reasoning

Every assertion links to evidence: file, section, exact text, and relevance score. Without this, enterprise users cannot validate conclusions.

## Cross-File Reasoning Patterns

Reusable patterns across any document set:

| Pattern | Question | Example |
|---------|----------|---------|
| Consistency | Do all docs use the same definition? | "Is 'confidential' defined consistently?" |
| Coverage | Is X addressed somewhere? | "Do all contracts have termination clauses?" |
| Conflict | Do any statements contradict? | "Are there conflicting liability terms?" |
| Redundancy | Are we repeating ourselves? | "Is the same clause duplicated?" |
| Completeness | What's missing? | "Which required sections are absent?" |
| Drift | Has X changed from the standard? | "How does this differ from the template?" |

## Entity Resolution

The same entity appears differently across files.

| Challenge | Example |
|-----------|---------|
| Name variations | "IBM", "International Business Machines", "Big Blue" |
| Role changes | "John Smith (CEO)" vs "John Smith (Board Member)" |
| Temporal | "Acme Corp" acquired by "MegaCorp" in 2022 |
| Abbreviations | "NDA", "Non-Disclosure Agreement" |

Resolution process: extraction → clustering → disambiguation → linking → propagation.

## Knowledge Graph

Entities link to Sections. Sections reference Sections. Documents relate to Documents. This graph grows over time and cannot be replicated by tools that process files in isolation.
