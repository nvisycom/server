# Intelligence

## Overview

Detection and redaction run in-process. The server embeds `elide-pipeline`
from [Elide Runtime](https://github.com/nvisycom/elide-runtime), the stateless
detect/redact library built on the [Elide](https://github.com/nvisycom/elide)
toolkit, and wraps it as the engine service (`service/engine`). The server's
role is to store detection policies and provider credentials, drive the engine
per request, and record the results.

The engine is configured once at startup with the deployment's NER/LLM
recognizer lineups and OCR/STT enricher backends. Deduplication and calibration
are engine-owned defaults, not configured here.

For the detection catalog (deterministic patterns, model-backed recognizers,
OCR/STT enrichers, and conflict resolution), see the Elide and Elide Runtime
documentation.

## What the Server Manages

### Detection Policies

The server stores and versions detection policies per workspace. A policy is
the engine's structured governance document: what to detect and how to redact
it. It carries no sensitive content, so it is stored as plaintext JSONB on its
version row. A detection runs the policies of a pipeline, or an explicit
ad-hoc policy list.

Policies can extend prebuilt regulation templates (HIPAA, GDPR, PCI-DSS, CCPA)
with workspace-specific additions. A detection pins the policy versions it ran
with, so audit trails reference the exact rules that governed each redaction
decision.

### Inference Providers

Model-backed recognizers call inference providers configured per workspace
(language models, named-entity-recognition models). Provider config is
encrypted with workspace-derived keys and never exposed through the API.

### Detection Results

A completed detection stores an encrypted audit: each detected entity, its
location, the recognizer that found it, and its confidence. Results are served
through the API for review workflows and audit reporting. The stored analysis
is immutable.

### Reviewer Edits

Before redacting, a reviewer can suppress a false positive, retag a detection,
or add one the analysis missed. The edits are layered onto a working copy of
the detection's analysis when the redaction runs, so a detection can be
redacted many times.

## Processing Flow

Detection is asynchronous:

1. A request creates the detection and enqueues its job in the same
   transaction (a transactional outbox)
2. The outbox drainer relays pending jobs to the detection NATS JetStream
   work-queue
3. The detection worker consumes the job and runs the policies against the
   document through the engine
4. The worker stores the encrypted audit and marks the detection `Complete`
   (or `Failed`)
5. Each terminal transition is broadcast on the detection's core-NATS status
   subject, streamed to watchers over SSE, and emitted as a webhook event

Redaction is synchronous: a request against a completed detection applies its
findings, with any reviewer edits, using the policy versions the detection
pinned. The redacted file is stored as a new document (kind `redacted`), along
with the redaction's own review audit.
