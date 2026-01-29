"""Pinecone vector database provider."""

from collections.abc import Sequence
from typing import TYPE_CHECKING, ClassVar, Self, cast

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind
from nvisy_dal.generated.datatypes import Embedding
from nvisy_dal.generated.params import VectorParams

if TYPE_CHECKING:
    from pinecone import Pinecone
    from pinecone.db_data.index import Index

try:
    from pinecone import Pinecone, UpsertResponse, Vector
except ImportError as e:
    _msg = "pinecone is required for Pinecone support. Install with: uv add 'nvisy-dal[pinecone]'"
    raise ImportError(_msg) from e


class PineconeCredentials(BaseModel, frozen=True):
    """Credentials for Pinecone connection."""

    api_key: str


class PineconeParams(VectorParams, frozen=True):
    """Parameters for Pinecone operations.

    Inherits `collection`, `dimension`, `metric`, and `batch_size` from VectorParams.
    """

    namespace: str = ""
    """Pinecone namespace for isolating vectors."""


class PineconeProvider:
    """Pinecone provider for vector database operations.

    Implements Provider[PineconeCredentials, PineconeParams] and
    DataOutput[Embedding].
    """

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
            index = client.Index(params.collection)  # pyright: ignore[reportUnknownMemberType]
            # Verify connection
            _ = index.describe_index_stats()  # pyright: ignore[reportUnknownMemberType]
        except Exception as e:
            msg = f"Failed to connect to Pinecone: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, index, params)

    async def disconnect(self) -> None:
        """Close the Pinecone client (no-op)."""

    async def write(self, items: Sequence[Embedding]) -> None:
        """Write embeddings to Pinecone."""
        if not items:
            return

        try:
            records = [
                Vector(id=e.id, values=e.vector, metadata=e.metadata)  # pyright: ignore[reportArgumentType]
                for e in items
            ]

            for i in range(0, len(records), self._params.batch_size):
                batch = list(records[i : i + self._params.batch_size])
                _ = cast(
                    "UpsertResponse",
                    self._index.upsert(  # pyright: ignore[reportUnknownMemberType]
                        vectors=batch,
                        namespace=self._params.namespace,
                    ),
                )
        except Exception as e:
            msg = f"Failed to write to Pinecone: {e}"
            raise DalError(msg, source=e) from e


Provider = PineconeProvider
