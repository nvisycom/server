"""PostgreSQL provider using asyncpg."""

from collections.abc import AsyncIterator, Sequence
from typing import TYPE_CHECKING, ClassVar, Self

from pydantic import BaseModel

from nvisy_dal.errors import DalError, ErrorKind
from nvisy_dal.generated.contexts import RelationalContext
from nvisy_dal.generated.params import RelationalParams

if TYPE_CHECKING:
    from asyncpg import Pool

try:
    import asyncpg
except ImportError as e:
    _msg = "asyncpg is required for PostgreSQL support. Install with: uv add 'nvisy-dal[postgres]'"
    raise ImportError(_msg) from e


class PostgresCredentials(BaseModel):
    """Credentials for PostgreSQL connection.

    Uses a connection string (DSN) format: postgres://user:pass@host:port/database
    """

    dsn: str


class PostgresParams(RelationalParams, frozen=True):
    """Parameters for PostgreSQL operations.

    Inherits `table` and `batch_size` from RelationalParams.
    """

    cursor_column: str = "id"
    """Column to use for keyset pagination cursor."""

    schema_name: str = "public"
    """Schema name (defaults to "public")."""

    where: dict[str, object] | None = None
    """WHERE clause conditions as key-value pairs."""


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
                dsn=credentials.dsn,
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

    def _build_where_conditions(
        self,
        params: list[object],
        conditions: list[str],
    ) -> None:
        """Add WHERE conditions from params.where to conditions list."""
        if not self._params.where:
            return
        for key, value in self._params.where.items():
            if value is None:
                conditions.append(f'"{key}" IS NULL')
            else:
                params.append(value)
                conditions.append(f'"{key}" = ${len(params)}')

    def _build_keyset_condition(
        self,
        ctx: RelationalContext,
        params: list[object],
        conditions: list[str],
        cursor_col: str,
    ) -> None:
        """Add keyset pagination condition if cursor exists."""
        if ctx.cursor is None:
            return
        params.append(ctx.cursor)
        if self._params.tiebreaker_column and ctx.tiebreaker is not None:
            tiebreaker_col = f'"{self._params.tiebreaker_column}"'
            params.append(ctx.tiebreaker)
            p1, p2 = len(params) - 1, len(params)
            conditions.append(f"({cursor_col}, {tiebreaker_col}) > (${p1}, ${p2})")
        else:
            conditions.append(f"{cursor_col} > ${len(params)}")

    def _extract_context(self, record_dict: dict[str, object]) -> RelationalContext:
        """Extract resumption context from a record."""
        cursor_val = record_dict.get(self._params.cursor_column, "")
        cursor_value = str(cursor_val) if cursor_val is not None else ""
        tiebreaker_value: str | None = None
        if self._params.tiebreaker_column:
            tb_val = record_dict.get(self._params.tiebreaker_column, "")
            tiebreaker_value = str(tb_val) if tb_val is not None else ""
        return RelationalContext(cursor=cursor_value, tiebreaker=tiebreaker_value)

    async def read(
        self, ctx: RelationalContext
    ) -> AsyncIterator[tuple[dict[str, object], RelationalContext]]:
        """Read records from the database using keyset pagination.

        Yields tuples of (record, context) where context can be used to resume
        reading from the next record if the stream is interrupted.
        """
        try:
            async with self._pool.acquire() as conn:
                columns = (
                    ", ".join(f'"{c}"' for c in self._params.columns)
                    if self._params.columns
                    else "*"
                )
                table = f'"{self._params.schema_name}"."{self._params.table}"'
                cursor_col = f'"{self._params.cursor_column}"'

                query_parts: list[str] = [f"SELECT {columns} FROM {table}"]  # noqa: S608
                params: list[object] = []
                conditions: list[str] = []

                self._build_where_conditions(params, conditions)
                self._build_keyset_condition(ctx, params, conditions, cursor_col)

                if conditions:
                    query_parts.append("WHERE " + " AND ".join(conditions))

                # Order by cursor column(s) for keyset pagination
                if self._params.tiebreaker_column:
                    tiebreaker_col = f'"{self._params.tiebreaker_column}"'
                    query_parts.append(f"ORDER BY {cursor_col}, {tiebreaker_col}")
                else:
                    query_parts.append(f"ORDER BY {cursor_col}")

                query = " ".join(query_parts)
                async for record in conn.cursor(query, *params):
                    record_dict: dict[str, object] = dict(record)
                    yield (record_dict, self._extract_context(record_dict))
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
