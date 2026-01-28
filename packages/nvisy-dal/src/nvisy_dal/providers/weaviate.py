"""Weaviate vector database provider."""

from collections.abc import Sequence
from typing import TYPE_CHECKING, ClassVar, Self, cast

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind
from nvisy_dal.generated.datatypes import Embedding
from nvisy_dal.generated.params import VectorParams

if TYPE_CHECKING:
    from weaviate import WeaviateAsyncClient
    from weaviate.collections.collection.async_ import CollectionAsync

try:
    import weaviate
    from weaviate.collections.classes.data import DataObject
    from weaviate.collections.classes.types import WeaviateProperties  # noqa: TC002
except ImportError as e:
    _msg = (
        "weaviate-client is required for Weaviate support. "
        "Install with: uv add 'nvisy-dal[weaviate]'"
    )
    raise ImportError(_msg) from e


class WeaviateCredentials(BaseModel, frozen=True):
    """Credentials for Weaviate connection."""

    url: str
    """Weaviate server URL (e.g., 'http://localhost:8080' or Weaviate Cloud URL)."""

    api_key: str | None = None
    """API key for Weaviate Cloud or secured instances."""


class WeaviateParams(VectorParams, frozen=True):
    """Parameters for Weaviate operations.

    Inherits `collection`, `dimension`, `metric`, and `batch_size` from VectorParams.
    """


class WeaviateProvider:
    """Weaviate provider for vector database operations.

    Implements Provider[WeaviateCredentials, WeaviateParams] and
    DataOutput[Embedding].
    """

    __slots__: ClassVar[tuple[str, str, str]] = ("_client", "_collection", "_params")

    _client: "WeaviateAsyncClient"
    _collection: "CollectionAsync"
    _params: WeaviateParams

    def __init__(
        self,
        client: "WeaviateAsyncClient",
        collection: "CollectionAsync",
        params: WeaviateParams,
    ) -> None:
        self._client = client
        self._collection = collection
        self._params = params

    @classmethod
    async def connect(cls, credentials: WeaviateCredentials, params: WeaviateParams) -> Self:
        """Create Weaviate client and verify connection."""
        try:
            if credentials.api_key:
                client = weaviate.use_async_with_weaviate_cloud(
                    cluster_url=credentials.url,
                    auth_credentials=weaviate.auth.AuthApiKey(credentials.api_key),
                )
            else:
                client = weaviate.use_async_with_local(
                    host=credentials.url.replace("http://", "").replace("https://", ""),
                )

            await client.connect()

            if not await client.collections.exists(params.collection):
                msg = f"Collection '{params.collection}' not found"
                raise DalError(msg, kind=ErrorKind.NOT_FOUND)  # noqa: TRY301

            collection = client.collections.get(params.collection)
        except DalError:
            raise
        except Exception as e:
            msg = f"Failed to connect to Weaviate: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, collection, params)

    async def disconnect(self) -> None:
        """Close the Weaviate client."""
        await self._client.close()

    async def write(self, items: Sequence[Embedding]) -> None:
        """Write embeddings to Weaviate."""
        if not items:
            return

        try:
            # Convert embeddings to Weaviate DataObject format
            objects: list[DataObject[WeaviateProperties, None]] = [
                DataObject(
                    uuid=e.id,
                    vector=e.vector,
                    properties=cast("WeaviateProperties", dict(e.metadata)),
                )
                for e in items
            ]

            # Insert in batches
            for i in range(0, len(objects), self._params.batch_size):
                batch = objects[i : i + self._params.batch_size]
                _ = await self._collection.data.insert_many(batch)
        except Exception as e:
            msg = f"Failed to write to Weaviate: {e}"
            raise DalError(msg, source=e) from e


Provider = WeaviateProvider
