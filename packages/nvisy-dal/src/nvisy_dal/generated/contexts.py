"""Context types for provider operations.

Generated from Rust schemas. Do not edit manually.
"""

from pydantic import BaseModel


class ObjectContext(BaseModel, frozen=True):
    """Context for object storage operations (S3, GCS, Azure Blob)."""

    prefix: str | None = None
    token: str | None = None
    limit: int | None = None


class RelationalContext(BaseModel, frozen=True):
    """Context for relational database operations (Postgres, MySQL)."""

    cursor: str | None = None
    tiebreaker: str | None = None
    limit: int | None = None


class VectorContext(BaseModel, frozen=True):
    """Context for vector database operations (Qdrant, Pinecone, pgvector)."""

    token: str | None = None
    limit: int | None = None
