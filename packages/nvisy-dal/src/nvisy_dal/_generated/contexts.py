"""Context types for provider operations.

Generated from Rust schemas. Do not edit manually.
"""

from pydantic import BaseModel


class ObjectContext(BaseModel, frozen=True):
    """Context for object storage operations."""

    prefix: str | None = None
    continuation_token: str | None = None
    limit: int | None = None


class RelationalContext(BaseModel, frozen=True):
    """Context for relational database operations."""

    table: str
    cursor: str | None = None
    tiebreaker: str | None = None
    limit: int | None = None


class VectorContext(BaseModel, frozen=True):
    """Context for vector store operations."""

    collection: str
    cursor: str | None = None
    limit: int | None = None
