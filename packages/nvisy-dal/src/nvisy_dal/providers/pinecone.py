"""Pinecone vector database provider."""

from collections.abc import Mapping, Sequence
from typing import TYPE_CHECKING, ClassVar, Self, cast

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind

if TYPE_CHECKING:
    from pinecone import Pinecone
    from pinecone.db_data.index import Index

try:
    from pinecone import Pinecone, UpsertResponse, Vector
except ImportError as e:
    _msg = "pinecone is required for Pinecone support. Install with: uv add 'nvisy-dal[pinecone]'"
    raise ImportError(_msg) from e

# Pinecone metadata value types
type MetadataValue = str | int | float | list[str] | list[int] | list[float]
type Metadata = Mapping[str, MetadataValue]


class PineconeCredentials(BaseModel):
    """Credentials for Pinecone connection."""

    api_key: str


class PineconeParams(BaseModel):
    """Parameters for Pinecone operations."""

    index_name: str
    namespace: str = ""


class PineconeVector(BaseModel):
    """Representation of a Pinecone vector."""

    id: str
    values: list[float]
    metadata: dict[str, MetadataValue] | None = None


class PineconeProvider:
    """Pinecone provider for vector upsert operations."""

    __slots__: ClassVar[tuple[str, str, str]] = ("_client", "_index", "_params")

    _client: "Pinecone"
    _index: "Index"
    _params: PineconeParams

    def __init__(self, client: "Pinecone", index: "Index", params: PineconeParams) -> None:
        self._client = client
        self._index = index
        self._params = params

    @classmethod
    async def connect(cls, credentials: PineconeCredentials, params: PineconeParams) -> Self:
        """Create Pinecone client and connect to index."""
        try:
            client = Pinecone(api_key=credentials.api_key)
            index = client.Index(params.index_name)  # pyright: ignore[reportUnknownMemberType]
            # Verify connection
            _ = index.describe_index_stats()  # pyright: ignore[reportUnknownMemberType]
        except Exception as e:
            msg = f"Failed to connect to Pinecone: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, index, params)

    async def disconnect(self) -> None:
        """Close the Pinecone client (no-op)."""

    async def upsert(self, vectors: Sequence[PineconeVector]) -> int:
        """Upsert vectors to Pinecone. Returns count of upserted vectors."""
        if not vectors:
            return 0

        try:
            records = [Vector(id=v.id, values=v.values, metadata=v.metadata) for v in vectors]

            upserted = 0
            batch_size = 100
            for i in range(0, len(records), batch_size):
                batch = list(records[i : i + batch_size])
                response = cast(
                    "UpsertResponse",
                    self._index.upsert(  # pyright: ignore[reportUnknownMemberType]
                        vectors=batch,
                        namespace=self._params.namespace,
                    ),
                )
                upserted += response.upserted_count or len(batch)
        except Exception as e:
            msg = f"Failed to upsert to Pinecone: {e}"
            raise DalError(msg, source=e) from e
        else:
            return upserted


Provider = PineconeProvider
