# Intelligence

## Overview

The server does not perform detection or redaction directly. Intelligence
capabilities live in the
[Nvisy Runtime](https://github.com/nvisycom/runtime). The server's role is to
store detection policies, dispatch processing jobs to the runtime over NATS,
and record the results.

For the full detection catalog (deterministic patterns, ML/NLP models, computer
vision, audio detection, orchestration, and conflict resolution), see the
runtime's [Detection](https://github.com/nvisycom/runtime/blob/main/docs/DETECTION.md)
documentation.

## What the Server Manages

### Detection Policies

The server stores and versions detection policies per workspace. A policy is a
named, versioned collection of rules specifying what to detect and how to
handle it. When dispatching a job to the runtime, the server includes the
workspace's active policy.

Policies can extend prebuilt regulation packs (HIPAA, GDPR, PCI-DSS, CCPA)
with workspace-specific additions. The server tracks policy versions so that
audit trails reference the exact rules that governed each redaction decision.

### Context Files

Workspaces can upload context files (stored as encrypted JSON in NATS object
storage) that provide additional input for detection: custom entity
definitions, domain-specific terminology, and organization-specific patterns.
These are passed to the runtime alongside the document and policy when a job
is dispatched.

### Results and Annotations

After the runtime processes a document, the server stores the detection
results: a structured set of annotations describing each detected entity, its
location, the triggering rule or model, and the confidence level. These
annotations are served through the API for review workflows and audit
reporting.

### Review Decisions

The server accepts review decisions (accept, reject, modify) against
individual annotations and passes them back to the runtime for a final
redaction pass. Each review action carries the reviewer identity and timestamp
for the audit trail.

## Job Dispatch

When a redaction job is requested, the server:

1. Decrypts the document and provider credentials
2. Resolves the workspace's active detection policy and context files
3. Dispatches the job to the runtime over NATS JetStream
4. Receives results (annotations, redacted document) from the runtime
5. Stores the redacted document as a new version and records annotations
6. Emits webhook events for processing lifecycle subscribers
