"""Data types for the DAL.

These types represent the data items that flow through providers:
- `Object` for object storage (S3, GCS, Azure Blob)
- `Document` for JSON documents
- `Embedding` for vector embeddings
- `Record` for relational rows
- `Message` for queue/stream messages
- `Graph`, `Node`, `Edge` for graph data

Generated from Rust schemas. Do not edit manually.
"""

from pydantic import BaseModel, Field

# JSON-compatible value type (matches serde_json::Value)
type JsonValue = str | int | float | bool | None | list["JsonValue"] | dict[str, "JsonValue"]

# Metadata associated with data items.
type Metadata = dict[str, JsonValue]


class Object(BaseModel):
    """An object representing a file or binary data (S3, GCS, Azure Blob)."""

    path: str
    """Path or key identifying this object."""

    data: bytes
    """Raw binary data."""

    content_type: str | None = None
    """Content type (MIME type)."""

    metadata: Metadata = Field(default_factory=dict)
    """Additional metadata."""


class Document(BaseModel):
    """A document with flexible JSON content."""

    id: str
    """Unique identifier."""

    content: JsonValue
    """Document content as JSON."""

    metadata: Metadata = Field(default_factory=dict)
    """Additional metadata."""


class Embedding(BaseModel):
    """A vector embedding with metadata."""

    id: str
    """Unique identifier."""

    vector: list[float]
    """The embedding vector."""

    metadata: Metadata = Field(default_factory=dict)
    """Additional metadata."""


class Record(BaseModel):
    """A record representing a row in a relational table."""

    columns: dict[str, JsonValue] = Field(default_factory=dict)
    """Column values keyed by column name."""


class Message(BaseModel):
    """A message from a queue or stream."""

    id: str
    """Unique identifier."""

    payload: bytes
    """Message payload."""

    headers: dict[str, str] = Field(default_factory=dict)
    """Message headers."""

    timestamp: str | None = None
    """Timestamp when the message was created."""


class Node(BaseModel):
    """A node in a graph."""

    id: str
    """Unique identifier."""

    labels: list[str] = Field(default_factory=list)
    """Node labels (types)."""

    properties: dict[str, JsonValue] = Field(default_factory=dict)
    """Node properties."""


class Edge(BaseModel):
    """An edge in a graph."""

    id: str
    """Unique identifier."""

    from_: str = Field(alias="from")
    """Source node ID."""

    to: str
    """Target node ID."""

    label: str
    """Edge label (relationship type)."""

    properties: dict[str, JsonValue] = Field(default_factory=dict)
    """Edge properties."""


class Graph(BaseModel):
    """A graph containing nodes and edges."""

    nodes: list[Node] = Field(default_factory=list)
    """Nodes in the graph."""

    edges: list[Edge] = Field(default_factory=list)
    """Edges in the graph."""
