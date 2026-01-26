"""Error types for provider operations."""

from enum import StrEnum
from typing import final


class ErrorKind(StrEnum):
    """Classification of provider errors."""

    CONNECTION = "connection"
    NOT_FOUND = "not_found"
    INVALID_INPUT = "invalid_input"
    TIMEOUT = "timeout"
    PROVIDER = "provider"


@final
class DalError(Exception):
    """Base error for all provider operations."""

    __slots__ = ("kind", "message", "source")

    def __init__(
        self,
        message: str,
        kind: ErrorKind = ErrorKind.PROVIDER,
        source: BaseException | None = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.kind = kind
        self.source = source

    def __repr__(self) -> str:
        return f"DalError({self.message!r}, kind={self.kind!r})"
