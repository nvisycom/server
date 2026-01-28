"""Milvus vector database provider."""

from collections.abc import Sequence
from typing import TYPE_CHECKING, ClassVar, Self

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind
from nvisy_dal.generated.datatypes import Embedding
from nvisy_dal.generated.params import VectorParams

if TYPE_CHECKING:
    from pymilvus import MilvusClient  # pyright: ignore[reportMissingTypeStubs]

try:
    from pymilvus import MilvusClient  # pyright: ignore[reportMissingTypeStubs]
except ImportError as e:
    _msg = "pymilvus is required for Milvus support. Install with: uv add 'nvisy-dal[milvus]'"
    raise ImportError(_msg) from e


class MilvusCredentials(BaseModel, frozen=True):
    """Credentials for Milvus connection."""

    uri: str
    """Milvus server URI (e.g., 'http://localhost:19530' or Zilliz Cloud URI)."""

    token: str | None = None
    """API token for Zilliz Cloud or secured instances."""


class MilvusParams(VectorParams, frozen=True):
    """Parameters for Milvus operations.

    Inherits `collection`, `dimension`, `metric`, and `batch_size` from VectorParams.
    """


class MilvusProvider:
    """Milvus provider for vector database operations.

    Implements Provider[MilvusCredentials, MilvusParams] and
    DataOutput[Embedding, VectorContext].
    """

    __slots__: ClassVar[tuple[str, str]] = ("_client", "_params")

    _client: "MilvusClient"
    _params: MilvusParams

    def __init__(self, client: "MilvusClient", params: MilvusParams) -> None:
        self._client = client
        self._params = params

    @classmethod
    async def connect(cls, credentials: MilvusCredentials, params: MilvusParams) -> Self:
        """Create Milvus client and verify connection."""
        try:
            client = MilvusClient(
                uri=credentials.uri,
                token=credentials.token or "",
            )
            if not await client.has_collection(params.collection):  # pyright: ignore[reportUnknownMemberType]
                msg = f"Collection '{params.collection}' not found"
                raise DalError(msg, kind=ErrorKind.NOT_FOUND)  # noqa: TRY301
        except DalError:
            raise
        except Exception as e:
            msg = f"Failed to connect to Milvus: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, params)

    async def disconnect(self) -> None:
        """Close the Milvus client."""
        self._client.close()

    async def write(self, items: Sequence[Embedding]) -> None:
        """Write embeddings to Milvus."""
        if not items:
            return

        try:
            data = [{"id": e.id, "vector": e.vector, **e.metadata} for e in items]
            for i in range(0, len(data), self._params.batch_size):
                batch = data[i : i + self._params.batch_size]
                self._client.upsert(  # pyright: ignore[reportUnknownMemberType,reportUnusedCallResult]
                    collection_name=self._params.collection,
                    data=batch,
                )
        except Exception as e:
            msg = f"Failed to write to Milvus: {e}"
            raise DalError(msg, source=e) from e


Provider = MilvusProvider
