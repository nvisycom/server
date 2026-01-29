"""Qdrant vector database provider."""

from collections.abc import Sequence
from typing import TYPE_CHECKING, ClassVar, Self

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind
from nvisy_dal.generated.datatypes import Embedding
from nvisy_dal.generated.params import VectorParams

if TYPE_CHECKING:
    from qdrant_client import AsyncQdrantClient

try:
    from qdrant_client import AsyncQdrantClient, models
except ImportError as e:
    _msg = "qdrant-client is required for Qdrant support. Install with: uv add 'nvisy-dal[qdrant]'"
    raise ImportError(_msg) from e


class QdrantCredentials(BaseModel, frozen=True):
    """Credentials for Qdrant connection."""

    url: str
    """Qdrant server URL (e.g., 'http://localhost:6333' or cloud URL)."""

    api_key: str | None = None
    """API key for Qdrant Cloud or secured instances."""


class QdrantParams(VectorParams, frozen=True):
    """Parameters for Qdrant operations.

    Inherits `collection`, `dimension`, `metric`, and `batch_size` from VectorParams.
    """


class QdrantProvider:
    """Qdrant provider for vector database operations.

    Implements Provider[QdrantCredentials, QdrantParams] and
    DataOutput[Embedding, VectorContext].
    """

    __slots__: ClassVar[tuple[str, str]] = ("_client", "_params")

    _client: "AsyncQdrantClient"
    _params: QdrantParams

    def __init__(self, client: "AsyncQdrantClient", params: QdrantParams) -> None:
        self._client = client
        self._params = params

    @classmethod
    async def connect(cls, credentials: QdrantCredentials, params: QdrantParams) -> Self:
        """Create Qdrant client and verify connection."""
        try:
            client = AsyncQdrantClient(
                url=credentials.url,
                api_key=credentials.api_key,
            )
            if not await client.collection_exists(params.collection):
                msg = f"Collection '{params.collection}' not found"
                raise DalError(msg, kind=ErrorKind.NOT_FOUND)  # noqa: TRY301
        except DalError:
            raise
        except Exception as e:
            msg = f"Failed to connect to Qdrant: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, params)

    async def disconnect(self) -> None:
        """Close the Qdrant client."""
        await self._client.close()

    async def write(self, items: Sequence[Embedding]) -> None:
        """Write embeddings to Qdrant."""
        if not items:
            return

        try:
            points = [
                models.PointStruct(id=e.id, vector=e.vector, payload=e.metadata)
                for e in items
            ]
            for i in range(0, len(points), self._params.batch_size):
                batch = points[i : i + self._params.batch_size]
                _ = await self._client.upsert(
                    collection_name=self._params.collection,
                    points=batch,
                )
        except Exception as e:
            msg = f"Failed to write to Qdrant: {e}"
            raise DalError(msg, source=e) from e


Provider = QdrantProvider
