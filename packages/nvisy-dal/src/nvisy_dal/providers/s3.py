"""S3 provider using boto3."""

from collections.abc import AsyncIterator, Sequence
from typing import TYPE_CHECKING, ClassVar, Self

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind

if TYPE_CHECKING:
    from mypy_boto3_s3 import S3Client

try:
    import boto3
    from botocore.exceptions import ClientError
except ImportError as e:
    _msg = "boto3 is required for S3 support. Install with: uv add 'nvisy-dal[s3]'"
    raise ImportError(_msg) from e


class S3Credentials(BaseModel):
    """Credentials for S3 connection."""

    access_key_id: str
    secret_access_key: str
    region: str = "us-east-1"
    endpoint_url: str | None = None


class S3Params(BaseModel):
    """Parameters for S3 operations."""

    bucket: str
    prefix: str = ""


class S3Context(BaseModel):
    """Context for read/write operations."""

    key: str | None = None
    prefix: str | None = None
    max_keys: int = 1000
    content_type: str = "application/octet-stream"


class S3Object(BaseModel):
    """Representation of an S3 object."""

    key: str
    size: int
    last_modified: str
    etag: str
    content: bytes | None = None


class S3Provider:
    """S3 provider for object storage operations."""

    __slots__: ClassVar[tuple[str, str]] = ("_client", "_params")

    _client: "S3Client"
    _params: S3Params

    def __init__(self, client: "S3Client", params: S3Params) -> None:
        self._client = client
        self._params = params

    @classmethod
    async def connect(cls, credentials: S3Credentials, params: S3Params) -> Self:
        """Create S3 client."""
        try:
            client: S3Client = boto3.client(  # pyright: ignore[reportUnknownMemberType]
                "s3",
                aws_access_key_id=credentials.access_key_id,
                aws_secret_access_key=credentials.secret_access_key,
                region_name=credentials.region,
                endpoint_url=credentials.endpoint_url,
            )
            # Verify connection by checking bucket exists
            _ = client.head_bucket(Bucket=params.bucket)
        except ClientError as e:
            error_code = e.response.get("Error", {}).get("Code", "Unknown")
            if error_code == "404":
                msg = f"Bucket '{params.bucket}' not found"
                raise DalError(msg, kind=ErrorKind.NOT_FOUND, source=e) from e
            msg = f"Failed to connect to S3: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e
        except Exception as e:
            msg = f"Failed to connect to S3: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(client, params)

    async def disconnect(self) -> None:
        """Close the S3 client (no-op for boto3)."""

    async def read(self, ctx: S3Context) -> AsyncIterator[S3Object]:
        """List and optionally read objects from S3."""
        prefix = ctx.prefix or self._params.prefix
        continuation_token: str | None = None

        try:
            while True:
                if continuation_token:
                    response = self._client.list_objects_v2(
                        Bucket=self._params.bucket,
                        Prefix=prefix,
                        MaxKeys=ctx.max_keys,
                        ContinuationToken=continuation_token,
                    )
                else:
                    response = self._client.list_objects_v2(
                        Bucket=self._params.bucket,
                        Prefix=prefix,
                        MaxKeys=ctx.max_keys,
                    )

                for obj in response.get("Contents", []):
                    obj_key = obj.get("Key")
                    obj_size = obj.get("Size")
                    obj_modified = obj.get("LastModified")
                    obj_etag = obj.get("ETag")

                    if not obj_key or obj_size is None or not obj_modified or not obj_etag:
                        continue

                    content = None
                    if ctx.key and obj_key == ctx.key:
                        get_response = self._client.get_object(
                            Bucket=self._params.bucket,
                            Key=obj_key,
                        )
                        content = get_response["Body"].read()

                    yield S3Object(
                        key=obj_key,
                        size=obj_size,
                        last_modified=obj_modified.isoformat(),
                        etag=obj_etag.strip('"'),
                        content=content,
                    )

                if not response.get("IsTruncated"):
                    break

                continuation_token = response.get("NextContinuationToken")

        except ClientError as e:
            msg = f"Failed to read from S3: {e}"
            raise DalError(msg, source=e) from e

    async def write(self, ctx: S3Context, items: Sequence[S3Object]) -> None:
        """Write objects to S3."""
        try:
            for item in items:
                if item.content is None:
                    continue

                key = self._resolve_key(item.key)
                _ = self._client.put_object(
                    Bucket=self._params.bucket,
                    Key=key,
                    Body=item.content,
                    ContentType=ctx.content_type,
                )
        except ClientError as e:
            msg = f"Failed to write to S3: {e}"
            raise DalError(msg, source=e) from e

    async def get(self, key: str) -> bytes:
        """Get object content by key."""
        try:
            full_key = self._resolve_key(key)
            response = self._client.get_object(
                Bucket=self._params.bucket,
                Key=full_key,
            )
            return response["Body"].read()
        except ClientError as e:
            error_code = e.response.get("Error", {}).get("Code", "Unknown")
            if error_code == "NoSuchKey":
                msg = f"Object '{key}' not found"
                raise DalError(msg, kind=ErrorKind.NOT_FOUND, source=e) from e
            msg = f"Failed to get object: {e}"
            raise DalError(msg, source=e) from e

    async def put(
        self,
        key: str,
        content: bytes,
        content_type: str = "application/octet-stream",
    ) -> None:
        """Put object content by key."""
        try:
            full_key = self._resolve_key(key)
            _ = self._client.put_object(
                Bucket=self._params.bucket,
                Key=full_key,
                Body=content,
                ContentType=content_type,
            )
        except ClientError as e:
            msg = f"Failed to put object: {e}"
            raise DalError(msg, source=e) from e

    async def delete(self, key: str) -> None:
        """Delete object by key."""
        try:
            full_key = self._resolve_key(key)
            _ = self._client.delete_object(
                Bucket=self._params.bucket,
                Key=full_key,
            )
        except ClientError as e:
            msg = f"Failed to remove object: {e}"
            raise DalError(msg, source=e) from e

    async def exists(self, key: str) -> bool:
        """Check if object exists."""
        try:
            full_key = self._resolve_key(key)
            _ = self._client.head_object(
                Bucket=self._params.bucket,
                Key=full_key,
            )
        except ClientError as e:
            error_code = e.response.get("Error", {}).get("Code", "Unknown")
            if error_code == "404":
                return False
            msg = f"Failed to check object existence: {e}"
            raise DalError(msg, source=e) from e
        else:
            return True

    def _resolve_key(self, key: str) -> str:
        """Resolve key with prefix if needed."""
        if self._params.prefix and not key.startswith(self._params.prefix):
            return f"{self._params.prefix}{key}"
        return key


Provider = S3Provider
