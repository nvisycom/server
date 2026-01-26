"""Data types for provider input/output.

Generated from Rust schemas. Do not edit manually.
"""

from pydantic import BaseModel, Field

# JSON-compatible value type (matches serde_json::Value)
type JsonValue = str | int | float | bool | None | list["JsonValue"] | dict[str, "JsonValue"]

# Metadata associated with data items
type Metadata = dict[str, JsonValue]


class Object(BaseModel):
    """An object representing a file or binary data (S3, GCS, Azure Blob)."""

    path: str
    data: bytes
    content_type: str | None = None
    metadata: Metadata = Field(default_factory=dict)


class Document(BaseModel):
    """A document with flexible JSON content."""

    id: str
    content: JsonValue
    metadata: Metadata = Field(default_factory=dict)


class Embedding(BaseModel):
    """A vector embedding with metadata."""

    id: str
    vector: list[float]
    metadata: Metadata = Field(default_factory=dict)


class Record(BaseModel):
    """A record representing a row in a relational table."""

    columns: dict[str, JsonValue] = Field(default_factory=dict)


class Message(BaseModel):
    """A message from a queue or stream."""

    id: str
    payload: bytes
    headers: dict[str, str] = Field(default_factory=dict)
    timestamp: str | None = None


class Node(BaseModel):
    """A node in a graph."""

    id: str
    labels: list[str] = Field(default_factory=list)
    properties: dict[str, JsonValue] = Field(default_factory=dict)


class Edge(BaseModel):
    """An edge in a graph."""

    id: str
    from_: str = Field(alias="from")
    to: str
    label: str
    properties: dict[str, JsonValue] = Field(default_factory=dict)


class Graph(BaseModel):
    """A graph containing nodes and edges."""

    nodes: list[Node] = Field(default_factory=list)
    edges: list[Edge] = Field(default_factory=list)
