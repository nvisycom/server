# Intelligence

Nvisy treats intelligence as a pipeline concern, not a separate layer.
AI-powered transforms sit alongside rule-based processors in the workflow graph,
operating on data as it flows from sources to sinks. Every transform is a node
that accepts input, produces output, and can be composed with any other node.

## Document Processing

Before content can be analyzed or enriched, it must be decomposed into
structured elements.

**Partitioning** breaks documents into typed elements — paragraphs, tables,
images, headers, list items — using either fast rule-based heuristics or
ML-based layout detection. For complex layouts, a vision-language model can be
used to interpret page structure directly from rendered images.

**Chunking** splits partitioned elements into smaller segments suitable for
embedding and retrieval. Strategies include fixed-size character windows,
page-boundary splits, section-aware splits that respect document headings, and
semantic similarity splits that group related content. Optionally, an LLM can
generate contextual summaries per chunk to improve downstream retrieval quality.

## Extraction

Extraction transforms convert unstructured content into structured
representations.

**Format conversion** handles tables and text. Tables can be converted to HTML,
Markdown, CSV, or JSON. Text can be converted to JSON or to structured JSON
conforming to a user-provided schema, enabling extraction of arbitrary
structured data from free-form content.

**Analysis** applies NLP tasks to content: named entity recognition (people,
places, organizations, dates, amounts), keyword extraction, text classification
against user-provided labels, sentiment analysis, and relationship extraction
between identified entities.

## Enrichment

Enrichment adds metadata and descriptions that were not present in the source
material.

**Table enrichment** generates natural language descriptions of table contents
and per-column descriptions, making tabular data searchable and understandable
without reading the raw data.

**Image enrichment** generates descriptions at varying levels of detail — brief
summaries, detailed descriptions covering people, objects, text, colors, and
layout. Generative OCR extracts text from images using vision models rather than
traditional OCR engines. Object detection identifies and lists entities present
in the image.

## Derivation

Derivation generates new content from existing input. **Summarization** produces
condensed versions of longer content. **Title generation** creates headings or
labels for untitled content. Both support custom prompt overrides for
domain-specific behavior.

## Embedding

Vector embedding generation converts content into dense numerical
representations for storage in vector databases. The embedding transform
supports configurable models and optional L2 normalization of output vectors.

## Cross-Content Intelligence

Beyond per-element transforms, Nvisy provides higher-order intelligence that
operates across content boundaries.

**Entity resolution** identifies when the same real-world entity appears in
different forms across data — name variations ("IBM" vs. "International Business
Machines"), role changes, acquisitions, and abbreviations. The resolution
pipeline extracts, clusters, disambiguates, links, and propagates entity
identities.

**Contradiction detection** identifies conflicting statements across data from
different sources, surfacing inconsistencies that would otherwise go unnoticed
in large document sets.

**Consistency checking** verifies that definitions and terms are used uniformly
across content — for example, ensuring that "confidential" is defined the same
way in every contract.

**Coverage analysis** determines whether required topics, clauses, or sections
are addressed across a body of content, identifying gaps and omissions.

**Drift detection** compares content against templates or standards, identifying
where and how a document has deviated from its expected form.

**Semantic diffing** detects changes in meaning across versions of content,
distinguishing substantive changes from cosmetic edits.

**Temporal queries** enable time-aware filtering and analysis — "what changed
since last quarter" or "show me the state of this document as of a specific
date."

## Routing

Switch nodes route data conditionally within the workflow graph, enabling
different processing paths based on data characteristics.

**File category routing** directs data based on content type — text, images,
audio, video, documents, spreadsheets, presentations, code, or archives —
allowing each type to flow through an appropriate processing branch.

**Language routing** directs data based on detected content language, with
configurable confidence thresholds, enabling language-specific processing within
a single workflow.
