"""Provider implementations for external services.

Each provider module exports a `Provider` class alias for the main provider class,
along with its credentials, params, and context types.

Available providers (require optional dependencies):
- postgres: PostgreSQL via asyncpg
- s3: AWS S3 / MinIO via boto3
- pinecone: Pinecone vector database
- qdrant: Qdrant vector database
- milvus: Milvus vector database
- weaviate: Weaviate vector database
"""

from nvisy_dal.providers import milvus, pinecone, postgres, qdrant, s3, weaviate

__all__ = [
    "milvus",
    "pinecone",
    "postgres",
    "qdrant",
    "s3",
    "weaviate",
]
