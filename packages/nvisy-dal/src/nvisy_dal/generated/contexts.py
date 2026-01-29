"""Context types for data operations.

Contexts carry state needed to resume reading from a specific position.
They only track *where* to resume, not *how much* to read (that's in Params).

Generated from Rust schemas. Do not edit manually.
"""

from pydantic import BaseModel


class ObjectContext(BaseModel, frozen=True):
    """Context for object storage operations (S3, GCS, Azure Blob).

    Uses marker-based pagination (last seen key) which is portable across
    S3, GCS, Azure Blob, and MinIO.
    """

    prefix: str | None = None
    """Path prefix for listing objects."""

    token: str | None = None
    """Last seen object key (used as StartAfter/marker for resumption)."""


class RelationalContext(BaseModel, frozen=True):
    """Context for relational database operations (Postgres, MySQL).

    Uses keyset pagination which is more efficient than offset-based
    pagination for large datasets and provides stable results.
    """

    cursor: str | None = None
    """Last seen cursor value (for keyset pagination)."""

    tiebreaker: str | None = None
    """Tiebreaker value for resolving cursor conflicts."""


class VectorContext(BaseModel, frozen=True):
    """Context for vector database operations (Qdrant, Pinecone, pgvector)."""

    token: str | None = None
    """Continuation token or offset for pagination."""
