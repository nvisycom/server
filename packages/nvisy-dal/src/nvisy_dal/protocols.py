"""Core protocols for data providers."""

from collections.abc import AsyncIterator, Sequence
from typing import Protocol, Self, TypeVar, runtime_checkable

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
Ctx_contra = TypeVar("Ctx_contra", contravariant=True)
Cred_contra = TypeVar("Cred_contra", contravariant=True)
Params_contra = TypeVar("Params_contra", contravariant=True)


@runtime_checkable
class DataInput(Protocol[T_co, Ctx_contra]):
    """Protocol for reading data from external sources."""

    async def read(self, ctx: Ctx_contra) -> AsyncIterator[T_co]:
        """Yield items from the source based on context."""
        ...


@runtime_checkable
class DataOutput(Protocol[T_contra, Ctx_contra]):
    """Protocol for writing data to external sinks."""

    async def write(self, ctx: Ctx_contra, items: Sequence[T_contra]) -> None:
        """Write a batch of items to the sink."""
        ...


@runtime_checkable
class Provider(Protocol[Cred_contra, Params_contra]):
    """Protocol for provider lifecycle management."""

    @classmethod
    async def connect(cls, credentials: Cred_contra, params: Params_contra) -> Self:
        """Establish connection to the external service."""
        ...

    async def disconnect(self) -> None:
        """Release resources and close connections."""
        ...
