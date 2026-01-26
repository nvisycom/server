"""Generated types from Rust JSON schemas.

This module contains Pydantic models generated from the Rust schema definitions.
Do not edit manually - regenerate with `make codegen`.
"""

from nvisy_dal.generated.contexts import ObjectContext, RelationalContext, VectorContext
from nvisy_dal.generated.datatypes import (
    Document,
    Edge,
    Embedding,
    Graph,
    JsonValue,
    Message,
    Metadata,
    Node,
    Object,
    Record,
)
from nvisy_dal.generated.params import (
    DistanceMetric,
    ObjectParams,
    RelationalParams,
    VectorParams,
)

__all__ = [
    # Contexts (runtime state)
    "ObjectContext",
    "RelationalContext",
    "VectorContext",
    # Params (configuration)
    "DistanceMetric",
    "ObjectParams",
    "RelationalParams",
    "VectorParams",
    # Data types
    "Document",
    "Edge",
    "Embedding",
    "Graph",
    "JsonValue",
    "Message",
    "Metadata",
    "Node",
    "Object",
    "Record",
]
