# Why `workspace_reviews` keeps a `workspace_id` column

`workspace_reviews.workspace_id` looks redundant — a review points at a
`document_id`, and a document already carries its `workspace_id`, so the
workspace is reachable transitively. It is kept anyway, deliberately, because the
column is **load-bearing**, not a convenience denormalization.

## Reason 1 — it anchors a same-workspace composite foreign key

The review does not reference a document with a plain `document_id -> documents(id)`
FK. It uses a **composite** FK:

```sql
CONSTRAINT workspace_reviews_document_fkey FOREIGN KEY (workspace_id, document_id)
    REFERENCES workspace_documents (workspace_id, id) ON DELETE CASCADE
```

This enforces "review -> *this document in this workspace*". Without the
`workspace_id` column, nothing at the database level would stop a review row from
holding `workspace_id = A` while its document lives in workspace B. The composite
FK makes that mismatch **unrepresentable** — the review's `workspace_id` is proven
equal to its document's. Dropping the column would *remove* an integrity
guarantee, not just a duplicate value.

## Reason 2 — it leads the review-queue index

```sql
workspace_reviews_workspace_idx ON workspace_reviews (workspace_id, review_status, created_at DESC)
    WHERE deleted_at IS NULL
```

The review queue ("this workspace's reviews by status, newest first") filters
`workspace_id` directly. Reaching workspace transitively would force every queue
read to JOIN `workspace_documents` on the hottest review path. The column carries
that scope for free.

## Contrast: the review *children* do NOT keep `workspace_id`

`workspace_review_comments` and `workspace_review_events` had their `workspace_id`
removed (normalized) — because for them the column had **no composite FK and no
leading index**; it was pure redundancy, and they scope through `review_id`. The
rule is: keep a denormalized `workspace_id` only where it (a) anchors a
same-scope composite FK or (b) leads a hot scoping index. `workspace_reviews`
qualifies on both counts; its children on neither.

(Same reasoning kept `workspace_id` — via composite FKs — on
`workspace_policy_versions` and `workspace_pipeline_policies`, and kept it on
`workspace_detections` for its per-workspace idempotency unique index.
`workspace_audits` does NOT carry it: its `detection_id` FK is plain, no composite,
so audits scope transitively through the detection.)
