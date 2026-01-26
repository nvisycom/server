"""PostgreSQL provider using asyncpg."""

from collections.abc import AsyncIterator, Sequence
from typing import TYPE_CHECKING, ClassVar, Self

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind

if TYPE_CHECKING:
    from asyncpg import Pool

try:
    import asyncpg
except ImportError as e:
    _msg = "asyncpg is required for PostgreSQL support. Install with: uv add 'nvisy-dal[postgres]'"
    raise ImportError(_msg) from e


class PostgresCredentials(BaseModel):
    """Credentials for PostgreSQL connection."""

    host: str = "localhost"
    port: int = 5432
    user: str = "postgres"
    password: str
    database: str


class PostgresParams(BaseModel):
    """Parameters for PostgreSQL operations."""

    table: str
    schema_name: str = "public"
    batch_size: int = 1000


class PostgresContext(BaseModel):
    """Context for read/write operations."""

    columns: list[str] | None = None
    where: dict[str, object] | None = None
    order_by: str | None = None
    limit: int | None = None
    offset: int | None = None


class PostgresProvider:
    """PostgreSQL provider for relational data operations."""

    __slots__: ClassVar[tuple[str, str]] = ("_params", "_pool")

    _params: PostgresParams
    _pool: "Pool"

    def __init__(self, pool: "Pool", params: PostgresParams) -> None:
        self._pool = pool
        self._params = params

    @classmethod
    async def connect(
        cls,
        credentials: PostgresCredentials,
        params: PostgresParams,
    ) -> Self:
        """Establish connection pool to PostgreSQL."""
        try:
            pool = await asyncpg.create_pool(
                host=credentials.host,
                port=credentials.port,
                user=credentials.user,
                password=credentials.password,
                database=credentials.database,
                min_size=1,
                max_size=10,
            )
        except Exception as e:
            msg = f"Failed to connect to PostgreSQL: {e}"
            raise DalError(msg, kind=ErrorKind.CONNECTION, source=e) from e

        return cls(pool, params)

    async def disconnect(self) -> None:
        """Close the connection pool."""
        await self._pool.close()

    async def read(self, ctx: PostgresContext) -> AsyncIterator[dict[str, object]]:
        """Read records from the database using parameterized queries."""
        try:
            async with self._pool.acquire() as conn:
                # Build query with proper parameter binding
                columns = ", ".join(f'"{c}"' for c in ctx.columns) if ctx.columns else "*"
                table = f'"{self._params.schema_name}"."{self._params.table}"'

                query_parts: list[str] = [f"SELECT {columns} FROM {table}"]  # noqa: S608
                params: list[object] = []

                if ctx.where:
                    conditions: list[str] = []
                    for key, value in ctx.where.items():
                        if value is None:
                            conditions.append(f'"{key}" IS NULL')
                        else:
                            params.append(value)
                            conditions.append(f'"{key}" = ${len(params)}')
                    if conditions:
                        query_parts.append("WHERE " + " AND ".join(conditions))

                if ctx.order_by:
                    # Order by should be validated/sanitized by caller
                    query_parts.append(f"ORDER BY {ctx.order_by}")

                if ctx.limit is not None:
                    params.append(ctx.limit)
                    query_parts.append(f"LIMIT ${len(params)}")

                if ctx.offset is not None:
                    params.append(ctx.offset)
                    query_parts.append(f"OFFSET ${len(params)}")

                query = " ".join(query_parts)
                async for record in conn.cursor(query, *params):
                    yield dict(record)
        except Exception as e:
            msg = f"Failed to read from PostgreSQL: {e}"
            raise DalError(msg, source=e) from e

    async def write(self, items: Sequence[dict[str, object]]) -> None:
        """Write records to the database."""
        if not items:
            return

        columns = list(items[0].keys())
        placeholders = ", ".join(f"${i + 1}" for i in range(len(columns)))
        column_names = ", ".join(f'"{c}"' for c in columns)
        table = f'"{self._params.schema_name}"."{self._params.table}"'
        query = f"INSERT INTO {table} ({column_names}) VALUES ({placeholders})"  # noqa: S608

        try:
            async with self._pool.acquire() as conn:
                for i in range(0, len(items), self._params.batch_size):
                    batch = items[i : i + self._params.batch_size]
                    await conn.executemany(query, [tuple(item.values()) for item in batch])
        except Exception as e:
            msg = f"Failed to write to PostgreSQL: {e}"
            raise DalError(msg, source=e) from e

    async def execute(self, query: str, *args: object) -> str:
        """Execute a raw SQL query."""
        try:
            async with self._pool.acquire() as conn:
                return await conn.execute(query, *args)
        except Exception as e:
            msg = f"Failed to execute query: {e}"
            raise DalError(msg, source=e) from e

    async def fetch_one(self, query: str, *args: object) -> dict[str, object] | None:
        """Fetch a single record."""
        try:
            async with self._pool.acquire() as conn:
                record = await conn.fetchrow(query, *args)
                return dict(record) if record else None
        except Exception as e:
            msg = f"Failed to fetch record: {e}"
            raise DalError(msg, source=e) from e

    async def fetch_all(self, query: str, *args: object) -> list[dict[str, object]]:
        """Fetch all records."""
        try:
            async with self._pool.acquire() as conn:
                records = await conn.fetch(query, *args)
                return [dict(record) for record in records]
        except Exception as e:
            msg = f"Failed to fetch records: {e}"
            raise DalError(msg, source=e) from e


Provider = PostgresProvider
