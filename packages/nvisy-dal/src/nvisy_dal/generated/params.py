"""Parameter types for provider configuration.

Generated from Rust schemas. Do not edit manually.
"""

from enum import Enum

from pydantic import BaseModel, Field


class RelationalParams(BaseModel, frozen=True):
    """Common parameters for relational database operations."""

    table: str | None = None
    cursor_column: str | None = None
    tiebreaker_column: str | None = None
    batch_size: int = Field(default=1000)


class ObjectParams(BaseModel, frozen=True):
    """Common parameters for object storage operations."""

    bucket: str | None = None
    prefix: str | None = None
    batch_size: int = Field(default=1000)


class DistanceMetric(str, Enum):
    """Distance metric for vector similarity search."""

    COSINE = "cosine"
    EUCLIDEAN = "euclidean"
    DOT_PRODUCT = "dot_product"


class VectorParams(BaseModel, frozen=True):
    """Common parameters for vector database operations."""

    collection: str | None = None
    dimension: int | None = None
    metric: DistanceMetric = DistanceMetric.COSINE
    batch_size: int = Field(default=1000)
