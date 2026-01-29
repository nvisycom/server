"""Parameter types for provider configuration.

Params define how providers operate (columns, batch sizes, etc.),
while contexts carry runtime state (cursors, tokens).

Generated from Rust schemas. Do not edit manually.
"""

from enum import Enum

from pydantic import BaseModel, Field


class RelationalParams(BaseModel, frozen=True):
    """Common parameters for relational database operations."""

    table: str
    """Target table name."""

    columns: list[str] | None = None
    """Columns to select. If None, selects all columns."""

    cursor_column: str | None = None
    """Column to use for cursor-based pagination (e.g., "id", "created_at")."""

    tiebreaker_column: str | None = None
    """Column to use as tiebreaker when cursor values are not unique (e.g., "id")."""

    batch_size: int = Field(default=1000)
    """Default batch size for bulk operations."""


class ObjectParams(BaseModel, frozen=True):
    """Common parameters for object storage operations."""

    bucket: str
    """Bucket name (S3 bucket, GCS bucket, Azure container)."""

    prefix: str | None = None
    """Default prefix for object keys."""

    batch_size: int = Field(default=1000)
    """Default batch size for bulk operations."""


class DistanceMetric(str, Enum):
    """Distance metric for vector similarity search."""

    COSINE = "cosine"
    """Cosine similarity (default)."""

    EUCLIDEAN = "euclidean"
    """Euclidean distance (L2)."""

    DOT_PRODUCT = "dot_product"
    """Dot product."""


class VectorParams(BaseModel, frozen=True):
    """Common parameters for vector database operations."""

    collection: str
    """Collection or index name (Pinecone index, Qdrant collection)."""

    dimension: int | None = None
    """Dimension of vectors (required for some providers)."""

    metric: DistanceMetric = DistanceMetric.COSINE
    """Distance metric for similarity search."""

    batch_size: int = Field(default=1000)
    """Default batch size for bulk operations."""
